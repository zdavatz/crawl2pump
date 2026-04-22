//! Indiana — indiana-sup.ch — Swiss brand, Shopware backend.
//!
//! Same sitemap+JSON-LD approach as Gong. Indiana ships from Switzerland
//! and sells pre-configured pumpfoil beginner sets — the keyword filter
//! catches both the bundles and the individual foil/board SKUs.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::{fetch_page_product, fetch_sitemap_urls, looks_like_foil_product};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use reqwest::Client;

const SITEMAP: &str = "https://www.indiana-sup.ch/sitemap.xml";
const BRAND: &str = "Indiana";
const CONCURRENCY: usize = 6;
const MAX_PRODUCTS: usize = 120;

pub struct IndianaSup {
    client: Client,
}

impl IndianaSup {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for IndianaSup {
    fn name(&self) -> &'static str {
        "indiana"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let urls = fetch_sitemap_urls(&self.client, SITEMAP).await?;
        let candidates: Vec<String> = urls
            .into_iter()
            .filter(|u| looks_like_foil_product(u))
            .filter(|u| !u.ends_with(".xml"))
            .take(MAX_PRODUCTS)
            .collect();

        let client = &self.client;
        let listings: Vec<Listing> = stream::iter(candidates)
            .map(|url| async move {
                let pp = fetch_page_product(client, &url).await.ok()?;
                let title = pp.title?;
                Some(Listing {
                    source: "indiana".to_string(),
                    brand: Some(BRAND.to_string()),
                    title,
                    url,
                    price: pp.price,
                    currency: pp.currency.or_else(|| Some("CHF".to_string())),
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
