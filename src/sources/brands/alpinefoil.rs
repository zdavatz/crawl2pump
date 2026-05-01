//! AlpineFoil — alpinefoil.com — French foil maker (Annecy).
//!
//! Sitemap-based, URLs carry plaintext pump-foil keywords
//! (`/kitefoil-windfoil-shop/pumping-dockstart/...`). Pages emit
//! JSON-LD Product. We narrow with `looks_like_pump_foil` and skip
//! the `album-kitefoil/` photo gallery noise.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::{fetch_page_product, fetch_sitemap_urls, looks_like_pump_foil};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::Client;

const SITEMAP: &str = "https://www.alpinefoil.com/sitemap.xml";
const BRAND: &str = "AlpineFoil";
const CONCURRENCY: usize = 6;
const MAX_PRODUCTS: usize = 60;

pub struct AlpineFoil {
    client: Client,
}

impl AlpineFoil {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for AlpineFoil {
    fn name(&self) -> &'static str {
        "alpinefoil"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let urls = fetch_sitemap_urls(&self.client, SITEMAP).await?;
        let candidates: Vec<String> = urls
            .into_iter()
            // The shop lives under /kitefoil-windfoil-shop/ — anything else
            // (album/, blog/, agenda/) is editorial. Products end with
            // `.html`; category pages don't, so this skips category landing
            // pages that also match the pumpfoil keyword.
            .filter(|u| u.contains("/kitefoil-windfoil-shop/"))
            .filter(|u| u.ends_with(".html"))
            .filter(|u| looks_like_pump_foil(u))
            .take(MAX_PRODUCTS)
            .collect();

        let client = &self.client;
        let listings: Vec<Listing> = stream::iter(candidates)
            .map(|url| async move {
                let pp = fetch_page_product(client, &url).await.ok()?;
                let title = pp.title?;
                Some(Listing {
                    source: "alpinefoil".to_string(),
                    brand: Some(BRAND.to_string()),
                    title,
                    url,
                    price: pp.price,
                    currency: pp.currency.or_else(|| Some("EUR".to_string())),
                    condition: Condition::New,
                    available: pp.available,
                    location: Some("France".to_string()),
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
