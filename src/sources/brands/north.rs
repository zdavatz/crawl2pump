//! North Action Sports — northactionsports.com — Shopify storefront.
//!
//! North has no dedicated "pump foil pack" collection (their packs are
//! sold through dealers and listed individually). For our pump-foil
//! catalog we pull:
//!   - `front-wings`   — Sonar MA / HA / Pulse series. Same multi-
//!                       discipline wings that pump foilers ride
//!                       (the MA xxxx number is area in cm²).
//!   - `foilboards`    — board-only SKUs.
//! Plus we keep a global title-filter for items that mention
//! `pump`/`pumping`/`dockstart`/`pump-foil` in case North adds explicit
//! pump-pack SKUs later.
use crate::listing::{Listing, Region};
use crate::sources::html_util::looks_like_pump_foil;
use crate::sources::shopify::{
    fetch_all_products, fetch_collection_products, is_target_product, product_to_listings,
};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashSet;

const BASE: &str = "https://northactionsports.com";
const BRAND: &str = "North";
const CURRENCY: &str = "EUR";
const COLLECTIONS: &[&str] = &["front-wings", "foilboards"];

pub struct North {
    client: Client,
}

impl North {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for North {
    fn name(&self) -> &'static str {
        "north"
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
                listings.extend(product_to_listings(
                    &p,
                    "north",
                    BRAND,
                    BASE,
                    CURRENCY,
                    Region::World,
                ));
            }
        }
        // Belt-and-suspenders: anything in the global catalog whose
        // title mentions pump-foil terms (in case future SKUs land
        // outside the front-wings / foilboards collections).
        for p in fetch_all_products(&self.client, BASE).await? {
            if !is_target_product(&p) {
                continue;
            }
            if !looks_like_pump_foil(&p.title) {
                continue;
            }
            if !seen.insert(p.handle.clone()) {
                continue;
            }
            listings.extend(product_to_listings(
                &p,
                "north",
                BRAND,
                BASE,
                CURRENCY,
                Region::World,
            ));
        }
        Ok(listings)
    }
}
