# Arachne

Classifieds scraper service with headless browser automation, profile deduplication, and S3-compatible photo storage.

## Features

- **Browser automation** via Chrome DevTools Protocol (CDP) using [chromiumoxide](https://github.com/mattsse/chromiumoxide)
- **Trait-based crawler architecture** — add new site scrapers by implementing `SiteCrawler`
- **Profile deduplication** — canonical phone number matching with name+city fallback
- **Photo pipeline** — download, compute perceptual hashes, upload to S3-compatible storage
- **Rate limiting** — randomized delays and optional Redis-backed rate limiter
- **Normalization** — Brazilian phone numbers (E.164), names, city slugs
- **Health endpoint** — `/healthz` for Kubernetes readiness/liveness probes
- **Nix-based builds** — reproducible Docker images via [crate2nix](https://github.com/nix-community/crate2nix)

## Architecture

```
  Listing Page          Profile Page           Storage
  ───────────          ────────────           ───────
  discover_profiles()  scrape_profile()       PostgreSQL (profiles, sources)
       │                    │                 S3 (photos with phash dedup)
       ▼                    ▼                 Redis (rate limiting)
  [ProfileStub]  ──►  [ScrapedProfile]  ──►  upsert + photo pipeline
```

Arachne runs as either:
- **CLI scraper** (`arachne scrape`) — one-shot scrape of a city's listings
- **Long-lived service** (`arachne serve`) — health endpoint for Kubernetes deployments

## Requirements

- Chrome or Chromium with remote debugging enabled (or a headless shell like `chromedp/headless-shell`)
- PostgreSQL 14+
- S3-compatible object storage (MinIO, RustFS, AWS S3)
- Redis (optional, for rate limiting)

## Usage

### CLI

```bash
# Scrape profiles (dry run — prints JSON to stdout)
arachne scrape --site fatal_model --city sao-paulo --pages 3

# Scrape with persistence and photo download
arachne scrape --site fatal_model --city sao-paulo --pages 5 --persist --photos

# Run as a service with health endpoint
arachne serve --port 8080
```

### Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `CHROME_WS_URL` | No | `ws://localhost:9222` | Chrome DevTools Protocol WebSocket URL |
| `DATABASE_URL` | With `--persist` | — | PostgreSQL connection string |
| `REDIS_URL` | No | — | Redis connection string (rate limiting) |
| `S3_ENDPOINT` | With `--photos` | — | S3-compatible endpoint URL |
| `S3_BUCKET` | No | `arachne-photos` | S3 bucket name (auto-created if missing) |
| `S3_ACCESS_KEY` | With `--photos` | — | S3 access key |
| `S3_SECRET_KEY` | With `--photos` | — | S3 secret key |
| `S3_REGION` | No | `us-east-1` | S3 region |
| `RUST_LOG` | No | `arachne=info` | Log level filter |

### Database Setup

Run the migration against your PostgreSQL instance:

```bash
psql $DATABASE_URL -f migrations/001_initial.sql
```

## Building

### With Nix (recommended)

```bash
# Generate Cargo.nix (required once, or after Cargo.lock changes)
nix run .#regenerate-cargo-nix

# Build Docker image (amd64)
nix build .#dockerImage-amd64

# Build Docker image (arm64)
nix build .#dockerImage-arm64
```

### With Cargo

```bash
cargo build --release
```

## Adding a New Crawler

Implement the `SiteCrawler` trait:

```rust
#[async_trait]
pub trait SiteCrawler: Send + Sync {
    fn site_name(&self) -> &str;
    async fn discover_profiles(&self, city: &str, page: u32) -> Result<Vec<ProfileStub>>;
    async fn scrape_profile(&self, stub: &ProfileStub) -> Result<ScrapedProfile>;
}
```

Then register it in `main.rs`:

```rust
let crawler: Box<dyn SiteCrawler> = match site {
    "fatal_model" => Box::new(FatalModelCrawler::new(browser)),
    "your_site" => Box::new(YourSiteCrawler::new(browser)),
    other => anyhow::bail!("unknown site: {other}"),
};
```

## License

[MIT](LICENSE)
