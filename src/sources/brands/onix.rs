//! Onix Foils — onix-foils.com — Shopify storefront.
//!
//! Onix has dedicated pump-foil collections so we don't post-filter on
//! the global product list — we fetch `combo-packs` (Pump Starter Pack)
//! and `foil-full-pack` (Osprey/Stingray/Albatros packs) directly and
//! merge.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_collection_products, product_to_listing};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://www.onix-foils.com";
const BRAND: &str = "Onix";
const CURRENCY: &str = "EUR";
const COLLECTIONS: &[&str] = &["combo-packs", "foil-full-pack"];

pub struct OnixFoils {
    client: Client,
}

impl OnixFoils {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for OnixFoils {
    fn name(&self) -> &'static str {
        "onix"
    }
    fn region(&self) -> Region {
        Region::World
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
                listings.push(product_to_listing(
                    &p,
                    "onix",
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
