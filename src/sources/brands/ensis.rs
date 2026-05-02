//! Ensis (Switzerland) — ensis.surf — wing/foil brand.
//!
//! Ensis is a brand info site, not a webshop — product pages emit
//! `og:title` / `og:description` / `og:image` but no price and no
//! JSON-LD `Product` schema. Listings come through with `price=None`,
//! which is fine — riders still want to see the new pump-foil lineup.
//!
//! Strategy: sitemap traversal, then a URL allowlist for Ensis's
//! pump-foil-relevant product slugs:
//!   - `pacer` / `maniac-pacer` — the Pacer pump foil
//!   - `stride-pump-foil` / `maniac-stride` / `maniac-stride-ace` — the
//!     Stride pump foil and entry-level pump-foil sets
//!   - `maniac-masts` — the Maniac mast lineup
//!   - `*pumpfoil*` — pump-foil backpacks etc.
//! Wing-foil-only items (Score, Spin, Topspin, Rocknroll, Hip Hop, etc.)
//! are dropped.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::{fetch_page_product, fetch_sitemap_entries};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::Client;

const SITEMAP: &str = "https://ensis.surf/product-sitemap.xml";
const BRAND: &str = "Ensis";
const CONCURRENCY: usize = 6;

pub struct Ensis {
    client: Client,
}

impl Ensis {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Ensis {
    fn name(&self) -> &'static str {
        "ensis"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let entries = fetch_sitemap_entries(&self.client, SITEMAP).await?;
        let candidates: Vec<String> = entries
            .into_iter()
            .filter(|e| e.loc.contains("/product/"))
            .filter(|e| is_pump_foil_slug(&e.loc))
            .map(|e| e.loc)
            .collect();

        let client = &self.client;
        let listings: Vec<Listing> = stream::iter(candidates)
            .map(|url| async move {
                let pp = fetch_page_product(client, &url).await.ok()?;
                let title = pp.title?;
                Some(Listing {
                    source: "ensis".to_string(),
                    brand: Some(BRAND.to_string()),
                    title,
                    url,
                    price: pp.price,
                    currency: pp.currency,
                    condition: Condition::New,
                    available: pp.available,
                    location: Some("Switzerland".to_string()),
                    description: pp.description,
                    image: pp.image,
                    region: Region::Ch,
                    fetched_at: Utc::now(),
                })
            })
            .buffer_unordered(CONCURRENCY)
            .filter_map(|x| async move { x })
            .collect()
            .await;

        Ok(listings)
    }
}

fn is_pump_foil_slug(url: &str) -> bool {
    let u = url.to_lowercase();
    // Drop the Maniac Infinity Ace combo — Infinity is Ensis's wing-foil
    // line, not pump-foil, even though the slug starts with `maniac-`.
    if u.contains("maniac-infinity") {
        return false;
    }
    // Ensis pump-foil model lines + the generic "pumpfoil" keyword.
    // - `pacer` / `stride` are foil model names that slot into pump
    //   foil sets (the Stride is explicit, the Pacer is the high-volume
    //   downwind/pump foil)
    // - `maniac-*` is the entry-level series (Maniac Stride, Maniac
    //   Pacer, Maniac Stride Ace, Maniac Masts) — all pump-foil gear
    // - `pumpfoil` / `pump-foil` catches accessories (Pumpfoil
    //   Backpack, Hip Hop Pumpfoil Backpack)
    u.contains("/pacer")
        || u.contains("/stride")
        || u.contains("/maniac-")
        || u.contains("pumpfoil")
        || u.contains("pump-foil")
        || u.contains("pump_foil")
}
