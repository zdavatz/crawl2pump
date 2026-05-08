//! Indiana — indiana-sup.ch / indiana-paddlesurf.com — Swiss brand,
//! Magento backend.
//!
//! Discovery: walk the sitemap and keep entries whose `<image:title>`
//! mentions "pump foil" / "pumpfoil". Indiana's product URLs are
//! often SKU-only (`/de_ch/3615sq-3615sq.html`) so URL-keyword filters
//! miss real sets like the Condor XL Complete — the human name only
//! lives in `<image:title>`.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::{
    fetch_page_product, fetch_sitemap_entries, looks_like_front_wing, looks_like_pump_foil,
};
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
        // Indiana sells foil components (front wings, stabilizers, masts)
        // that are pump-foil-capable but don't carry "pump" in the title
        // — e.g. their HP line `Indiana Foil HP Front Wing 920 H-AR`,
        // and the regular line `Indiana Foil Front Wing 820P` /
        // `1150DWR`. We want them in the catalog. So accept any of:
        //   1. SLUG or image-title mentions pumpfoil / pumping / dockstart
        //   2. SLUG contains "front-wing" / "frontwing" / "stabilizer"
        //      (Indiana foil components — pump-relevant by line)
        // The category page entries (`shop/foils/.../front-wings.html`)
        // are caught here too, but get filtered out later when their
        // detail-fetch yields no JSON-LD Product price.
        let entries = fetch_sitemap_entries(&self.client, SITEMAP).await?;
        let candidates: Vec<String> = entries
            .into_iter()
            .filter(|e| !e.loc.ends_with(".xml"))
            .filter(|e| !e.loc.contains("/news/") && !e.loc.contains("/blog/"))
            // Skip category landing pages (they end with .html but have
            // /shop/.../<category>.html paths — they yield no Product).
            .filter(|e| !e.loc.contains("/shop/"))
            .filter(|e| {
                looks_like_pump_foil(&e.loc)
                    || e.titles.iter().any(|t| looks_like_pump_foil(t))
                    || looks_like_front_wing(&e.loc)
                    || e.titles.iter().any(|t| looks_like_front_wing(t))
                    // Indiana sells stabilizers across the same HP /
                    // monobloc lines — keep them as components alongside
                    // the front wings. Some stabilizer products use
                    // SKU-only URLs (e.g. `3569sr-3569sr.html` for the
                    // HP Stabilizer Condor S), so also match the
                    // sitemap's `<image:title>`.
                    || e.loc.contains("stabilizer")
                    || e.titles.iter().any(|t| t.to_lowercase().contains("stabilizer"))
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
