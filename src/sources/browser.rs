//! Shared lazy-launched headless Chrome, used by classifieds sources that
//! sit behind Cloudflare TLS-fingerprint detection and by
//! JS-rendered marketplaces (Facebook Marketplace, later).
//!
//! The browser is launched on first use via `tokio::sync::OnceCell` so that
//! runs which only touch the Shopify brand shops never pay Chrome's
//! startup cost.
use anyhow::{anyhow, Result};
use chromiumoxide::{Browser, BrowserConfig};
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::OnceCell;

#[derive(Debug, Clone, Copy)]
pub struct BrowserOptions {
    pub headful: bool,
}

pub struct SharedBrowser {
    cell: OnceCell<Browser>,
    opts: BrowserOptions,
}

impl SharedBrowser {
    pub fn new(opts: BrowserOptions) -> Arc<Self> {
        Arc::new(Self {
            cell: OnceCell::new(),
            opts,
        })
    }

    pub async fn get(&self) -> Result<&Browser> {
        self.cell.get_or_try_init(|| launch(self.opts)).await
    }

    pub fn is_headful(&self) -> bool {
        self.opts.headful
    }
}

async fn launch(opts: BrowserOptions) -> Result<Browser> {
    let profile: PathBuf = std::env::current_dir()?.join(".chrome-profile");
    std::fs::create_dir_all(&profile).ok();

    // A previous run that was Ctrl-C'd leaves a stale `SingletonLock`
    // that blocks Chrome from starting. If the file is older than 30s we
    // can safely assume the previous Chrome is long gone and remove it.
    let lock = profile.join("SingletonLock");
    if let Ok(meta) = std::fs::symlink_metadata(&lock) {
        let age = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .unwrap_or(Duration::from_secs(0));
        if age > Duration::from_secs(30) {
            std::fs::remove_file(&lock).ok();
            // Chrome also creates `SingletonCookie` / `SingletonSocket` —
            // drop them too so we don't race against their validation.
            std::fs::remove_file(profile.join("SingletonCookie")).ok();
            std::fs::remove_file(profile.join("SingletonSocket")).ok();
        }
    }

    let mut builder = BrowserConfig::builder()
        .user_data_dir(&profile)
        .arg("--disable-blink-features=AutomationControlled")
        .arg("--disable-features=IsolateOrigins,site-per-process,AutomationControlled")
        .arg("--disable-infobars")
        .arg("--no-default-browser-check")
        .arg("--no-first-run")
        .arg("--lang=de-CH")
        // Mimic a real Chrome UA (chromiumoxide's default exposes "HeadlessChrome").
        .arg("--user-agent=Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
              AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36");

    // Prefer the user's real Chrome install over any Chromium-for-Testing
    // chromiumoxide might auto-discover — Cloudflare's challenge platform
    // differentiates the two via subtle runtime tells.
    if let Some(path) = find_real_chrome() {
        builder = builder.chrome_executable(path);
    }

    if opts.headful {
        builder = builder.with_head();
    }

    let config = builder.build().map_err(|e| anyhow!(e))?;
    let (browser, mut handler) = Browser::launch(config).await?;

    // Handler must be polled or CDP events back up and the browser locks.
    tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            let _ = h;
        }
    });

    // Small warmup so the first navigation doesn't race the profile init.
    tokio::time::sleep(Duration::from_millis(200)).await;
    Ok(browser)
}

fn find_real_chrome() -> Option<PathBuf> {
    if let Ok(env_path) = std::env::var("CHROME") {
        let p = PathBuf::from(env_path);
        if p.exists() {
            return Some(p);
        }
    }
    let candidates: &[&str] = &[
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta",
        "/Applications/Google Chrome Dev.app/Contents/MacOS/Google Chrome Dev",
        "/usr/bin/google-chrome-stable",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
    ];
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}
