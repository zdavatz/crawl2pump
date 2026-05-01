//! Takuma — storefront URL **UNVERIFIED**, please confirm.
//!
//! `takumafoils.com` returns NXDOMAIN; `takuma.com` exists but does not
//! respond to `/products.json` (may be a corporate splash page, not a
//! shop). Until the canonical Shopify storefront is confirmed this source
//! will error at runtime — update `BASE` below once known.
//!
//! If Takuma has moved to a non-Shopify platform, rewrite this module
//! against `crate::sources::html_util` (see gong.rs history for a template).
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_all_products, is_target_product, product_to_listings};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://takumafoils.com";
const BRAND: &str = "Takuma";
const CURRENCY: &str = "EUR";

pub struct TakumaFoils {
    client: Client,
}

impl TakumaFoils {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for TakumaFoils {
    fn name(&self) -> &'static str {
        "takuma"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let products = fetch_all_products(&self.client, BASE).await?;
        Ok(products
            .iter()
            .filter(|p| is_target_product(p))
            .flat_map(|p| product_to_listings(p, "takuma", BRAND, BASE, CURRENCY, Region::World))
            .collect())
    }
}
