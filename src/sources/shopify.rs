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
    pub id: Option<i64>,
    #[serde(default)]
    pub title: Option<String>,
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

/// Convert a Shopify product into one or more `Listing`s — one per
/// variant whose title looks like a *size*. Used for items where the
/// variants are different sizes (Onix Osprey 550/750/950/1250/1450/
/// 1850/2250 cm², Armstrong HA 480/580/680/770 cm², Takoon Foil Pump
/// Front Wing 1500/1700/1900). Each variant gets its own URL
/// (`?variant=<id>`) so the SQLite layer can dedupe and price-track
/// per size.
///
/// Single-variant products and products whose variants are clearly
/// non-size (just "Default Title" or a single name like "Black /
/// Carbon") collapse to one Listing with the min price across
/// variants — same shape as before.
pub fn product_to_listings(
    p: &ShopifyProduct,
    source: &str,
    brand: &str,
    base_url: &str,
    currency: &str,
    region: Region,
) -> Vec<Listing> {
    let base = base_url.trim_end_matches('/');
    let image = p.images.first().map(|i| i.src.clone());
    let description = p.body_html.as_deref().map(strip_html);
    let mut explode = p.variants.len() > 1
        && p.variants
            .iter()
            .any(|v| v.title.as_deref().is_some_and(looks_like_size_variant));
    // Defensive: collapse if every variant has the same `Default Title`
    // (Shopify's placeholder when the seller didn't set one).
    if explode
        && p.variants
            .iter()
            .all(|v| v.title.as_deref() == Some("Default Title"))
    {
        explode = false;
    }
    if !explode {
        let (price, any_available) =
            p.variants.iter().fold((None::<f64>, false), |acc, v| {
                let parsed = v.price.as_ref().and_then(|s| s.parse::<f64>().ok());
                let min = match (acc.0, parsed) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) => Some(a),
                    (None, Some(b)) => Some(b),
                    (None, None) => None,
                };
                (min, acc.1 || v.available)
            });
        return vec![Listing {
            source: source.to_string(),
            brand: Some(brand.to_string()),
            title: p.title.clone(),
            url: format!("{base}/products/{}", p.handle),
            price,
            currency: Some(currency.to_string()),
            condition: Condition::New,
            available: Some(any_available),
            location: None,
            description,
            image,
            region,
            fetched_at: Utc::now(),
        }];
    }
    p.variants
        .iter()
        .map(|v| {
            let suffix = v.title.as_deref().unwrap_or("");
            let title = if suffix.is_empty() {
                p.title.clone()
            } else {
                format!("{} {}", p.title, suffix)
            };
            // URL fragment `?variant=<id>` ensures uniqueness per size
            // even when the storefront pushes them onto the same product
            // page. Falls back to a synthetic `#size=<title>` when the
            // variant has no id.
            let url = match v.id {
                Some(id) => format!("{base}/products/{}?variant={}", p.handle, id),
                None => format!("{base}/products/{}#size={}", p.handle, suffix),
            };
            let price = v.price.as_ref().and_then(|s| s.parse::<f64>().ok());
            Listing {
                source: source.to_string(),
                brand: Some(brand.to_string()),
                title,
                url,
                price,
                currency: Some(currency.to_string()),
                condition: Condition::New,
                available: Some(v.available),
                location: None,
                description: description.clone(),
                image: image.clone(),
                region,
                fetched_at: Utc::now(),
            }
        })
        .collect()
}

/// Backwards-compatible single-listing shim. New code should call
/// `product_to_listings` and consume the resulting Vec.
pub fn product_to_listing(
    p: &ShopifyProduct,
    source: &str,
    brand: &str,
    base_url: &str,
    currency: &str,
    region: Region,
) -> Listing {
    // Take the first listing; for size-varianted products this loses
    // information (the caller should switch to product_to_listings).
    product_to_listings(p, source, brand, base_url, currency, region)
        .into_iter()
        .next()
        .expect("product_to_listings always returns at least one listing")
}

/// True if a Shopify variant title looks like a foil-gear size token
/// rather than a colour/material/option ("OSPREY 1850", "HA570",
/// "S1 1250", "490cm2", "1500", "1700"). Heuristic:
///
/// - Contains a 3-4 digit number that's plausibly a wing area (cm²)
///   or span (mm) — i.e. between 100 and 2500.
/// - The title is short (< 24 chars) so we don't false-positive on
///   long combo strings.
///
/// "Default Title" / "Black" / "Carbon" / "Standard" return false.
pub fn looks_like_size_variant(title: &str) -> bool {
    if title.eq_ignore_ascii_case("default title") {
        return false;
    }
    if title.chars().count() > 24 {
        return false;
    }
    // Variant titles like `2250 / 180 carve / 71` are multi-axis option
    // combos (front-wing size / stabilizer / mast length) — exploding
    // them produces hundreds of pack permutations. Only explode pure
    // size tokens.
    if title.contains('/') {
        return false;
    }
    // `$108` etc. is an upcharge marker for an add-on option (Axis
    // foilboards have a "Foot Strap Holes" variant with `Yes ($108)`
    // / `No` choices). Numbers next to a `$` are prices, not sizes.
    if title.contains('$') {
        return false;
    }
    // "Yes" / "No" / "Default" — option-toggle choices.
    let lower = title.to_lowercase();
    if matches!(lower.trim(), "yes" | "no" | "default" | "standard") {
        return false;
    }
    let re = regex::Regex::new(r"\d{3,4}").unwrap();
    re.find(title)
        .and_then(|m| m.as_str().parse::<u32>().ok())
        .is_some_and(|n| (100..=2500).contains(&n))
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
