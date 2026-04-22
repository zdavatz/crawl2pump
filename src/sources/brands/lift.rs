//! Lift Foils — liftfoils.com — Shopify storefront.
//!
//! Lift's catalog is pump-heavy; includes complete foil packages. USD.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_all_products, is_target_product, product_to_listing};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://liftfoils.com";
const BRAND: &str = "Lift";
const CURRENCY: &str = "USD";

pub struct LiftFoils {
    client: Client,
}

impl LiftFoils {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for LiftFoils {
    fn name(&self) -> &'static str {
        "lift"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let products = fetch_all_products(&self.client, BASE).await?;
        Ok(products
            .iter()
            .filter(|p| is_target_product(p))
            .map(|p| product_to_listing(p, "lift", BRAND, BASE, CURRENCY, Region::World))
            .collect())
    }
}
