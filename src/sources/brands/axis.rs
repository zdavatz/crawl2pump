//! Axis Foils — axisfoils.com — Shopify storefront.
//!
//! Fetched via `/products.json`. Prices are in USD on the main storefront.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_all_products, is_target_product, product_to_listing};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://axisfoils.com";
const BRAND: &str = "Axis";
const CURRENCY: &str = "USD";

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
        let products = fetch_all_products(&self.client, BASE).await?;
        Ok(products
            .iter()
            .filter(|p| is_target_product(p))
            .map(|p| product_to_listing(p, "axis", BRAND, BASE, CURRENCY, Region::World))
            .collect())
    }
}
