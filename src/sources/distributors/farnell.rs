//! Farnell / element14 Product Search API. Needs `FARNELL_API_KEY`
//! (register at partner.element14.com). One product search per part
//! against the Swiss store (`ch.farnell.com`, CHF). Skips cleanly when
//! the key is absent.

use super::{offer, SensorOffer};
use crate::sensors::Part;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

const ENDPOINT: &str = "https://api.element14.com/catalog/products";
const STORE: &str = "ch.farnell.com";

pub async fn fetch(client: &Client, parts: &[Part]) -> Result<Vec<SensorOffer>> {
    let key = std::env::var("FARNELL_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow!("skip: FARNELL_API_KEY not set (.sensors.env)"))?;

    let mut out = Vec::new();
    for p in parts {
        let Some(mpn) = p.mpns.first() else { continue };
        let resp = client
            .get(ENDPOINT)
            .query(&[
                ("term", format!("manuPartNum:{mpn}").as_str()),
                ("storeInfo.id", STORE),
                ("resultsSettings.offset", "0"),
                ("resultsSettings.numberOfResults", "1"),
                ("resultsSettings.responseGroup", "large"),
                ("callInfo.responseDataFormat", "json"),
                ("callInfo.apiKey", key.as_str()),
            ])
            .send()
            .await;
        let v: Value = match resp {
            Ok(r) => match r.error_for_status() {
                Ok(r) => r.json().await.unwrap_or(Value::Null),
                Err(e) => {
                    eprintln!("    farnell: {} HTTP {e}", p.key);
                    continue;
                }
            },
            Err(e) => {
                eprintln!("    farnell: {} {e}", p.key);
                continue;
            }
        };
        // The response key differs by search type; manuPartNum search
        // returns `manufacturerPartNumberSearchReturn`.
        let products = v
            .get("manufacturerPartNumberSearchReturn")
            .or_else(|| v.get("keywordSearchReturn"))
            .and_then(|r| r.get("products"))
            .and_then(Value::as_array);
        let Some(prod) = products.and_then(|a| a.first()) else {
            continue;
        };
        let price = prod
            .get("prices")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|b| b.get("cost"))
            .and_then(|c| c.as_f64().or_else(|| c.as_str().and_then(|s| s.parse().ok())));
        let sku = prod.get("sku").and_then(Value::as_str).unwrap_or("");
        let url = if sku.is_empty() {
            format!("https://{STORE}/w/search?st={mpn}")
        } else {
            format!("https://{STORE}/{sku}")
        };
        let image = prod
            .get("image")
            .and_then(|im| im.get("baseName"))
            .and_then(Value::as_str)
            .map(|b| format!("https://{STORE}/productimages/standard/en_GB/{b}"));
        let avail = prod
            .get("stock")
            .and_then(|s| s.get("level"))
            .and_then(Value::as_i64)
            .map(|l| l > 0);
        let title = format!("{} · {} @ Farnell", p.name, mpn);
        out.push(offer(
            p,
            "farnell",
            title,
            url,
            price,
            Some("CHF".into()),
            avail,
            image,
            None,
        ));
    }
    Ok(out)
}
