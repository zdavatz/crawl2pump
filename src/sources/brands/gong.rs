//! Gong — gong-galaxy.com — Shopify storefront.
//!
//! `gongsurfboards.com` is only a marketing redirect; the actual shop lives
//! at `gong-galaxy.com` (Shopify, EUR pricing). Gong has well-curated pump
//! collections, so we fetch them directly rather than post-filtering the
//! global catalog (which mixes kite/SUP/wing gear). The "Push Edito" rows
//! that show up in these collections are editorial content cards (price=0)
//! filtered out downstream by `pumpfoil_report`.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_collection_products, product_to_listings};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

const BASE: &str = "https://www.gong-galaxy.com";
const BRAND: &str = "Gong";
const CURRENCY: &str = "EUR";
const COLLECTIONS: &[&str] = &[
    "pumping-planches",
    "pumping-packs",
    "pumping-foils-complets",
    "pumping-spare-parts-foil-front-wings",
];

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
                    "gong",
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
