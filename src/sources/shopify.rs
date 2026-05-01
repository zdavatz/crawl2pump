use crate::listing::{Condition, Listing, Region};
use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::Deserialize;

/// A Shopify `/products.json` response.
#[derive(Debug, Deserialize)]
pub struct ShopifyResponse {
    pub products: Vec<ShopifyProduct>,
}

#[derive(Debug, Deserialize)]
pub struct ShopifyProduct {
    pub id: u64,
    pub title: String,
    pub handle: String,
    #[serde(default)]
    pub body_html: Option<String>,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub product_type: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub variants: Vec<ShopifyVariant>,
    #[serde(default)]
    pub images: Vec<ShopifyImage>,
}

#[derive(Debug, Deserialize)]
pub struct ShopifyVariant {
    #[serde(default)]
    pub price: Option<String>,
    #[serde(default)]
    pub available: bool,
}

#[derive(Debug, Deserialize)]
pub struct ShopifyImage {
    pub src: String,
}

/// Fetch every product from a Shopify storefront via `/products.json`.
///
/// Pages through up to 4000 products (16 pages * 250) which is enough for
/// all foil brands; bails early when a page is short.
pub async fn fetch_all_products(
    client: &Client,
    base_url: &str,
) -> Result<Vec<ShopifyProduct>> {
    fetch_paginated(client, base_url, "/products.json").await
}

/// Fetch a single Shopify collection's products via `/collections/<handle>/products.json`.
/// Useful when a brand has a curated pump-foil collection (Axis, e.g.) that
/// classifies items the global /products.json doesn't tag in the title.
pub async fn fetch_collection_products(
    client: &Client,
    base_url: &str,
    collection_handle: &str,
) -> Result<Vec<ShopifyProduct>> {
    let path = format!("/collections/{collection_handle}/products.json");
    fetch_paginated(client, base_url, &path).await
}

async fn fetch_paginated(
    client: &Client,
    base_url: &str,
    path: &str,
) -> Result<Vec<ShopifyProduct>> {
    let base = base_url.trim_end_matches('/');
    let mut all = Vec::new();
    for page in 1..=16 {
        let url = format!("{base}{path}?limit=250&page={page}");
        let resp = client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        if !resp.status().is_success() {
            anyhow::bail!("{url} returned status {}", resp.status());
        }
        let body: ShopifyResponse = resp
            .json()
            .await
            .with_context(|| format!("parse JSON from {url}"))?;
        let n = body.products.len();
        all.extend(body.products);
        if n < 250 {
            break;
        }
    }
    Ok(all)
}

/// Convert a Shopify product into our neutral `Listing` shape.
pub fn product_to_listing(
    p: &ShopifyProduct,
    source: &str,
    brand: &str,
    base_url: &str,
    currency: &str,
    region: Region,
) -> Listing {
    let (price, any_available) = p.variants.iter().fold((None::<f64>, false), |acc, v| {
        let parsed = v.price.as_ref().and_then(|s| s.parse::<f64>().ok());
        let min = match (acc.0, parsed) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        (min, acc.1 || v.available)
    });
    let image = p.images.first().map(|i| i.src.clone());
    let base = base_url.trim_end_matches('/');
    let url = format!("{base}/products/{}", p.handle);
    Listing {
        source: source.to_string(),
        brand: Some(brand.to_string()),
        title: p.title.clone(),
        url,
        price,
        currency: Some(currency.to_string()),
        condition: Condition::New,
        available: Some(any_available),
        location: None,
        description: p.body_html.as_deref().map(strip_html),
        image,
        region,
        fetched_at: Utc::now(),
    }
}

/// Exclusion-list filter: drop obvious merch/accessories so the catalog is
/// foil / board / wing / mast / kit gear.
pub fn is_target_product(p: &ShopifyProduct) -> bool {
    let hay = format!(
        "{} {} {}",
        p.title.to_lowercase(),
        p.product_type.as_deref().unwrap_or("").to_lowercase(),
        p.tags.join(" ").to_lowercase(),
    );
    const EXCLUDE: &[&str] = &[
        "t-shirt", "tshirt", "tee", "hoodie", "cap", "beanie", "hat",
        "sticker", "poster", "mug", "bottle", "lanyard", "sock", "glove",
        "towel", "wax", "leash string", "keychain", "pin badge",
        "boardshort", "wetsuit top", "impact vest", "gift card",
    ];
    !EXCLUDE.iter().any(|e| hay.contains(e))
}

fn strip_html(s: &str) -> String {
    let frag = scraper::Html::parse_fragment(s);
    frag.root_element()
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
