//! Code Foils — codefoils.com — boutique foil brand, no e-commerce.
//!
//! Code Foils sells through dealers (no retail prices on the brand site)
//! and runs WordPress without a per-product sitemap, so we enumerate
//! product URLs by scraping the `/products/` index page and pull each
//! detail page's OpenGraph title/image. Prices come back as `None` —
//! the user contacts a dealer.
//!
//! Their X / R / S series wings are foiler-designed and pump-foil
//! capable; they don't sell pre-built sets, only components.
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

const BASE: &str = "https://codefoils.com";
const INDEX: &str = "https://codefoils.com/products/";
const BRAND: &str = "Code Foils";
const CONCURRENCY: usize = 4;

pub struct CodeFoils {
    client: Client,
}

impl CodeFoils {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for CodeFoils {
    fn name(&self) -> &'static str {
        "code"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let html = self
            .client
            .get(INDEX)
            .send()
            .await
            .with_context(|| format!("GET {INDEX}"))?
            .error_for_status()?
            .text()
            .await?;
        let urls = extract_product_urls(&html);

        let client = &self.client;
        let listings: Vec<Listing> = stream::iter(urls)
            .map(|url| async move {
                let pp = fetch_page_product(client, &url).await.ok()?;
                let title = pp.title?;
                // Skip apparel that creeps in alongside foil hardware.
                let t = title.to_lowercase();
                if t.contains("apparel") || t.contains("paddle cover") {
                    return None;
                }
                Some(Listing {
                    source: "code".to_string(),
                    brand: Some(BRAND.to_string()),
                    title,
                    url,
                    price: pp.price,
                    currency: pp.currency,
                    condition: Condition::New,
                    available: pp.available,
                    location: Some("USA".to_string()),
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
    let sel = Selector::parse(r#"a[href*="/product/"]"#).unwrap();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for a in doc.select(&sel) {
        if let Some(h) = a.value().attr("href") {
            // /product/foo/ — keep absolute, normalize by trimming query/fragment.
            let url = if h.starts_with("http") {
                h.split(['?', '#']).next().unwrap_or(h).to_string()
            } else {
                format!(
                    "{BASE}{}",
                    h.split(['?', '#']).next().unwrap_or(h)
                )
            };
            if !url.contains("/product/") {
                continue;
            }
            if seen.insert(url.clone()) {
                out.push(url);
            }
        }
    }
    out
}
