use sqlx::PgPool;
use uuid::Uuid;

use crate::error::Result;

/// Find an existing profile by canonical phone number (strongest match signal).
pub async fn find_by_phone(pool: &PgPool, phone: &str) -> Result<Option<Uuid>> {
    let row = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM profiles WHERE canonical_phone = $1",
    )
    .bind(phone)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Fallback: find by exact name + city match.
pub async fn find_by_name_city(pool: &PgPool, name: &str, city: &str) -> Result<Option<Uuid>> {
    let row = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM profiles WHERE LOWER(name) = LOWER($1) AND LOWER(city) = LOWER($2)",
    )
    .bind(name)
    .bind(city)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Try to match a scraped profile to an existing one.
/// Returns the profile ID if a match is found.
pub async fn find_match(
    pool: &PgPool,
    phone: Option<&str>,
    name: &str,
    city: &str,
) -> Result<Option<Uuid>> {
    // Phone is the strongest signal
    if let Some(phone) = phone {
        if !phone.is_empty() {
            if let Some(id) = find_by_phone(pool, phone).await? {
                return Ok(Some(id));
            }
        }
    }

    // Fallback to name + city
    find_by_name_city(pool, name, city).await
}
