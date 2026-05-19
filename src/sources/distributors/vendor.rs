//! Vendor **reference** source for parts whose only no-key lookup
//! path is a generic manufacturer page (`direct_url`) — u-blox,
//! Espressif, Raspberry Pi … These vendors don't sell direct at a
//! scrapeable price, so (like the ST reference source) we emit one
//! price-less reference offer per part pointing at the canonical
//! product page. This guarantees **every BOM part appears in the
//! catalog even with zero API keys configured**; the API distributors
//! fill in price/stock when keys are present.

use super::{offer, SensorOffer};
use crate::sensors::Part;
use anyhow::Result;
use reqwest::Client;

pub async fn fetch(_client: &Client, parts: &[Part]) -> Result<Vec<SensorOffer>> {
    let mut out = Vec::new();
    for p in parts.iter().filter(|p| p.direct_url.is_some()) {
        let url = p.direct_url.unwrap();
        let title = format!(
            "{} · {} @ {} (vendor page)",
            p.name,
            p.mpns.first().copied().unwrap_or(p.manufacturer),
            p.manufacturer
        );
        out.push(offer(
            p,
            "vendor",
            title,
            url.to_string(),
            None,
            None,
            None,
            None,
            Some("manufacturer page — price/stock via Mouser/DigiKey/Farnell".into()),
        ));
    }
    Ok(out)
}
