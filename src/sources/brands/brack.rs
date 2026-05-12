//! Brack.ch — curated waterproof accessories.
//!
//! Brack is a Swiss generalist retailer, not a foil brand — but they
//! carry the Peli Micro Case line at fair Swiss prices, and a small
//! IP67 case is the natural place for foilers to stash a session-log
//! sensor (e.g. the STEVAL-MKBOXPRO at 63 × 40 × 20 mm). We hard-code
//! the seven Peli Micro Case product URLs and pull their JSON-LD
//! `Product` metadata via the shared `fetch_page_product` helper.
//!
//! Source name `"brack"` must be added to the trusted-curated set in
//! `src/bin/pumpfoil_report.rs` — these titles don't carry any of the
//! `pumpfoil` / `pumping` / `dockstart` keywords the post-source
//! filter looks for, so without the trust flag the seven rows are
//! dropped before classification.
//!
//! Rows land in `Category::Accessories` by classifier fall-through
//! (the title contains none of the board / pack / wing keywords).
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::fetch_page_product;
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;

const BRAND: &str = "Peli";

/// `(short_name, brack_product_url)`. `short_name` becomes the
/// Listing title — clean, classifier-neutral text.
const PRODUCTS: &[(&str, &str)] = &[
    (
        "Peli Micro 1010 Schutzkoffer (IP67, wasserdicht)",
        "https://www.brack.ch/peli-schutzkoffer-micro-1010-ohne-schaumstoffeinlage-1483931",
    ),
    (
        "Peli Micro 1015 Schutzkoffer (IP67, wasserdicht)",
        "https://www.brack.ch/peli-schutzkoffer-micro-1015-ohne-schaumstoffeinlage-1483932",
    ),
    (
        "Peli Micro 1020 Schutzkoffer (IP67, wasserdicht)",
        "https://www.brack.ch/peli-schutzkoffer-micro-1020-ohne-schaumstoffeinlage-1483933",
    ),
    (
        "Peli Micro 1030 Schutzkoffer (IP67, wasserdicht)",
        "https://www.brack.ch/peli-schutzkoffer-micro-1030-ohne-schaumstoffeinlage-1483934",
    ),
    (
        "Peli Micro 1040 Schutzkoffer (IP67, wasserdicht)",
        "https://www.brack.ch/peli-schutzkoffer-micro-1040-ohne-schaumstoffeinlage-1483935",
    ),
    (
        "Peli Micro 1050 Schutzkoffer (IP67, wasserdicht)",
        "https://www.brack.ch/peli-schutzkoffer-micro-1050-ohne-schaumstoffeinlage-1483936",
    ),
    (
        "Peli Micro 1060 Schutzkoffer (IP67, wasserdicht)",
        "https://www.brack.ch/peli-schutzkoffer-micro-1060-ohne-schaumstoffeinlage-1483937",
    ),
];

pub struct Brack {
    client: Client,
}

impl Brack {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Brack {
    fn name(&self) -> &'static str {
        "brack"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let mut out = Vec::new();
        for (title, url) in PRODUCTS {
            let pp = match fetch_page_product(&self.client, url).await {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("brack: skipped {url}: {e}");
                    continue;
                }
            };
            out.push(Listing {
                source: "brack".to_string(),
                brand: Some(BRAND.to_string()),
                title: (*title).to_string(),
                url: (*url).to_string(),
                price: pp.price,
                currency: pp.currency.or_else(|| Some("CHF".to_string())),
                condition: Condition::New,
                available: pp.available,
                location: Some("Schweiz".to_string()),
                description: pp.description,
                image: pp.image,
                region: Region::Ch,
                fetched_at: Utc::now(),
            });
        }
        Ok(out)
    }
}
