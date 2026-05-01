//! Axis Foils — axisfoils.com — Shopify storefront.
//!
//! Fetched via `/collections/all-pump/products.json` rather than the
//! global product list. Axis curates a 128-item pump-foil collection
//! that includes wings (PNG/BSC/HPS/SP/HA series), front wings,
//! fuselages, masts, and the SES beginner packages — none of which
//! carry "pump" in their product titles, so a global fetch +
//! title-keyword filter would miss most of them.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_collection_products, is_target_product, product_to_listings};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://axisfoils.com";
const BRAND: &str = "Axis";
const CURRENCY: &str = "USD";
const COLLECTION: &str = "all-pump";

pub struct AxisFoils {
    client: Client,
}

impl AxisFoils {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for AxisFoils {
    fn name(&self) -> &'static str {
        "axis"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let products = fetch_collection_products(&self.client, BASE, COLLECTION).await?;
        Ok(products
            .iter()
            .filter(|p| is_target_product(p))
            .flat_map(|p| product_to_listings(p, "axis", BRAND, BASE, CURRENCY, Region::World))
            .collect())
    }
}
