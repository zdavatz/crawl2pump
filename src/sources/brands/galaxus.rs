//! Galaxus.ch — curated waterproof accessories.
//!
//! Sibling to `brands::brack` — Galaxus is a Swiss generalist retailer
//! (Digitec/Galaxus group), not a foil brand. We surface cases that
//! work for foil-session sensors (e.g. the STEVAL-MKBOXPRO at
//! 63 × 40 × 20 mm).
//!
//! **No live scrape.** Galaxus sits behind an Akamai bot filter that
//! blocks reqwest's TLS handshake (returns HTTP 403) even with a real
//! Chrome User-Agent and `Sec-Fetch-*` headers — the fingerprint
//! mismatch (rustls vs Chrome's BoringSSL) is detected. The project
//! already runs FlareSolverr for classifieds (Tutti/Anibis), but
//! standing up a full headless-Chrome pipeline for one accessory row
//! isn't worth it. We hard-code the product metadata instead; if the
//! price changes, the constants below need a manual edit. Re-fetch
//! by visiting the URL with a real browser and updating the values.
//!
//! Source name `"galaxus"` is in the trusted-curated set in
//! `src/bin/pumpfoil_report.rs` so titles without pumpfoil keywords
//! still survive the post-source filter.
use crate::listing::{Condition, Listing, Region};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;

/// Static product record. Fields mirror what `parse_page_product`
/// would return from a live JSON-LD fetch; the values below were
/// pulled by hand from the live page on 2026-05-12.
struct StaticProduct {
    title: &'static str,
    brand: &'static str,
    url: &'static str,
    price: f64,
    currency: &'static str,
    image: &'static str,
    description: &'static str,
}

const PRODUCTS: &[StaticProduct] = &[
    StaticProduct {
        title: "Sonoff Waterproof Case IP66 (Gehäuse 132 × 69 × 50 mm)",
        brand: "Sonoff",
        url: "https://www.galaxus.ch/de/s1/product/sonoff-waterproof-case-ip66-gehaeuse-elektronikzubehoer-gehaeuse-33388523",
        price: 5.50,
        currency: "CHF",
        image: "https://static01.galaxus.com/productimages/2/7/1/8/4/4/7/0/3/3/3/5/6/3/5/5/2/6/2/0197878c-bed8-7828-82f8-29eea86f5559_sea.jpeg",
        description: "Sonoff IP66 ist ein hochwertiges wasserdichtes Gehäuse aus ABS V0. \
            Abmessungen: 132,2 × 68,7 × 50,1 mm. Gewicht: 145 g. Schutzgrad IP66 \
            (Strahlwasser, kein dauerhaftes Eintauchen). Kabeldurchführungen für \
            3–6,5 mm Drähte. Ursprünglich für Sonoff-Relais entwickelt — auch \
            passend für Sensor-Boxen (z. B. STEVAL-MKBOXPRO, 63 × 40 × 20 mm).",
    },
    StaticProduct {
        title: "Purecrea IP67 Kunststoffgehäuse transparent (110 × 80 × 45 mm)",
        brand: "Purecrea",
        url: "https://www.galaxus.ch/de/s1/product/purecrea-110x80x45mm-ip67-kunststoffgehaeuse-transparent-gehaeuse-elektronikzubehoer-gehaeuse-39891524",
        price: 21.90,
        currency: "CHF",
        image: "https://static01.galaxus.com/productimages/5/1/5/5/7/1/2/2/9/8/1/0/0/5/7/9/9/6/d69230e7-97bb-4bd8-a73d-0608bcb63c8e_cropped.jpg_sea.jpeg",
        description: "Wasserdichtes IP67 Kunststoffgehäuse mit transparentem Deckel. \
            Abmessungen 110 × 80 × 45 mm. Geeignet für Arduino UNO, ESP32 oder ESP8266 — \
            und für Foil-Session-Sensoren wie den STEVAL-MKBOXPRO (63 × 40 × 20 mm), \
            der bequem hineinpasst. Robuste Kunststoffschrauben für den Deckel, \
            transparenter Deckel macht LEDs / LCDs sichtbar. IP67 = bis 1 m / 30 Min \
            unter Wasser.",
    },
];

pub struct Galaxus;

impl Galaxus {
    pub fn new(_client: Client) -> Self {
        Self
    }
}

#[async_trait]
impl Source for Galaxus {
    fn name(&self) -> &'static str {
        "galaxus"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let now = Utc::now();
        Ok(PRODUCTS
            .iter()
            .map(|p| Listing {
                source: "galaxus".to_string(),
                brand: Some(p.brand.to_string()),
                title: p.title.to_string(),
                url: p.url.to_string(),
                price: Some(p.price),
                currency: Some(p.currency.to_string()),
                condition: Condition::New,
                available: Some(true),
                location: Some("Schweiz".to_string()),
                description: Some(p.description.to_string()),
                image: Some(p.image.to_string()),
                region: Region::Ch,
                fetched_at: now,
            })
            .collect())
    }
}
