use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "arachne", about = "Classifieds scraper service")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Scrape profiles from a classifieds site
    Scrape {
        /// Site to scrape (fatal_model, skokka)
        #[arg(long)]
        site: String,

        /// City slug (e.g., sao-paulo)
        #[arg(long)]
        city: String,

        /// Number of listing pages to scrape
        #[arg(long, default_value = "1")]
        pages: u32,

        /// Save to database (requires DATABASE_URL)
        #[arg(long, default_value = "false")]
        persist: bool,

        /// Download and store photos (requires S3 config)
        #[arg(long, default_value = "false")]
        photos: bool,
    },

    /// Run as a long-lived service with health endpoint
    Serve {
        /// HTTP port for health/metrics
        #[arg(long, env = "PORT", default_value = "8080")]
        port: u16,
    },
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub chrome_ws_url: String,
    pub database_url: Option<String>,
    pub redis_url: Option<String>,
    pub s3_endpoint: Option<String>,
    pub s3_bucket: String,
    pub s3_access_key: Option<String>,
    pub s3_secret_key: Option<String>,
    pub s3_region: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            chrome_ws_url: std::env::var("CHROME_WS_URL")
                .unwrap_or_else(|_| "ws://localhost:9222".to_string()),
            database_url: std::env::var("DATABASE_URL").ok(),
            redis_url: std::env::var("REDIS_URL").ok(),
            s3_endpoint: std::env::var("S3_ENDPOINT").ok(),
            s3_bucket: std::env::var("S3_BUCKET")
                .unwrap_or_else(|_| "arachne-photos".to_string()),
            s3_access_key: std::env::var("S3_ACCESS_KEY").ok(),
            s3_secret_key: std::env::var("S3_SECRET_KEY").ok(),
            s3_region: std::env::var("S3_REGION")
                .unwrap_or_else(|_| "us-east-1".to_string()),
        }
    }
}
