pub mod fatal_model;

use async_trait::async_trait;

use crate::error::Result;
use crate::models::{ProfileStub, ScrapedProfile};

/// Trait for site-specific crawlers.
#[async_trait]
pub trait SiteCrawler: Send + Sync {
    /// Site identifier (e.g., "fatal_model", "skokka").
    fn site_name(&self) -> &str;

    /// Discover profile stubs from a listing page.
    async fn discover_profiles(&self, city: &str, page: u32) -> Result<Vec<ProfileStub>>;

    /// Scrape full details from an individual profile page.
    async fn scrape_profile(&self, stub: &ProfileStub) -> Result<ScrapedProfile>;
}
