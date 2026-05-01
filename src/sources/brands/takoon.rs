//! Takoon — takoon.com — Shopify storefront, French foil & wing maker.
//!
//! Their `foil-pump` and `pack-foil-pump` collections are surprisingly
//! sparse (don't include the "Pack Pump One Carbon" / "Pack Pump
//! Performance Aluminium" SKUs that are clearly pump-foil bundles).
//! Belt-and-suspenders: pull both curated collections AND title-filter
//! the global catalog for `pump` to catch their pack-pump-* SKUs.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{
    fetch_all_products, fetch_collection_products, is_target_product, product_to_listing,
};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashSet;

const BASE: &str = "https://takoon.com";
const BRAND: &str = "Takoon";
const CURRENCY: &str = "EUR";
const COLLECTIONS: &[&str] = &["pack-foil-pump", "foil-pump"];

pub struct Takoon {
    client: Client,
}

impl Takoon {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Takoon {
    fn name(&self) -> &'static str {
        "takoon"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let mut listings = Vec::new();
        let mut seen = HashSet::new();

        for handle in COLLECTIONS {
            for p in fetch_collection_products(&self.client, BASE, handle).await? {
                if !seen.insert(p.handle.clone()) {
                    continue;
                }
                listings.push(product_to_listing(
                    &p,
                    "takoon",
                    BRAND,
                    BASE,
                    CURRENCY,
                    Region::World,
                ));
            }
        }

        for p in fetch_all_products(&self.client, BASE).await? {
            if !is_target_product(&p) {
                continue;
            }
            let title_lc = p.title.to_lowercase();
            if !title_lc.contains("pump") {
                continue;
            }
            if !seen.insert(p.handle.clone()) {
                continue;
            }
            listings.push(product_to_listing(
                &p,
                "takoon",
                BRAND,
                BASE,
                CURRENCY,
                Region::World,
            ));
        }

        Ok(listings)
    }
}
