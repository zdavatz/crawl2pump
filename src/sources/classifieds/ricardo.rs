//! Ricardo.ch — Swiss auction/classifieds marketplace.
//!
//! Search URL: `/de/s/{query}?sort=newest`. Each result is an anchor whose
//! `href` contains `/de/a/` (the article detail path). Prices render as
//! `CHF 1'499.-` inside the card container.
use super::{absolute, encode_query, fetch_rendered, parse_swiss_price, walk_up};
use crate::listing::{Condition, Listing, Region};
use crate::sources::browser::SharedBrowser;
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::sync::Arc;

const ORIGIN: &str = "https://www.ricardo.ch";
const CARD_ANCHOR: &str = r#"a[href*="/de/a/"]"#;
const SETTLE_MS: u64 = 5000;

pub struct Ricardo {
    browser: Arc<SharedBrowser>,
}

impl Ricardo {
    pub fn new(browser: Arc<SharedBrowser>) -> Self {
        Self { browser }
    }
}

#[async_trait]
impl Source for Ricardo {
    fn name(&self) -> &'static str {
        "ricardo"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, query: &str) -> Result<Vec<Listing>> {
        let url = format!("{ORIGIN}/de/s/{}?sort=newest", encode_query(query));
        let html = fetch_rendered(&self.browser, &url, SETTLE_MS).await?;
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
            let title = first_nonempty_line(&a.text().collect::<String>())
                .or_else(|| first_nonempty_line(&card_text))
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
                source: "ricardo".to_string(),
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

fn first_nonempty_line(s: &str) -> Option<String> {
    s.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .map(str::to_string)
}
