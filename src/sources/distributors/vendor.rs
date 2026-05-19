//! Vendor source for parts whose only no-key lookup path is a generic
//! manufacturer page (`direct_url`) — u-blox, Espressif, Seeed Studio …
//!
//! **Best-effort fetch:** unlike st.com (Akamai-blocked, see `st.rs`),
//! these vendor pages are normal sites, so we `fetch_page_product`
//! them to recover `og:image` (and a price/title where the page ships
//! JSON-LD — Seeed sells direct). If the fetch fails or yields no
//! image we still emit a price-less **reference** offer pointing at
//! the canonical page, so the part never disappears from the catalog.
//! A guaranteed placeholder image is applied downstream in
//! `sensor_report` when no real photo was found — every card always
//! shows an image.

use super::{offer, SensorOffer};
use crate::sensors::Part;
use crate::sources::html_util::fetch_page_product;
use anyhow::Result;
use reqwest::Client;

pub async fn fetch(client: &Client, parts: &[Part]) -> Result<Vec<SensorOffer>> {
    let mut out = Vec::new();
    for p in parts.iter().filter(|p| p.direct_url.is_some()) {
        let url = p.direct_url.unwrap();
        let mpn = p.mpns.first().copied().unwrap_or(p.manufacturer);
        // Best-effort — never let one slow/blocked vendor page drop
        // the part from the catalog.
        let pp = fetch_page_product(client, url).await.ok();
        let image = pp.as_ref().and_then(|x| x.image.clone());
        let price = pp.as_ref().and_then(|x| x.price);
        let currency = pp.as_ref().and_then(|x| x.currency.clone());
        let available = pp.as_ref().and_then(|x| x.available);
        let suffix = if image.is_some() {
            "vendor page"
        } else {
            "vendor page (reference)"
        };
        let title = format!("{} · {mpn} @ {} ({suffix})", p.name, p.manufacturer);
        out.push(offer(
            p,
            "vendor",
            title,
            url.to_string(),
            price,
            currency,
            available,
            image,
            Some("manufacturer page — price/stock also via Mouser/DigiKey/Farnell".into()),
        ));
    }
    Ok(out)
}
