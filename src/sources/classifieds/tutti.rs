//! Tutti.ch — Swiss classifieds. Behind Cloudflare "Managed Challenge" /
//! Turnstile, which rejects headless-Chrome clicks. We go through
//! **FlareSolverr** (Docker proxy with proper stealth patches) instead.
use super::{absolute, encode_query, parse_swiss_price, walk_up};
use crate::listing::{Condition, Listing, Region};
use crate::sources::flaresolverr::FlareSolverrClient;
use crate::sources::Source;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::sync::Arc;

const ORIGIN: &str = "https://www.tutti.ch";
const CARD_ANCHOR: &str = r#"a[href*="/de/vi/"]"#;

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
        let url = format!(
            "{ORIGIN}/de/q/suchresultate?query={}&sorting=newest",
            encode_query(query)
        );
        let html = fs.get(&url).await?;

        if let Ok(dir) = std::env::var("CRAWL2PUMP_DEBUG_HTML") {
            if !dir.is_empty() {
                let path = std::path::Path::new(&dir).join("tutti.html");
                std::fs::create_dir_all(&dir).ok();
                std::fs::write(&path, &html).ok();
            }
        }

        let doc = Html::parse_document(&html);
        let link_sel = Selector::parse(CARD_ANCHOR).unwrap();
        let img_sel = Selector::parse("img").unwrap();

        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for a in doc.select(&link_sel) {
            let Some(href) = a.value().attr("href") else {
                continue;
            };
            let abs = absolute(href, ORIGIN);
            if !seen.insert(abs.clone()) {
                continue;
            }
            let card = walk_up(a, 4);
            let card_text = card.text().collect::<String>();
            let title = a
                .text()
                .collect::<String>()
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    card_text
                        .lines()
                        .map(str::trim)
                        .find(|l| !l.is_empty())
                        .map(str::to_string)
                })
                .unwrap_or_default();
            if title.is_empty() || title.len() > 200 {
                continue;
            }
            let price = parse_swiss_price(&card_text);
            let image = card
                .select(&img_sel)
                .next()
                .and_then(|i| i.value().attr("src").map(str::to_string));
            out.push(Listing {
                source: "tutti".to_string(),
                brand: None,
                title,
                url: abs,
                price,
                currency: Some("CHF".to_string()),
                condition: Condition::Used,
                available: Some(true),
                location: None,
                description: None,
                image,
                region: Region::Ch,
                fetched_at: Utc::now(),
            });
        }
        Ok(out)
    }
}
