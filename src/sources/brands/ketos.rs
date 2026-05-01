//! Ketos — ketos-foil.com — French foil maker (carbon foils since 2009).
//!
//! WordPress with WooCommerce. The English shop lives under `/en/...`
//! with category slugs `pumping-en`, `pumping-board`, `pumping-front-wing`,
//! `pumping-packs`. We restrict to `/en/` to keep titles in English and
//! narrow with `looks_like_pump_foil`.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::{fetch_page_product, fetch_sitemap_urls, looks_like_pump_foil};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::Client;

const SITEMAP: &str = "https://www.ketos-foil.com/product-sitemap.xml";
const BRAND: &str = "Ketos";
const CONCURRENCY: usize = 6;
const MAX_PRODUCTS: usize = 60;

pub struct Ketos {
    client: Client,
}

impl Ketos {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Ketos {
    fn name(&self) -> &'static str {
        "ketos"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let urls = fetch_sitemap_urls(&self.client, SITEMAP).await?;
        let candidates: Vec<String> = urls
            .into_iter()
            .filter(|u| !u.ends_with(".xml"))
            // Skip t-shirts, screws, and the FR mirror — keep English shop only.
            .filter(|u| u.contains("/en/"))
            .filter(|u| !u.contains("t-shirt") && !u.contains("/screws-"))
            .filter(|u| looks_like_pump_foil(u))
            .take(MAX_PRODUCTS)
            .collect();

        let client = &self.client;
        let listings: Vec<Listing> = stream::iter(candidates)
            .map(|url| async move {
                let pp = fetch_page_product(client, &url).await.ok()?;
                let title = pp.title?;
                Some(Listing {
                    source: "ketos".to_string(),
                    brand: Some(BRAND.to_string()),
                    title,
                    url,
                    price: pp.price,
                    currency: pp.currency.or_else(|| Some("EUR".to_string())),
                    condition: Condition::New,
                    available: pp.available,
                    location: Some("France".to_string()),
                    description: pp.description,
                    image: pp.image,
                    region: Region::World,
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
