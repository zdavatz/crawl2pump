//! Mio (mioboards.com) — Eco kite/foil shop, custom Store29 platform.
//!
//! Not Shopify and no per-product sitemap that's useful here. We
//! enumerate `/p/*` product URLs from the `/c/shop/boards/foil` index
//! page (the foil-board section of their shop), then fetch each one
//! for OpenGraph title / price / image. Mio's pump-specific SKU is
//! "Pumpboard - The Beat is Pumping"; their other foil boards
//! (Fritziflitzer, Heavy Rotation, Best Of Two Worlds, Enjoy The
//! Silence, Protoy) are kite-foil / freestyle / freeride and slip
//! through the global pump-foil filter at the report level.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::fetch_page_product;
use crate::sources::Source;
use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::Client;
use scraper::{Html, Selector};
use std::collections::HashSet;

const BASE: &str = "https://www.mioboards.com";
const BRAND: &str = "Mio";
const FOIL_INDEX: &str = "https://www.mioboards.com/c/shop/boards/foil";
const CONCURRENCY: usize = 4;

pub struct Mio {
    client: Client,
}

impl Mio {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Mio {
    fn name(&self) -> &'static str {
        "mio"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let html = self
            .client
            .get(FOIL_INDEX)
            .send()
            .await
            .with_context(|| format!("GET {FOIL_INDEX}"))?
            .error_for_status()?
            .text()
            .await?;
        let urls = extract_product_urls(&html);

        let client = &self.client;
        let listings: Vec<Listing> = stream::iter(urls)
            .map(|url| async move {
                let pp = fetch_page_product(client, &url).await.ok()?;
                let title = pp.title?;
                Some(Listing {
                    source: "mio".to_string(),
                    brand: Some(BRAND.to_string()),
                    title,
                    url,
                    price: pp.price,
                    currency: pp.currency.or_else(|| Some("EUR".to_string())),
                    condition: Condition::New,
                    available: pp.available,
                    location: Some("Switzerland".to_string()),
                    description: pp.description,
                    image: pp.image,
                    region: Region::World,
                    fetched_at: Utc::now(),
                })
            })
            .buffer_unordered(CONCURRENCY)
            .filter_map(|x| async move { x })
            .collect()
            .await;
        Ok(listings)
    }
}

fn extract_product_urls(html: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"a[href^="/p/"]"#).unwrap();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for a in doc.select(&sel) {
        if let Some(h) = a.value().attr("href") {
            let url = format!("{BASE}{}", h.split(['?', '#']).next().unwrap_or(h));
            if seen.insert(url.clone()) {
                out.push(url);
            }
        }
    }
    out
}
