//! Tutti.ch — Swiss classifieds. Behind Cloudflare "Managed Challenge" /
//! Turnstile, fetched via FlareSolverr.
//!
//! ### Search strategy
//!
//! Tutti's search page lives at `/de/q/suche/{token}` where `{token}` is a
//! base64url-encoded msgpack blob encoding filter state. Freetext queries
//! aren't accepted (dropped server-side), so we hit multiple *category*
//! tokens (see `tutti_anibis_cards::CATEGORY_TOKENS`) and filter by the
//! user's query client-side. Each category page shows ~30 most-recent
//! listings, so we get a few hundred listings per crawl across all
//! foil-relevant categories.
use super::tutti_anibis_cards::{matches_query, parse_cards, to_listing, CATEGORY_TOKENS};
use crate::listing::{Listing, Region};
use crate::sources::flaresolverr::FlareSolverrClient;
use crate::sources::Source;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

const ORIGIN: &str = "https://www.tutti.ch";

pub struct Tutti {
    fs: Option<Arc<FlareSolverrClient>>,
}

impl Tutti {
    pub fn new(fs: Option<Arc<FlareSolverrClient>>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Source for Tutti {
    fn name(&self) -> &'static str {
        "tutti"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, query: &str) -> Result<Vec<Listing>> {
        let fs = self.fs.as_ref().ok_or_else(|| {
            anyhow!(
                "Tutti requires FlareSolverr (headless Chrome can't beat Turnstile).\n\
                 \t  → docker run -d --name flaresolverr -p 8191:8191 \
                 ghcr.io/flaresolverr/flaresolverr:latest\n\
                 \t  Then re-run (defaults to http://localhost:8191/v1)."
            )
        })?;

        let debug_dir = std::env::var("CRAWL2PUMP_DEBUG_HTML")
            .ok()
            .filter(|s| !s.is_empty());
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for (slug, token) in CATEGORY_TOKENS {
            let url = format!("{ORIGIN}/de/q/suche/{token}");
            let html = match fs.get(&url).await {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("  tutti[{slug}]: {e}");
                    continue;
                }
            };
            if let Some(dir) = &debug_dir {
                let path = std::path::Path::new(dir).join(format!("tutti_{slug}.html"));
                std::fs::create_dir_all(dir).ok();
                std::fs::write(&path, &html).ok();
            }
            for card in parse_cards(&html, ORIGIN) {
                if !matches_query(query, &card.title, &card.body) {
                    continue;
                }
                if !seen.insert(card.url.clone()) {
                    continue;
                }
                out.push(to_listing("tutti", card));
            }
        }
        Ok(out)
    }
}
