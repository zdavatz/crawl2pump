//! Indiana — indiana-sup.ch / indiana-paddlesurf.com — Swiss brand,
//! Magento backend.
//!
//! Discovery: walk the sitemap and keep entries whose `<image:title>`
//! mentions "pump foil" / "pumpfoil". Indiana's product URLs are
//! often SKU-only (`/de_ch/3615sq-3615sq.html`) so URL-keyword filters
//! miss real sets like the Condor XL Complete — the human name only
//! lives in `<image:title>`.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::{fetch_page_product, fetch_sitemap_entries, looks_like_pump_foil};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::Client;

const SITEMAP: &str = "https://www.indiana-sup.ch/sitemap.xml";
const BRAND: &str = "Indiana";
const CONCURRENCY: usize = 6;
const MAX_PRODUCTS: usize = 200;

pub struct IndianaSup {
    client: Client,
}

impl IndianaSup {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for IndianaSup {
    fn name(&self) -> &'static str {
        "indiana"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        // Strict: the SLUG or any <image:title> for the URL must mention
        // pumpfoil / pumping / dockstart. This narrow filter loses
        // unrelated foil components (mast/wing alone) but keeps every
        // real pumpfoil set even when the URL is just an SKU.
        let entries = fetch_sitemap_entries(&self.client, SITEMAP).await?;
        let candidates: Vec<String> = entries
            .into_iter()
            .filter(|e| !e.loc.ends_with(".xml"))
            // /news/ posts and /album-...  pages match the keyword too —
            // skip the editorial side, we want SKUs.
            .filter(|e| !e.loc.contains("/news/") && !e.loc.contains("/blog/"))
            .filter(|e| {
                looks_like_pump_foil(&e.loc)
                    || e.titles.iter().any(|t| looks_like_pump_foil(t))
            })
            .map(|e| e.loc)
            .take(MAX_PRODUCTS)
            .collect();

        let client = &self.client;
        let listings: Vec<Listing> = stream::iter(candidates)
            .map(|url| async move {
                let pp = fetch_page_product(client, &url).await.ok()?;
                let title = pp.title?;
                Some(Listing {
                    source: "indiana".to_string(),
                    brand: Some(BRAND.to_string()),
                    title,
                    url,
                    price: pp.price,
                    currency: pp.currency.or_else(|| Some("CHF".to_string())),
                    condition: Condition::New,
                    available: pp.available,
                    location: Some("Switzerland".to_string()),
                    description: pp.description,
                    image: pp.image,
                    region: Region::Ch,
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
