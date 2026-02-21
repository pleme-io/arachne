use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Raw data scraped from a single profile page before normalization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapedProfile {
    pub source_url: String,
    pub source_id: Option<String>,
    pub site: String,
    pub name: String,
    pub city: String,
    pub state: Option<String>,
    pub age: Option<i32>,
    pub phone: Option<String>,
    pub bio: Option<String>,
    pub services: Vec<String>,
    pub pricing: serde_json::Value,
    pub body_stats: serde_json::Value,
    pub photo_urls: Vec<String>,
    pub scraped_at: DateTime<Utc>,
}

/// Stub returned from a listing page — enough to navigate to the full profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileStub {
    pub name: String,
    pub url: String,
    pub thumbnail_url: Option<String>,
    pub city: String,
}

/// Persisted profile after normalization and dedup.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Profile {
    pub id: Uuid,
    pub canonical_phone: Option<String>,
    pub name: String,
    pub city: String,
    pub state: Option<String>,
    pub age: Option<i32>,
    pub bio: Option<String>,
    pub services: Vec<String>,
    pub pricing: serde_json::Value,
    pub body_stats: serde_json::Value,
    pub scrape_count: i32,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Link between a persisted profile and its source on a specific site.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProfileSource {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub site: String,
    pub source_url: String,
    pub source_id: Option<String>,
    pub raw_data: Option<serde_json::Value>,
    pub last_scraped_at: DateTime<Utc>,
}

/// Photo record linking a profile to a stored image.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Photo {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub rustfs_path: String,
    pub original_url: String,
    pub phash: Option<i64>,
    pub position: i32,
    pub downloaded_at: DateTime<Utc>,
}

/// Metadata about a scrape run for auditing.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ScrapeRun {
    pub id: Uuid,
    pub site: String,
    pub city: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub profiles_found: i32,
    pub profiles_new: i32,
    pub profiles_updated: i32,
    pub photos_downloaded: i32,
    pub errors: i32,
    pub status: String,
}

/// City configuration for the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct City {
    pub id: Uuid,
    pub name: String,
    pub state: String,
    pub slug: String,
    pub site: String,
    pub priority: i32,
    pub enabled: bool,
    pub last_full_scrape: Option<DateTime<Utc>>,
}
