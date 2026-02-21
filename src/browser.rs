use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::Page;
use futures::StreamExt;
use tracing::{debug, info};

use crate::error::{ArachneError, Result};

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:126.0) Gecko/20100101 Firefox/126.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:126.0) Gecko/20100101 Firefox/126.0",
];

/// Wrapper around chromiumoxide for CDP-based browser automation.
pub struct BrowserClient {
    browser: Browser,
    _handle: tokio::task::JoinHandle<()>,
}

impl BrowserClient {
    /// Connect to an existing Chrome instance via CDP WebSocket.
    pub async fn connect(ws_url: &str) -> Result<Self> {
        info!(ws_url, "connecting to Chrome via CDP");

        let (browser, mut handler) = Browser::connect(ws_url)
            .await
            .map_err(|e| ArachneError::Browser(format!("failed to connect to Chrome: {e}")))?;

        let handle = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                debug!(?event, "CDP event");
            }
        });

        Ok(Self {
            browser,
            _handle: handle,
        })
    }

    /// Launch a new Chrome instance (for local development).
    pub async fn launch() -> Result<Self> {
        info!("launching headless Chrome");

        let config = BrowserConfig::builder()
            .no_sandbox()
            .arg("--disable-gpu")
            .arg("--disable-dev-shm-usage")
            .arg("--disable-setuid-sandbox")
            .build()
            .map_err(|e| ArachneError::Browser(format!("invalid browser config: {e}")))?;

        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| ArachneError::Browser(format!("failed to launch Chrome: {e}")))?;

        let handle = tokio::spawn(async move {
            while let Some(event) = handler.next().await {
                debug!(?event, "CDP event");
            }
        });

        Ok(Self {
            browser,
            _handle: handle,
        })
    }

    /// Open a new page with a randomized user agent.
    pub async fn new_page(&self) -> Result<Page> {
        let ua = USER_AGENTS[rand::random::<usize>() % USER_AGENTS.len()];

        let page = self
            .browser
            .new_page("about:blank")
            .await
            .map_err(|e| ArachneError::Browser(format!("failed to open page: {e}")))?;

        page.set_user_agent(ua)
            .await
            .map_err(|e| ArachneError::Browser(format!("failed to set user agent: {e}")))?;

        debug!(user_agent = ua, "opened new page");
        Ok(page)
    }

    /// Navigate to a URL and wait for the page to be ready.
    pub async fn navigate_and_wait(page: &Page, url: &str) -> Result<()> {
        debug!(url, "navigating");

        page.goto(url)
            .await
            .map_err(|e| ArachneError::Browser(format!("navigation failed: {e}")))?;

        page.wait_for_navigation()
            .await
            .map_err(|_| ArachneError::NavigationTimeout {
                url: url.to_string(),
            })?;

        // Additional delay for JS rendering
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        Ok(())
    }

    /// Extract the full page HTML content.
    pub async fn get_html(page: &Page) -> Result<String> {
        let html = page
            .content()
            .await
            .map_err(|e| ArachneError::Browser(format!("failed to get page content: {e}")))?;
        Ok(html)
    }

    /// Close a page/tab.
    pub async fn close_page(page: Page) -> Result<()> {
        page.close()
            .await
            .map_err(|e| ArachneError::Browser(format!("failed to close page: {e}")))?;
        Ok(())
    }
}
