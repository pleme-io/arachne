use image::ImageReader;
use image_hasher::{HashAlg, HasherConfig};
use std::io::Cursor;
use tracing::debug;
use uuid::Uuid;

use crate::error::{ArachneError, Result};
use crate::storage::rustfs::RustFsClient;

/// Download a photo, compute its perceptual hash, and upload to RustFS.
///
/// Returns (rustfs_path, phash) on success.
pub async fn process_photo(
    http: &reqwest::Client,
    rustfs: &RustFsClient,
    site: &str,
    profile_id: &Uuid,
    photo_url: &str,
) -> Result<(String, Option<i64>)> {
    // Download
    let response = http.get(photo_url).send().await?;

    if !response.status().is_success() {
        return Err(ArachneError::S3(format!(
            "failed to download photo {photo_url}: HTTP {}",
            response.status()
        )));
    }

    let bytes = response.bytes().await?;
    debug!(url = photo_url, size = bytes.len(), "downloaded photo");

    // Compute perceptual hash
    let phash = compute_phash(&bytes);

    // Upload to RustFS
    let ext = photo_url
        .rsplit('.')
        .next()
        .filter(|e| matches!(*e, "jpg" | "jpeg" | "png" | "webp"))
        .unwrap_or("jpg");

    let photo_id = Uuid::new_v4();
    let key = format!("arachne/{site}/{profile_id}/{photo_id}.{ext}");

    rustfs.upload(&key, &bytes).await?;
    debug!(key = %key, phash = ?phash, "uploaded photo to RustFS");

    Ok((key, phash))
}

/// Compute a perceptual hash from image bytes.
fn compute_phash(bytes: &[u8]) -> Option<i64> {
    let img = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;

    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::DoubleGradient)
        .hash_size(8, 8)
        .to_hasher();

    let hash = hasher.hash_image(&img);
    let hash_bytes = hash.as_bytes();

    // Convert first 8 bytes to i64
    if hash_bytes.len() >= 8 {
        Some(i64::from_be_bytes(
            hash_bytes[..8].try_into().unwrap_or([0u8; 8]),
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_phash_invalid_bytes() {
        let result = compute_phash(b"not an image");
        assert!(result.is_none());
    }
}
