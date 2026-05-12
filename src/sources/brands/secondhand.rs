//! Second-hand community adverts.
//!
//! Holds hand-curated entries for used gear that community members
//! offer — currently a single complete Onix/MIO set that was posted
//! to the Pump Tsüri FB Page on 2026-05-10. Lives as a "brand source"
//! so the existing pumpfoil_report pipeline picks it up uniformly:
//! curated-filter passes (source is trusted), classifier lands it in
//! Sets, the PDF render treats it like any other entry.
//!
//! Title prefix `🔁 GEBRAUCHT — ` is what visually flags the row as
//! used to a reader flipping through the Sets section.
//!
//! Images are baked into the binary via `include_bytes!` and served
//! as `data:image/jpeg;base64,…` data URLs. We don't host them on the
//! original FB CDN because those URLs carry session-bound `oh=`/`oe=`
//! tokens that work in some clients (curl) but flake in
//! `optimize_thumbnails`'s reqwest path, leaving the PDF with a broken
//! image. Data URL = no network, no expiry, no surprise. Source name
//! `"secondhand"` is in the trusted-curated set in
//! `src/bin/pumpfoil_report.rs`. Condition stays `New` so the report
//! bin's `Condition::New` filter doesn't drop the row.
use crate::listing::{Condition, Listing, Region};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use base64::Engine;
use chrono::Utc;
use reqwest::Client;

struct StaticAd {
    title: &'static str,
    brand: &'static str,
    url: &'static str,
    /// Raw JPEG bytes — `include_bytes!` makes this compile-time.
    image_bytes: &'static [u8],
    description: &'static str,
    location: &'static str,
    price: f64,
    currency: &'static str,
}

const ADS: &[StaticAd] = &[StaticAd {
    title: "🔁 GEBRAUCHT — Komplettes Pump Foil Set + Board Onix/MIO (CHF 1'350 – 1'450)",
    brand: "Onix/MIO (gebraucht)",
    url: "https://www.facebook.com/permalink.php?story_fbid=122102796939306040&id=1118089814717727",
    image_bytes: include_bytes!("../../../images/secondhand/onix-mio-set.jpg"),
    description: "Preis: CHF 1'350 – 1'450 (NP 2'400). \
        Komplettes Pump Foil Set von Onix/MIO bestehend aus: \
        Onix Osprey Front Wing 1850, MIO Board „Pump to the beat\" 92 cm, \
        Onix Mast Alu 80 cm, Onix Split-Fuselage 61 und 66 cm, \
        Onix Rear Wing 180 Glide und 220 Curve. \
        Zusätzlich: Osprey 2250 (NP 900) für 450.- extra. Alternativ Set \
        mit dem 2250 statt 1850 (+100.-). Onix und MIO werden in Europa \
        entwickelt und von Hand produziert. Alles in einem sehr guten \
        Zustand.",
    location: "Zürich",
    price: 1350.0,
    currency: "CHF",
}];

pub struct SecondHand {
    /// Pre-encoded data URLs (one per ad), built once at `new()` time
    /// so each `search()` call doesn't re-base64 the same bytes.
    image_urls: Vec<String>,
}

impl SecondHand {
    pub fn new(_client: Client) -> Self {
        let image_urls = ADS
            .iter()
            .map(|a| {
                let b64 = base64::engine::general_purpose::STANDARD.encode(a.image_bytes);
                format!("data:image/jpeg;base64,{}", b64)
            })
            .collect();
        Self { image_urls }
    }
}

#[async_trait]
impl Source for SecondHand {
    fn name(&self) -> &'static str {
        "secondhand"
    }
    fn region(&self) -> Region {
        Region::Ch
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let now = Utc::now();
        Ok(ADS
            .iter()
            .zip(self.image_urls.iter())
            .map(|(a, img)| Listing {
                source: "secondhand".to_string(),
                brand: Some(a.brand.to_string()),
                title: a.title.to_string(),
                url: a.url.to_string(),
                price: Some(a.price),
                currency: Some(a.currency.to_string()),
                condition: Condition::New, // see module docs
                available: Some(true),
                location: Some(a.location.to_string()),
                description: Some(a.description.to_string()),
                image: Some(img.clone()),
                region: Region::Ch,
                fetched_at: now,
            })
            .collect())
    }
}
