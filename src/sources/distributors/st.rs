//! ST (st.com) **reference** source. st.com sits behind aggressive
//! bot protection (Akamai) that hangs/blocks a plain reqwest GET —
//! scraping it cost ~30 s/part of dead timeout and yielded nothing.
//!
//! ST's eStore never exposes a price without auth anyway, so the only
//! thing the manufacturer page added was a canonical link. We emit
//! that **without any HTTP fetch**: one price-less reference offer per
//! ST part pointing at its st.com product page. The actual price /
//! stock / image for ST silicon comes from the API distributors
//! (Mouser / DigiKey / Farnell), which carry every ST MPN in the BOM.

use super::{offer, SensorOffer};
use crate::sensors::Part;
use anyhow::Result;
use reqwest::Client;

pub async fn fetch(_client: &Client, parts: &[Part]) -> Result<Vec<SensorOffer>> {
    let mut out = Vec::new();
    for p in parts.iter().filter(|p| p.st_url.is_some()) {
        let url = p.st_url.unwrap();
        let title = format!(
            "{} · {} @ st.com (manufacturer page)",
            p.name,
            p.mpns.first().copied().unwrap_or(p.manufacturer)
        );
        out.push(offer(
            p,
            "st",
            title,
            url.to_string(),
            None,
            None,
            None,
            None,
            Some("canonical ST page — price/stock via Mouser/DigiKey/Farnell".into()),
        ));
    }
    Ok(out)
}
