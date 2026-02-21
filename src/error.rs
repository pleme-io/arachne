use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArachneError {
    #[error("browser error: {0}")]
    Browser(String),

    #[error("navigation timeout: {url}")]
    NavigationTimeout { url: String },

    #[error("selector not found: {selector}")]
    SelectorNotFound { selector: String },

    #[error("scrape error on {site}/{city}: {message}")]
    Scrape {
        site: String,
        city: String,
        message: String,
    },

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("s3 error: {0}")]
    S3(String),

    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("phone parse error: {0}")]
    PhoneParse(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, ArachneError>;
