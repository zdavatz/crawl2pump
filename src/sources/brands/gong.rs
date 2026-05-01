//! Gong — gong-galaxy.com — Shopify storefront.
//!
//! `gongsurfboards.com` is only a marketing redirect; the actual shop lives
//! at `gong-galaxy.com` (Shopify, EUR pricing). Gong runs a permanent
//! OUTLET/discount program that surfaces here alongside the full catalog.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_all_products, is_target_product, product_to_listings};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://www.gong-galaxy.com";
const BRAND: &str = "Gong";
const CURRENCY: &str = "EUR";

pub struct GongSurfboards {
    client: Client,
}

impl GongSurfboards {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for GongSurfboards {
    fn name(&self) -> &'static str {
        "gong"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let products = fetch_all_products(&self.client, BASE).await?;
        Ok(products
            .iter()
            .filter(|p| is_target_product(p))
            .flat_map(|p| product_to_listings(p, "gong", BRAND, BASE, CURRENCY, Region::World))
            .collect())
    }
}
