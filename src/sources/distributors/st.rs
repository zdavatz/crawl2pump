//! ST source. The `www.st.com` product *page* is Akamai-blocked
//! (a plain GET hangs ~30 s and returns nothing — never re-add an
//! HTML fetch there). But ST's Magento eStore image CDN,
//! `estore.st.com/media/catalog/product/<c1>/<c2>/<key>.jpg`, **is**
//! reachable without auth, so we pull the real product photo from
//! there.
//!
//! Most bare sensor ICs have no eStore photo — ST serves a single
//! shared placeholder PNG for those (identical SHA-256). We detect
//! that hash and treat it as "no image" so the downstream
//! `sensor_report` placeholder fires instead of showing ST's blank
//! tile. The eval board (STEVAL-MKBOXPRO) and a few ICs (e.g.
//! LIS2MDL) do have real photos and come through.
//!
//! Price/stock for ST silicon still comes from the API distributors
//! (Mouser/DigiKey/Farnell) — the eStore needs auth for pricing — so
//! these stay price-less reference offers, just *with an image*.

use super::{offer, SensorOffer};
use crate::sensors::Part;
use anyhow::Result;
use reqwest::Client;
use sha2::{Digest, Sha256};

/// SHA-256 of ST's shared "no product image" eStore placeholder PNG
/// (5830 bytes). Captured 2026-05; if ST swaps their placeholder this
/// just means a few ST ICs show the real ST blank instead of ours —
/// re-capture with
/// `curl -s estore.st.com/media/catalog/product/s/t/stc3115.jpg | shasum -a256`.
const ST_PLACEHOLDER_SHA: &str =
    "a571126f0422c134e52b04e21ba68de155100beb90a943f43511676939f28edb";

fn estore_image_url(part: &Part) -> Option<String> {
    // The eStore url-key is the generic base part (shortest MPN),
    // lowercased — e.g. "LSM6DSV16XTR"/"LSM6DSV16X" → "lsm6dsv16x",
    // "STEVAL-MKBOXPRO" → "steval-mkboxpro".
    let key = part.mpns.iter().min_by_key(|m| m.len())?.to_lowercase();
    let mut ch = key.chars();
    let c1 = ch.next()?;
    let c2 = ch.next()?;
    Some(format!(
        "https://estore.st.com/media/catalog/product/{c1}/{c2}/{key}.jpg"
    ))
}

pub async fn fetch(client: &Client, parts: &[Part]) -> Result<Vec<SensorOffer>> {
    let mut out = Vec::new();
    for p in parts.iter().filter(|p| p.st_url.is_some()) {
        let page = p.st_url.unwrap();
        let mpn = p.mpns.first().copied().unwrap_or(p.manufacturer);

        // Best-effort real photo from the eStore image CDN.
        let mut image = None;
        if let Some(img_url) = estore_image_url(p) {
            if let Ok(resp) = client.get(&img_url).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        let sha = hex(&Sha256::digest(&bytes));
                        if sha != ST_PLACEHOLDER_SHA && bytes.len() > 1000 {
                            image = Some(img_url);
                        }
                    }
                }
            }
        }

        let suffix = if image.is_some() {
            "st.com"
        } else {
            "st.com (reference)"
        };
        let title = format!("{} · {mpn} @ {suffix}", p.name);
        out.push(offer(
            p,
            "st",
            title,
            page.to_string(),
            None,
            None,
            None,
            image,
            Some("canonical ST page — price/stock via Mouser/DigiKey/Farnell".into()),
        ));
    }
    Ok(out)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}
