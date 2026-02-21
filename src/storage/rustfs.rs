use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::Region;
use tracing::info;

use crate::error::{ArachneError, Result};

/// S3-compatible client for RustFS (MinIO-compatible object storage).
pub struct RustFsClient {
    bucket: Box<Bucket>,
}

impl RustFsClient {
    pub async fn new(
        endpoint: &str,
        bucket_name: &str,
        access_key: &str,
        secret_key: &str,
        region: &str,
    ) -> Result<Self> {
        let region = Region::Custom {
            region: region.to_string(),
            endpoint: endpoint.to_string(),
        };

        let credentials = Credentials::new(
            Some(access_key),
            Some(secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| ArachneError::S3(format!("invalid credentials: {e}")))?;

        let mut bucket = Bucket::new(bucket_name, region, credentials)
            .map_err(|e| ArachneError::S3(format!("bucket init error: {e}")))?;

        bucket.set_path_style();

        info!(endpoint, bucket_name, "RustFS client initialized");

        Ok(Self { bucket })
    }

    /// Upload bytes to the given key.
    pub async fn upload(&self, key: &str, data: &[u8]) -> Result<()> {
        let response = self
            .bucket
            .put_object(key, data)
            .await
            .map_err(|e| ArachneError::S3(format!("upload failed for {key}: {e}")))?;

        if response.status_code() >= 300 {
            return Err(ArachneError::S3(format!(
                "upload returned HTTP {}",
                response.status_code()
            )));
        }

        Ok(())
    }

    /// Check if an object exists.
    pub async fn exists(&self, key: &str) -> Result<bool> {
        match self.bucket.head_object(key).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}
