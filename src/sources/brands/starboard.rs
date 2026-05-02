//! Starboard — star-board.com — Shopify storefront.
//!
//! Starboard is primarily a SUP/wing/windsurf brand; their pump-foil
//! product is "Pump Foilboard" (the dedicated pump board) plus the
//! "Foilboard Bag - Pump" carry case. Both live in the `foilboards`
//! collection alongside their wingfoil boards.
//!
//! Strategy: pull `foilboards` and filter via the strict
//! `looks_like_pump_foil` keyword test — only items whose title carries
//! `pump foil`/`pumpfoil` survive. Their wingfoil-only boards (Above /
//! Take Off / Ace Foil / iGnite / X-15) are dropped at the source.
use crate::listing::{Listing, Region};
use crate::sources::html_util::looks_like_pump_foil;
use crate::sources::shopify::{fetch_collection_products, product_to_listings, ShopifyProduct};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashSet;

const BASE: &str = "https://www.star-board.com";
const BRAND: &str = "Starboard";
const CURRENCY: &str = "EUR";
const COLLECTIONS: &[&str] = &["foilboards", "2025-foilboard"];

pub struct Starboard {
    client: Client,
}

impl Starboard {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Starboard {
    fn name(&self) -> &'static str {
        "starboard"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let mut listings = Vec::new();
        let mut seen = HashSet::new();
        for (i, handle) in COLLECTIONS.iter().enumerate() {
            // Same Cloudflare/Shopify rate-limit dance as Naish — sleep
            // between collection fetches and retry once on transient
            // errors so a 429 on one collection doesn't kill the source.
            if i > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            let products = match fetch_with_retry(&self.client, BASE, handle).await {
                Ok(p) => p,
                Err(_) => continue,
            };
            for p in products {
                if !seen.insert(p.handle.clone()) {
                    continue;
                }
                if !looks_like_pump_foil(&p.title) {
                    continue;
                }
                listings.extend(product_to_listings(
                    &p,
                    "starboard",
                    BRAND,
                    BASE,
                    CURRENCY,
                    Region::World,
                ));
            }
        }
        Ok(listings)
    }
}

async fn fetch_with_retry(
    client: &Client,
    base_url: &str,
    handle: &str,
) -> Result<Vec<ShopifyProduct>> {
    match fetch_collection_products(client, base_url, handle).await {
        Ok(p) => Ok(p),
        Err(_) => {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            fetch_collection_products(client, base_url, handle).await
        }
    }
}
