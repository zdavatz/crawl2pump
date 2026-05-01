//! Armstrong Foils — armstrongfoils.com — Shopify storefront.
//!
//! Armstrong has no dedicated "pump foil" collection — they market across
//! disciplines (surf/wing/wake/downwind) and let riders mix the A+ system.
//! Their closest pump-foil package is the **Step One Collection** (S1
//! beginner kit: board + mast + foil kit + front foil + stabilizer),
//! which we fetch directly. Plus we keep the global product list so any
//! item with "pump" in the title (e.g. Pump 202 Stabilizer) still surfaces
//! through the downstream pump-foil filter.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{
    fetch_all_products, fetch_collection_products, is_target_product, product_to_listings,
};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashSet;

const BASE: &str = "https://armstrongfoils.com";
const BRAND: &str = "Armstrong";
const CURRENCY: &str = "USD";
const PUMP_RELEVANT_COLLECTIONS: &[&str] = &["step-one-collection", "front-foils"];

pub struct ArmstrongFoils {
    client: Client,
}

impl ArmstrongFoils {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for ArmstrongFoils {
    fn name(&self) -> &'static str {
        "armstrong"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let mut listings = Vec::new();
        let mut seen = HashSet::new();

        for handle in PUMP_RELEVANT_COLLECTIONS {
            for p in fetch_collection_products(&self.client, BASE, handle).await? {
                if !seen.insert(p.handle.clone()) {
                    continue;
                }
                listings.extend(product_to_listings(
                    &p,
                    "armstrong",
                    BRAND,
                    BASE,
                    CURRENCY,
                    Region::World,
                ));
            }
        }

        // From the global catalog: keep only items whose title literally
        // says "pump" (Pump 202 Stabilizer etc.). Armstrong has no pump
        // collection so we can't filter by category — and we don't want
        // the full 106-item catalog leaking into a pump-foil report.
        for p in fetch_all_products(&self.client, BASE).await? {
            if !is_target_product(&p) {
                continue;
            }
            let title_lc = p.title.to_lowercase();
            let is_pump_titled = title_lc.contains("pump")
                && !title_lc.contains("a-wing pump"); // inflation pump for wings, not foil
            if !is_pump_titled {
                continue;
            }
            if !seen.insert(p.handle.clone()) {
                continue;
            }
            listings.extend(product_to_listings(
                &p,
                "armstrong",
                BRAND,
                BASE,
                CURRENCY,
                Region::World,
            ));
        }

        Ok(listings)
    }
}
