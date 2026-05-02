//! Pump Zürich (Switzerland) — pump.zuerich — single-product brand.
//!
//! Pump Zürich is a small Swiss pumping community / brand whose only
//! current product is the **Pump Tsüri Skate** — a hand-made pumping
//! skateboard built in Tarifa by Tarifafoilbords. It's a land-trainer
//! for foil pumping technique, not a foil board itself, but it lives
//! in the catalog because it's pump-relevant gear sourced through a
//! pumping community.
//!
//! The product page (`/skate/`) is WordPress.com / Atomic, so it ships
//! `og:title` / `og:image` / `og:description` but no JSON-LD `Product`
//! schema and no `og:price:*` meta. We pull the OG metadata via the
//! shared `fetch_page_product` helper and parse the price out of the
//! free-text description ("Price without shipping is EUR 660.-").
//!
//! The og:title is just "Skate" — too generic to recognise in a mixed
//! brand catalog — so we override the title to "Pump Tsüri Skate" for
//! display.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::fetch_page_product;
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use reqwest::Client;

const PRODUCT_URL: &str = "https://pump.zuerich/skate/";
const BRAND: &str = "Pump Zürich";
const DISPLAY_TITLE: &str = "Pump Tsüri Skate";

pub struct PumpZuerich {
    client: Client,
}

impl PumpZuerich {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for PumpZuerich {
    fn name(&self) -> &'static str {
        "pumpzuerich"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let pp = match fetch_page_product(&self.client, PRODUCT_URL).await {
            Ok(p) => p,
            Err(_) => return Ok(Vec::new()),
        };
        let (price, currency) = pp
            .description
            .as_deref()
            .and_then(extract_price_from_description)
            .map(|p| (Some(p), Some("EUR".to_string())))
            .unwrap_or((pp.price, pp.currency));

        Ok(vec![Listing {
            source: "pumpzuerich".to_string(),
            brand: Some(BRAND.to_string()),
            title: DISPLAY_TITLE.to_string(),
            url: PRODUCT_URL.to_string(),
            price,
            currency,
            condition: Condition::New,
            available: pp.available,
            location: Some("Zürich, Switzerland".to_string()),
            description: pp.description,
            image: pp.image,
            region: Region::Ch,
            fetched_at: Utc::now(),
        }])
    }
}

/// Pump Zürich's product page has no price meta — the price lives in
/// free-text inside `og:description` ("Price without shipping is EUR
/// 660.-"). Match `EUR <number>` so a future CHF price would also
/// trigger if they switch.
fn extract_price_from_description(desc: &str) -> Option<f64> {
    let re = Regex::new(r"(?i)\b(?:EUR|CHF|USD)\s*(\d{2,5})(?:[.,]\-?)?").ok()?;
    let caps = re.captures(desc)?;
    caps.get(1)?.as_str().parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_eur_price() {
        let d = "The PumpSkate by Pump Zürich has landed! Perfect finish, 1.790 g. \
                 Hand made in Tarifa by Tarifafoilbords. Price without shipping is EUR 660.-.";
        assert_eq!(extract_price_from_description(d), Some(660.0));
    }
}
