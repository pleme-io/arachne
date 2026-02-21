use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use tracing::debug;

use crate::error::Result;

/// Redis-backed rate limiter for scrape requests.
pub struct RateLimiter {
    conn: ConnectionManager,
}

impl RateLimiter {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url)?;
        let conn = ConnectionManager::new(client).await?;
        Ok(Self { conn })
    }

    /// Check if a request to this site is allowed (sliding window rate limit).
    ///
    /// Returns true if the request is allowed.
    pub async fn check_rate_limit(&self, site: &str, max_per_minute: u32) -> Result<bool> {
        let key = format!("arachne:rate:{site}");
        let now = chrono::Utc::now().timestamp();

        let mut conn = self.conn.clone();

        // Remove entries older than 60 seconds
        let _: () = conn.zrembyscore(&key, 0, now - 60).await?;

        // Count recent requests
        let count: u32 = conn.zcard(&key).await?;

        if count >= max_per_minute {
            debug!(site, count, max_per_minute, "rate limit hit");
            return Ok(false);
        }

        // Add current request
        let _: () = conn.zadd(&key, now, format!("{now}:{}", uuid::Uuid::new_v4())).await?;
        let _: () = conn.expire(&key, 120).await?;

        Ok(true)
    }

    /// Wait until rate limit allows a request.
    pub async fn wait_for_slot(&self, site: &str, max_per_minute: u32) -> Result<()> {
        loop {
            if self.check_rate_limit(site, max_per_minute).await? {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    }
}
