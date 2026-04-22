//! Anibis.ch — Swiss classifieds. Same Next.js codebase as Tutti (same
//! owner) with identical card DOM, same base64url-msgpack URL-token
//! encoding. See `tutti.rs` for the search strategy.
use super::tutti_anibis_cards::{matches_query, parse_cards, to_listing, CATEGORY_TOKENS};
use crate::listing::{Listing, Region};
use crate::sources::flaresolverr::FlareSolverrClient;
use crate::sources::Source;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashSet;
use std::sync::Arc;

const ORIGIN: &str = "https://www.anibis.ch";

pub struct Anibis {
    fs: Option<Arc<FlareSolverrClient>>,
}

impl Anibis {
    pub fn new(fs: Option<Arc<FlareSolverrClient>>) -> Self {
        Self { fs }
    }
}

#[async_trait]
impl Source for Anibis {
    fn name(&self) -> &'static str {
        "anibis"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, query: &str) -> Result<Vec<Listing>> {
        let fs = self.fs.as_ref().ok_or_else(|| {
            anyhow!(
                "Anibis requires FlareSolverr (headless Chrome can't beat Turnstile).\n\
                 \t  → docker run -d --name flaresolverr -p 8191:8191 \
                 ghcr.io/flaresolverr/flaresolverr:latest"
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
                    eprintln!("  anibis[{slug}]: {e}");
                    continue;
                }
            };
            if let Some(dir) = &debug_dir {
                let path = std::path::Path::new(dir).join(format!("anibis_{slug}.html"));
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
                out.push(to_listing("anibis", card));
            }
        }
        Ok(out)
    }
}
