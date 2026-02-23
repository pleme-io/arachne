use async_trait::async_trait;

use crate::error::Result;
use crate::models::{ProfileStub, ScrapedProfile};

/// Trait for site-specific crawlers.
///
/// Implement this trait in a plugin crate and register it via [`crate::App::register`].
#[async_trait]
pub trait SiteCrawler: Send + Sync {
    /// Site identifier used as the key in the crawler registry.
    fn site_name(&self) -> &str;

    /// Discover profile stubs from a listing page.
    async fn discover_profiles(&self, city: &str, page: u32) -> Result<Vec<ProfileStub>>;

    /// Scrape full details from an individual profile page.
    async fn scrape_profile(&self, stub: &ProfileStub) -> Result<ScrapedProfile>;
}
