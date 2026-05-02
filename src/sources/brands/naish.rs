//! Naish — naish.com — Shopify storefront.
//!
//! Naish doesn't tag any of their foil products with "pump" — their
//! pump-relevant lineup is the Glider HA / SHA / Excalibur / Siren
//! "Semi-Complete" sets plus the "Hover Downwind" board. Their nomenclature
//! is generic ("Foil Front Wing", "Foil Mast Carbon", "Hover Wing Ascend
//! Carbon Ultra") so a title-keyword filter would either drop everything
//! or pull the wing/kite boards in too.
//!
//! Strategy: pull the curated `foil-collection` (~97 items, all foil
//! components) and filter by `product_type` — keep front wings / masts
//! / stabilizers / fuselages / semi-completes / SUP+downwind foil boards;
//! drop wing-only boards / kite-foil boards / wing-bar/boom merch.
use crate::listing::{Listing, Region};
use crate::sources::shopify::{fetch_collection_products, product_to_listings, ShopifyProduct};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashSet;

const BASE: &str = "https://www.naish.com";
const BRAND: &str = "Naish";
const CURRENCY: &str = "USD";
const COLLECTIONS: &[&str] = &[
    "foil-collection",
    "foil-completes",
    "foil-boards",
    "front-wings-a-la-cart",
];

/// Allowlist of Naish `product_type` values that are pump-foil-relevant.
/// Wing Boards / Kite Foil Boards / Windsurf Foil Boards / Wings / Wing
/// Booms / Bars / Anchors / Leashes / etc. are excluded — pump foilers
/// don't need them and they'd flood the report.
const ALLOW_PRODUCT_TYPES: &[&str] = &[
    "front wing",
    "mast",
    "stabilizer",
    "fuselage",
    "semi-complete",
    "downwind foil board",
    "sup foil board",
    "foil parts",
    "foil case",
    "board bag",
];

pub struct Naish {
    client: Client,
}

impl Naish {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Naish {
    fn name(&self) -> &'static str {
        "naish"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let mut listings = Vec::new();
        let mut seen = HashSet::new();
        for (i, handle) in COLLECTIONS.iter().enumerate() {
            // Naish rate-limits aggressive Shopify hits — `foil-collection`
            // returns 429 if we slam four collection endpoints
            // back-to-back. Sleep half a second between calls.
            if i > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            let products = match fetch_with_retry(&self.client, BASE, handle).await {
                Ok(p) => p,
                Err(_) => continue,
            };
            for p in products {
                if !seen.insert(p.handle.clone()) {
                    continue;
                }
                let pt = p
                    .product_type
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase();
                if !ALLOW_PRODUCT_TYPES.iter().any(|a| pt == *a) {
                    continue;
                }
                listings.extend(product_to_listings(
                    &p,
                    "naish",
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

/// Naish's Shopify backend 429s when several collection endpoints are
/// hit quickly. Single retry after a 2 s pause clears it; if we still
/// get an error the second time, give up so the source returns the
/// other collections we already have.
async fn fetch_with_retry(
    client: &Client,
    base_url: &str,
    handle: &str,
) -> Result<Vec<ShopifyProduct>> {
    match fetch_collection_products(client, base_url, handle).await {
        Ok(p) => Ok(p),
        Err(_) => {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            fetch_collection_products(client, base_url, handle).await
        }
    }
}
