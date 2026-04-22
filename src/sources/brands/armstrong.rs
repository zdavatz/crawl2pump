//! Armstrong Foils — armstrongfoils.com — Shopify storefront.
//!
//! Armstrong publishes beginner "Kits" under their product catalog. Prices
//! on the canonical site are USD.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_all_products, is_target_product, product_to_listing};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://armstrongfoils.com";
const BRAND: &str = "Armstrong";
const CURRENCY: &str = "USD";

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
        let products = fetch_all_products(&self.client, BASE).await?;
        Ok(products
            .iter()
            .filter(|p| is_target_product(p))
            .map(|p| product_to_listing(p, "armstrong", BRAND, BASE, CURRENCY, Region::World))
            .collect())
    }
}
