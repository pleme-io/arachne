use std::collections::HashMap;

use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use crate::browser::BrowserClient;
use crate::config::{AppConfig, Cli, Command};
use crate::crawler::SiteCrawler;
use crate::error::{ArachneError, Result};
use crate::pipeline::photos;
use crate::storage::{postgres as db, rustfs::RustFsClient};

/// Factory function that creates a site crawler given a browser client.
pub type CrawlerFactory = Box<dyn Fn(BrowserClient) -> Box<dyn SiteCrawler> + Send + Sync>;

/// Application builder for registering site crawlers and running the scraper.
pub struct App {
    crawlers: HashMap<String, CrawlerFactory>,
}

impl App {
    pub fn new() -> Self {
        Self {
            crawlers: HashMap::new(),
        }
    }

    /// Register a named site crawler factory.
    pub fn register(mut self, name: &str, factory: CrawlerFactory) -> Self {
        self.crawlers.insert(name.to_string(), factory);
        self
    }

    /// Parse CLI, connect to services, and execute the requested command.
    pub async fn run(self) -> Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                EnvFilter::new("arachne=info")
            }))
            .init();

        let cli = Cli::parse();
        let config = AppConfig::from_env();

        match cli.command {
            Command::Scrape {
                site,
                city,
                pages,
                persist,
                photos: download_photos,
            } => {
                self.run_scrape(&config, &site, &city, pages, persist, download_photos)
                    .await?;
            }
            Command::Serve { port } => {
                Self::run_server(port).await?;
            }
        }

        Ok(())
    }

    async fn run_scrape(
        &self,
        config: &AppConfig,
        site: &str,
        city: &str,
        pages: u32,
        persist: bool,
        download_photos: bool,
    ) -> Result<()> {
        info!(site, city, pages, persist, download_photos, "starting scrape");

        // Build the crawler from registered factories
        let factory = self.crawlers.get(site).ok_or_else(|| {
            let available: Vec<&str> = self.crawlers.keys().map(|s| s.as_str()).collect();
            ArachneError::Config(format!(
                "unknown site: {site}. Registered: {}",
                available.join(", ")
            ))
        })?;

        let browser = BrowserClient::connect(&config.chrome_ws_url).await?;
        let crawler = factory(browser);

        // Optional: Postgres pool
        let pool = if persist {
            let url = config
                .database_url
                .as_ref()
                .ok_or_else(|| {
                    ArachneError::Config("DATABASE_URL required when --persist is set".to_string())
                })?;
            Some(
                sqlx::postgres::PgPoolOptions::new()
                    .max_connections(5)
                    .connect(url)
                    .await
                    .map_err(ArachneError::Database)?,
            )
        } else {
            None
        };

        // Optional: RustFS client
        let rustfs = if download_photos {
            let endpoint = config
                .s3_endpoint
                .as_ref()
                .ok_or_else(|| {
                    ArachneError::Config("S3_ENDPOINT required when --photos is set".to_string())
                })?;
            let access_key = config
                .s3_access_key
                .as_ref()
                .ok_or_else(|| {
                    ArachneError::Config("S3_ACCESS_KEY required when --photos is set".to_string())
                })?;
            let secret_key = config
                .s3_secret_key
                .as_ref()
                .ok_or_else(|| {
                    ArachneError::Config("S3_SECRET_KEY required when --photos is set".to_string())
                })?;
            let client =
                RustFsClient::new(endpoint, &config.s3_bucket, access_key, secret_key, &config.s3_region)
                    .await?;
            client.ensure_bucket().await?;
            Some(client)
        } else {
            None
        };

        let http_client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/125.0")
            .build()
            .map_err(|e| ArachneError::Other(e.into()))?;

        // Optional: create scrape run record
        let run_id = if let Some(ref pool) = pool {
            Some(db::create_scrape_run(pool, site, city).await?)
        } else {
            None
        };

        let mut total_found = 0i32;
        let mut total_new = 0i32;
        let mut total_updated = 0i32;
        let mut total_photos = 0i32;
        let mut total_errors = 0i32;

        // Discover and scrape profiles
        for page_num in 1..=pages {
            info!(page = page_num, "scraping listing page");

            let stubs = match crawler.discover_profiles(city, page_num).await {
                Ok(s) => s,
                Err(e) => {
                    error!(page = page_num, error = %e, "failed to discover profiles");
                    total_errors += 1;
                    continue;
                }
            };

            if stubs.is_empty() {
                info!(page = page_num, "no more profiles found, stopping");
                break;
            }

            total_found += stubs.len() as i32;

            for stub in &stubs {
                // Random delay between profile scrapes (2-5 seconds)
                let delay = 2000 + rand::random::<u64>() % 3000;
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

                let profile = match crawler.scrape_profile(stub).await {
                    Ok(p) => p,
                    Err(e) => {
                        warn!(url = %stub.url, error = %e, "failed to scrape profile");
                        total_errors += 1;
                        continue;
                    }
                };

                if persist {
                    if let Some(ref pool) = pool {
                        match db::upsert_profile(pool, &profile).await {
                            Ok((profile_id, is_new)) => {
                                if is_new {
                                    total_new += 1;
                                } else {
                                    total_updated += 1;
                                }

                                // Upsert source
                                if let Err(e) = db::upsert_source(pool, profile_id, &profile).await
                                {
                                    warn!(error = %e, "failed to upsert source");
                                    total_errors += 1;
                                }

                                // Download photos
                                if download_photos {
                                    if let Some(ref rustfs) = rustfs {
                                        for (pos, photo_url) in
                                            profile.photo_urls.iter().enumerate()
                                        {
                                            match photos::process_photo(
                                                &http_client,
                                                rustfs,
                                                site,
                                                &profile_id,
                                                photo_url,
                                            )
                                            .await
                                            {
                                                Ok((path, phash)) => {
                                                    if let Err(e) = db::insert_photo(
                                                        pool,
                                                        profile_id,
                                                        &path,
                                                        photo_url,
                                                        phash,
                                                        pos as i32,
                                                    )
                                                    .await
                                                    {
                                                        warn!(error = %e, "failed to insert photo record");
                                                        total_errors += 1;
                                                    } else {
                                                        total_photos += 1;
                                                    }
                                                }
                                                Err(e) => {
                                                    warn!(url = photo_url, error = %e, "failed to process photo");
                                                    total_errors += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!(error = %e, name = %profile.name, "failed to upsert profile");
                                total_errors += 1;
                            }
                        }
                    }
                } else {
                    // Print to stdout as JSON
                    let json = serde_json::to_string_pretty(&profile)
                        .map_err(|e| ArachneError::Other(e.into()))?;
                    println!("{json}");
                }
            }

            // Delay between pages (3-7 seconds)
            if page_num < pages {
                let delay = 3000 + rand::random::<u64>() % 4000;
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }
        }

        // Complete scrape run
        if let (Some(ref pool), Some(run_id)) = (&pool, run_id) {
            let status = if total_errors > 0 {
                "completed_with_errors"
            } else {
                "completed"
            };
            db::complete_scrape_run(
                pool,
                run_id,
                total_found,
                total_new,
                total_updated,
                total_photos,
                total_errors,
                status,
            )
            .await?;
        }

        info!(
            found = total_found,
            new = total_new,
            updated = total_updated,
            photos = total_photos,
            errors = total_errors,
            "scrape complete"
        );

        Ok(())
    }

    async fn run_server(port: u16) -> Result<()> {
        let app = crate::health::router();
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .map_err(|e| ArachneError::Other(e.into()))?;
        info!(port, "arachne health server listening");
        axum::serve(listener, app)
            .await
            .map_err(|e| ArachneError::Other(e.into()))?;
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
