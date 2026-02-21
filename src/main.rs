use clap::Parser;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use arachne::browser::BrowserClient;
use arachne::config::{AppConfig, Cli, Command};
use arachne::crawler::fatal_model::FatalModelCrawler;
use arachne::crawler::SiteCrawler;
use arachne::pipeline::photos;
use arachne::storage::{postgres as db, rustfs::RustFsClient};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("arachne=info")
        }))
        .init();

    let cli = Cli::parse();
    let config = AppConfig::from_env();

    match cli.command {
        Command::Scrape {
            site,
            city,
            pages,
            persist,
            photos: download_photos,
        } => {
            run_scrape(&config, &site, &city, pages, persist, download_photos).await?;
        }
        Command::Serve { port } => {
            run_server(port).await?;
        }
    }

    Ok(())
}

async fn run_scrape(
    config: &AppConfig,
    site: &str,
    city: &str,
    pages: u32,
    persist: bool,
    download_photos: bool,
) -> anyhow::Result<()> {
    info!(site, city, pages, persist, download_photos, "starting scrape");

    // Connect to Chrome
    let browser = BrowserClient::connect(&config.chrome_ws_url).await?;

    // Build the crawler
    let crawler: Box<dyn SiteCrawler> = match site {
        "fatal_model" => Box::new(FatalModelCrawler::new(browser)),
        other => anyhow::bail!("unknown site: {other}. Supported: fatal_model"),
    };

    // Optional: Postgres pool
    let pool = if persist {
        let url = config
            .database_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("DATABASE_URL required when --persist is set"))?;
        Some(
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(5)
                .connect(url)
                .await?,
        )
    } else {
        None
    };

    // Optional: RustFS client
    let rustfs = if download_photos {
        let endpoint = config
            .s3_endpoint
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("S3_ENDPOINT required when --photos is set"))?;
        let access_key = config
            .s3_access_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("S3_ACCESS_KEY required when --photos is set"))?;
        let secret_key = config
            .s3_secret_key
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("S3_SECRET_KEY required when --photos is set"))?;
        let client = RustFsClient::new(endpoint, &config.s3_bucket, access_key, secret_key, &config.s3_region)
            .await?;
        client.ensure_bucket().await?;
        Some(client)
    } else {
        None
    };

    let http_client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/125.0")
        .build()?;

    // Optional: create scrape run record
    let run_id = if let Some(ref pool) = pool {
        Some(db::create_scrape_run(pool, site, city).await?)
    } else {
        None
    };

    let mut total_found = 0i32;
    let mut total_new = 0i32;
    let mut total_updated = 0i32;
    let mut total_photos = 0i32;
    let mut total_errors = 0i32;

    // Discover and scrape profiles
    for page_num in 1..=pages {
        info!(page = page_num, "scraping listing page");

        let stubs = match crawler.discover_profiles(city, page_num).await {
            Ok(s) => s,
            Err(e) => {
                error!(page = page_num, error = %e, "failed to discover profiles");
                total_errors += 1;
                continue;
            }
        };

        if stubs.is_empty() {
            info!(page = page_num, "no more profiles found, stopping");
            break;
        }

        total_found += stubs.len() as i32;

        for stub in &stubs {
            // Random delay between profile scrapes (2-5 seconds)
            let delay = 2000 + rand::random::<u64>() % 3000;
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

            let profile = match crawler.scrape_profile(stub).await {
                Ok(p) => p,
                Err(e) => {
                    warn!(url = %stub.url, error = %e, "failed to scrape profile");
                    total_errors += 1;
                    continue;
                }
            };

            if persist {
                if let Some(ref pool) = pool {
                    match db::upsert_profile(pool, &profile).await {
                        Ok((profile_id, is_new)) => {
                            if is_new {
                                total_new += 1;
                            } else {
                                total_updated += 1;
                            }

                            // Upsert source
                            if let Err(e) = db::upsert_source(pool, profile_id, &profile).await {
                                warn!(error = %e, "failed to upsert source");
                                total_errors += 1;
                            }

                            // Download photos
                            if download_photos {
                                if let Some(ref rustfs) = rustfs {
                                    for (pos, photo_url) in profile.photo_urls.iter().enumerate() {
                                        match photos::process_photo(
                                            &http_client,
                                            rustfs,
                                            site,
                                            &profile_id,
                                            photo_url,
                                        )
                                        .await
                                        {
                                            Ok((path, phash)) => {
                                                if let Err(e) = db::insert_photo(
                                                    pool,
                                                    profile_id,
                                                    &path,
                                                    photo_url,
                                                    phash,
                                                    pos as i32,
                                                )
                                                .await
                                                {
                                                    warn!(error = %e, "failed to insert photo record");
                                                    total_errors += 1;
                                                } else {
                                                    total_photos += 1;
                                                }
                                            }
                                            Err(e) => {
                                                warn!(url = photo_url, error = %e, "failed to process photo");
                                                total_errors += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!(error = %e, name = %profile.name, "failed to upsert profile");
                            total_errors += 1;
                        }
                    }
                }
            } else {
                // Print to stdout as JSON
                println!("{}", serde_json::to_string_pretty(&profile)?);
            }
        }

        // Delay between pages (3-7 seconds)
        if page_num < pages {
            let delay = 3000 + rand::random::<u64>() % 4000;
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        }
    }

    // Complete scrape run
    if let (Some(ref pool), Some(run_id)) = (&pool, run_id) {
        let status = if total_errors > 0 { "completed_with_errors" } else { "completed" };
        db::complete_scrape_run(
            pool,
            run_id,
            total_found,
            total_new,
            total_updated,
            total_photos,
            total_errors,
            status,
        )
        .await?;
    }

    info!(
        found = total_found,
        new = total_new,
        updated = total_updated,
        photos = total_photos,
        errors = total_errors,
        "scrape complete"
    );

    Ok(())
}

async fn run_server(port: u16) -> anyhow::Result<()> {
    let app = arachne::health::router();
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await?;
    info!(port, "arachne health server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
