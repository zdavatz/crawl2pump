//! Onix Foils — onix-foils.com — Shopify storefront.
//!
//! Onix is a pumpfoil-focused brand, so we treat their whole foil-gear
//! catalog as pump-relevant. We fetch the pump-curated collections
//! (`combo-packs`, `foil-full-pack`) plus the per-component collections
//! (`front-wings`, `stabilizers`, `foil-covers` bags, plus the small
//! `foil-adaptors` / `fuselage-adapters` parts) and merge. Apparels +
//! wetsuits are skipped — those are not foil gear.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_collection_products, product_to_listings};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://www.onix-foils.com";
const BRAND: &str = "Onix";
const CURRENCY: &str = "EUR";
const COLLECTIONS: &[&str] = &[
    "combo-packs",
    "foil-full-pack",
    "front-wings",
    "stabilizers",
    "foil-covers",
    "foil-adaptors",
    "fuselage-adapters",
];

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
                listings.extend(product_to_listings(
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
