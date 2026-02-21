use sqlx::PgPool;
use tracing::{debug, info};
use uuid::Uuid;

use crate::error::Result;
use crate::models::ScrapedProfile;
use crate::pipeline::dedup;

/// Upsert a scraped profile into the database.
///
/// Matches by phone first, then by name+city. On match, merges fields
/// without nulling out existing data.
///
/// Returns (profile_id, is_new).
pub async fn upsert_profile(pool: &PgPool, scraped: &ScrapedProfile) -> Result<(Uuid, bool)> {
    let existing = dedup::find_match(
        pool,
        scraped.phone.as_deref(),
        &scraped.name,
        &scraped.city,
    )
    .await?;

    match existing {
        Some(profile_id) => {
            // Merge: update fields only if the scraped value is non-empty/non-null
            sqlx::query(
                r#"
                UPDATE profiles SET
                    canonical_phone = COALESCE($2, canonical_phone),
                    name = CASE WHEN LENGTH($3) > LENGTH(name) THEN $3 ELSE name END,
                    state = COALESCE($4, state),
                    age = COALESCE($5, age),
                    bio = CASE WHEN $6 IS NOT NULL AND LENGTH($6) > COALESCE(LENGTH(bio), 0)
                              THEN $6 ELSE bio END,
                    services = CASE WHEN array_length($7::text[], 1) > COALESCE(array_length(services, 1), 0)
                                    THEN $7 ELSE services END,
                    pricing = CASE WHEN $8::jsonb != '{}'::jsonb THEN $8 ELSE pricing END,
                    body_stats = CASE WHEN $9::jsonb != '{}'::jsonb THEN $9 ELSE body_stats END,
                    scrape_count = scrape_count + 1,
                    last_seen_at = NOW(),
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(profile_id)
            .bind(&scraped.phone)
            .bind(&scraped.name)
            .bind(&scraped.state)
            .bind(scraped.age)
            .bind(&scraped.bio)
            .bind(&scraped.services)
            .bind(&scraped.pricing)
            .bind(&scraped.body_stats)
            .execute(pool)
            .await?;

            debug!(profile_id = %profile_id, name = %scraped.name, "updated existing profile");
            Ok((profile_id, false))
        }
        None => {
            let profile_id = Uuid::new_v4();
            sqlx::query(
                r#"
                INSERT INTO profiles (id, canonical_phone, name, city, state, age, bio, services, pricing, body_stats)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(profile_id)
            .bind(&scraped.phone)
            .bind(&scraped.name)
            .bind(&scraped.city)
            .bind(&scraped.state)
            .bind(scraped.age)
            .bind(&scraped.bio)
            .bind(&scraped.services)
            .bind(&scraped.pricing)
            .bind(&scraped.body_stats)
            .execute(pool)
            .await?;

            info!(profile_id = %profile_id, name = %scraped.name, "inserted new profile");
            Ok((profile_id, true))
        }
    }
}

/// Upsert a profile source record (site + source_url unique constraint).
pub async fn upsert_source(
    pool: &PgPool,
    profile_id: Uuid,
    scraped: &ScrapedProfile,
) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO profile_sources (id, profile_id, site, source_url, source_id, raw_data, last_scraped_at)
        VALUES ($1, $2, $3, $4, $5, $6, NOW())
        ON CONFLICT (site, source_url) DO UPDATE SET
            profile_id = $2,
            source_id = COALESCE($5, profile_sources.source_id),
            raw_data = $6,
            last_scraped_at = NOW()
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(profile_id)
    .bind(&scraped.site)
    .bind(&scraped.source_url)
    .bind(&scraped.source_id)
    .bind(serde_json::to_value(scraped).ok())
    .execute(pool)
    .await?;

    Ok(())
}

/// Insert a photo record, skipping if the same phash already exists for this profile.
pub async fn insert_photo(
    pool: &PgPool,
    profile_id: Uuid,
    rustfs_path: &str,
    original_url: &str,
    phash: Option<i64>,
    position: i32,
) -> Result<bool> {
    let result = sqlx::query(
        r#"
        INSERT INTO photos (id, profile_id, rustfs_path, original_url, phash, position)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (profile_id, phash) DO NOTHING
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(profile_id)
    .bind(rustfs_path)
    .bind(original_url)
    .bind(phash)
    .bind(position)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Create a new scrape run record.
pub async fn create_scrape_run(pool: &PgPool, site: &str, city: &str) -> Result<Uuid> {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO scrape_runs (id, site, city) VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(site)
    .bind(city)
    .execute(pool)
    .await?;

    Ok(id)
}

/// Update a scrape run with final stats.
pub async fn complete_scrape_run(
    pool: &PgPool,
    run_id: Uuid,
    profiles_found: i32,
    profiles_new: i32,
    profiles_updated: i32,
    photos_downloaded: i32,
    errors: i32,
    status: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE scrape_runs SET
            completed_at = NOW(),
            profiles_found = $2,
            profiles_new = $3,
            profiles_updated = $4,
            photos_downloaded = $5,
            errors = $6,
            status = $7
        WHERE id = $1
        "#,
    )
    .bind(run_id)
    .bind(profiles_found)
    .bind(profiles_new)
    .bind(profiles_updated)
    .bind(photos_downloaded)
    .bind(errors)
    .bind(status)
    .execute(pool)
    .await?;

    Ok(())
}
