use async_trait::async_trait;
use chrono::Utc;
use scraper::{Html, Selector};
use tracing::{debug, info};

use crate::browser::BrowserClient;
use crate::error::Result;
use crate::models::{ProfileStub, ScrapedProfile};
use crate::pipeline::normalize;

use super::SiteCrawler;

const BASE_URL: &str = "https://fatalmodel.com";

pub struct FatalModelCrawler {
    browser: BrowserClient,
}

impl FatalModelCrawler {
    pub fn new(browser: BrowserClient) -> Self {
        Self { browser }
    }

    fn listing_url(city: &str, page: u32) -> String {
        if page <= 1 {
            format!("{BASE_URL}/acompanhantes/{city}")
        } else {
            format!("{BASE_URL}/acompanhantes/{city}?page={page}")
        }
    }

    fn parse_listing_page(html: &str, city: &str) -> Vec<ProfileStub> {
        let document = Html::parse_document(html);
        let mut stubs = Vec::new();

        // Fatal Model uses card-based listings. The exact selectors may need
        // adjustment as the site evolves, but the common patterns are:
        // - Profile cards in a grid/list container
        // - Each card has a link to the profile page, name, and thumbnail
        let card_selector = Selector::parse(
            "a[href*='/acompanhantes/'][href*='-']"
        ).expect("valid selector");

        let img_selector = Selector::parse("img").expect("valid selector");

        for card in document.select(&card_selector) {
            let href = match card.value().attr("href") {
                Some(h) if h.contains("/acompanhantes/") && h.len() > 20 => h,
                _ => continue,
            };

            // Skip pagination/category links
            if href.contains("?page=") || href.ends_with(&format!("/{city}")) {
                continue;
            }

            let url = if href.starts_with("http") {
                href.to_string()
            } else {
                format!("{BASE_URL}{href}")
            };

            let name = card.text().collect::<Vec<_>>().join(" ");
            let name = name.trim().to_string();
            if name.is_empty() {
                continue;
            }

            let thumbnail_url = card
                .select(&img_selector)
                .next()
                .and_then(|img| img.value().attr("src").or_else(|| img.value().attr("data-src")))
                .map(String::from);

            stubs.push(ProfileStub {
                name,
                url,
                thumbnail_url,
                city: city.to_string(),
            });
        }

        // Deduplicate by URL
        stubs.sort_by(|a, b| a.url.cmp(&b.url));
        stubs.dedup_by(|a, b| a.url == b.url);

        stubs
    }

    fn parse_profile_page(html: &str, stub: &ProfileStub) -> Result<ScrapedProfile> {
        let document = Html::parse_document(html);

        let name = Self::extract_text(&document, "h1")
            .unwrap_or_else(|| stub.name.clone());

        let bio = Self::extract_text(&document, "[class*='description'], [class*='bio'], [class*='about']");

        let age = Self::extract_text(&document, "[class*='age'], [class*='idade']")
            .and_then(|t| {
                t.chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse::<i32>()
                    .ok()
            });

        let phone = Self::extract_text(
            &document,
            "a[href^='tel:'], [class*='phone'], [class*='whatsapp'], [class*='telefone']",
        )
        .map(|p| normalize::normalize_phone(&p));

        // Extract photo URLs
        let img_selector = Selector::parse(
            "[class*='gallery'] img, [class*='photo'] img, [class*='slider'] img, [class*='carousel'] img"
        ).expect("valid selector");

        let photo_urls: Vec<String> = document
            .select(&img_selector)
            .filter_map(|img| {
                img.value()
                    .attr("src")
                    .or_else(|| img.value().attr("data-src"))
                    .map(String::from)
            })
            .filter(|url| !url.contains("placeholder") && !url.contains("avatar"))
            .collect();

        // Extract services/tags
        let services = Self::extract_list(
            &document,
            "[class*='service'] li, [class*='tag'], [class*='skill'], [class*='category'] span",
        );

        // Extract pricing info
        let pricing = Self::extract_pricing(&document);

        // Extract body stats
        let body_stats = Self::extract_body_stats(&document);

        // Extract state from breadcrumb or location
        let state = Self::extract_text(&document, "[class*='state'], [class*='uf'], [class*='location'] span");

        // Try to extract source ID from URL
        let source_id = stub
            .url
            .split('/')
            .last()
            .and_then(|segment| {
                // Fatal Model URLs often end with name-id pattern
                segment.rsplit('-').next()
            })
            .and_then(|id| {
                if id.chars().all(|c| c.is_ascii_digit()) && !id.is_empty() {
                    Some(id.to_string())
                } else {
                    None
                }
            });

        Ok(ScrapedProfile {
            source_url: stub.url.clone(),
            source_id,
            site: "fatal_model".to_string(),
            name,
            city: stub.city.clone(),
            state,
            age,
            phone,
            bio,
            services,
            pricing,
            body_stats,
            photo_urls,
            scraped_at: Utc::now(),
        })
    }

    fn extract_text(document: &Html, selector_str: &str) -> Option<String> {
        let selector = Selector::parse(selector_str).ok()?;
        document.select(&selector).next().map(|el| {
            el.text().collect::<Vec<_>>().join(" ").trim().to_string()
        }).filter(|s| !s.is_empty())
    }

    fn extract_list(document: &Html, selector_str: &str) -> Vec<String> {
        let selector = match Selector::parse(selector_str) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };

        document
            .select(&selector)
            .map(|el| el.text().collect::<Vec<_>>().join(" ").trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn extract_pricing(document: &Html) -> serde_json::Value {
        let mut pricing = serde_json::Map::new();

        let selector_str = "[class*='price'], [class*='valor'], [class*='rate']";
        if let Ok(selector) = Selector::parse(selector_str) {
            for el in document.select(&selector) {
                let text = el.text().collect::<Vec<_>>().join(" ");
                let text = text.trim();
                if !text.is_empty() {
                    // Try to extract duration-price pairs
                    let clean = text.replace("R$", "").replace('.', "").replace(',', ".");
                    if let Ok(val) = clean.trim().parse::<f64>() {
                        let label = el
                            .value()
                            .attr("class")
                            .unwrap_or("price")
                            .to_string();
                        pricing.insert(label, serde_json::Value::from(val));
                    }
                }
            }
        }

        serde_json::Value::Object(pricing)
    }

    fn extract_body_stats(document: &Html) -> serde_json::Value {
        let mut stats = serde_json::Map::new();

        let fields = &[
            ("height", "[class*='height'], [class*='altura']"),
            ("weight", "[class*='weight'], [class*='peso']"),
            ("ethnicity", "[class*='ethnic'], [class*='etnia']"),
            ("hair", "[class*='hair'], [class*='cabelo']"),
            ("eyes", "[class*='eyes'], [class*='olhos']"),
        ];

        for (key, selector_str) in fields {
            if let Some(value) = Self::extract_text(document, selector_str) {
                stats.insert(key.to_string(), serde_json::Value::String(value));
            }
        }

        serde_json::Value::Object(stats)
    }
}

#[async_trait]
impl SiteCrawler for FatalModelCrawler {
    fn site_name(&self) -> &str {
        "fatal_model"
    }

    async fn discover_profiles(&self, city: &str, page_num: u32) -> Result<Vec<ProfileStub>> {
        let url = Self::listing_url(city, page_num);
        info!(url = %url, city, page = page_num, "discovering profiles");

        let page = self.browser.new_page().await?;
        BrowserClient::navigate_and_wait(&page, &url).await?;
        let html = BrowserClient::get_html(&page).await?;
        BrowserClient::close_page(page).await?;

        let stubs = Self::parse_listing_page(&html, city);
        info!(count = stubs.len(), city, page = page_num, "found profile stubs");

        Ok(stubs)
    }

    async fn scrape_profile(&self, stub: &ProfileStub) -> Result<ScrapedProfile> {
        info!(url = %stub.url, name = %stub.name, "scraping profile");

        let page = self.browser.new_page().await?;
        BrowserClient::navigate_and_wait(&page, &stub.url).await?;
        let html = BrowserClient::get_html(&page).await?;
        BrowserClient::close_page(page).await?;

        let profile = Self::parse_profile_page(&html, stub)?;
        debug!(
            name = %profile.name,
            phone = ?profile.phone,
            photos = profile.photo_urls.len(),
            services = profile.services.len(),
            "scraped profile"
        );

        Ok(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_listing_url() {
        assert_eq!(
            FatalModelCrawler::listing_url("sao-paulo", 1),
            "https://fatalmodel.com/acompanhantes/sao-paulo"
        );
        assert_eq!(
            FatalModelCrawler::listing_url("sao-paulo", 3),
            "https://fatalmodel.com/acompanhantes/sao-paulo?page=3"
        );
    }

    #[test]
    fn test_parse_listing_empty() {
        let html = "<html><body><div>No results</div></body></html>";
        let stubs = FatalModelCrawler::parse_listing_page(html, "sao-paulo");
        assert!(stubs.is_empty());
    }

    #[test]
    fn test_parse_listing_with_cards() {
        let html = r#"
        <html><body>
            <a href="/acompanhantes/sao-paulo/maria-123456">
                <img src="https://cdn.fatalmodel.com/thumb1.jpg" />
                Maria Silva
            </a>
            <a href="/acompanhantes/sao-paulo/ana-789012">
                <img src="https://cdn.fatalmodel.com/thumb2.jpg" />
                Ana Santos
            </a>
        </body></html>
        "#;

        let stubs = FatalModelCrawler::parse_listing_page(html, "sao-paulo");
        assert_eq!(stubs.len(), 2);
        assert!(stubs[0].url.contains("ana-789012") || stubs[0].url.contains("maria-123456"));
    }
}
