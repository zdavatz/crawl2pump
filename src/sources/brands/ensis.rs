//! Ensis (Switzerland) — ensis.surf — Shopify storefront.
//!
//! Ensis migrated from a WordPress brand-info site (no prices, og-only)
//! to a full Shopify shop in 2026. We now pull their four curated
//! pump-foil collections — `pump-foiling` (combo sets), `pump-foils`,
//! `pump-boards`, `pump-accessories` — and merge.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_collection_products, product_to_listings};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://ensis.surf";
const BRAND: &str = "Ensis";
const CURRENCY: &str = "EUR";
const COLLECTIONS: &[&str] = &[
    "pump-foiling",
    "pump-foils",
    "pump-boards",
    "pump-accessories",
];

pub struct Ensis {
    client: Client,
}

impl Ensis {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Ensis {
    fn name(&self) -> &'static str {
        "ensis"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let mut listings = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for handle in COLLECTIONS {
            let products = fetch_collection_products(&self.client, BASE, handle).await?;
            for p in products {
                if !seen.insert(p.handle.clone()) {
                    continue;
                }
                listings.extend(product_to_listings(
                    &p,
                    "ensis",
                    BRAND,
                    BASE,
                    CURRENCY,
                    Region::Ch,
                ));
            }
        }
        Ok(listings)
    }
}
