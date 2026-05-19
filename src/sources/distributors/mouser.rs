//! Mouser Search API v1. Needs `MOUSER_API_KEY` (free, register at
//! mouser.com/api-hub). One `search/partnumber` POST per part (first
//! MPN). Skips cleanly when the key is absent.

use super::{offer, SensorOffer};
use crate::sensors::Part;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};

const ENDPOINT: &str = "https://api.mouser.com/api/v1/search/partnumber";

pub async fn fetch(client: &Client, parts: &[Part]) -> Result<Vec<SensorOffer>> {
    let key = std::env::var("MOUSER_API_KEY")
        .ok()
        .filter(|k| !k.trim().is_empty())
        .ok_or_else(|| anyhow!("skip: MOUSER_API_KEY not set (.sensors.env)"))?;

    let mut out = Vec::new();
    for p in parts {
        let Some(mpn) = p.mpns.first() else { continue };
        let body = json!({
            "SearchByPartRequest": { "mouserPartNumber": mpn }
        });
        let resp = client
            .post(ENDPOINT)
            .query(&[("apiKey", key.as_str())])
            .json(&body)
            .send()
            .await;
        let v: Value = match resp {
            Ok(r) => match r.error_for_status() {
                Ok(r) => r.json().await.unwrap_or(Value::Null),
                Err(e) => {
                    eprintln!("    mouser: {} HTTP {e}", p.key);
                    continue;
                }
            },
            Err(e) => {
                eprintln!("    mouser: {} {e}", p.key);
                continue;
            }
        };
        let Some(part) = v
            .pointer("/SearchResults/Parts")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
        else {
            continue;
        };
        let (price, currency) = first_price_break(part);
        let avail = part
            .get("Availability")
            .and_then(Value::as_str)
            .map(|s| !s.starts_with('0') && !s.eq_ignore_ascii_case("none"));
        let url = part
            .get("ProductDetailUrl")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://www.mouser.com/c/?q={mpn}"));
        let image = part
            .get("ImagePath")
            .and_then(Value::as_str)
            .map(str::to_string);
        let title = format!("{} · {} @ Mouser", p.name, mpn);
        out.push(offer(
            p, "mouser", title, url, price, currency, avail, image, None,
        ));
    }
    Ok(out)
}

/// Mouser `PriceBreaks` is qty-tiered; take the lowest-quantity break
/// (the single-unit price) and normalise the `"1,23 €"`-style string.
fn first_price_break(part: &Value) -> (Option<f64>, Option<String>) {
    let Some(pb) = part
        .get("PriceBreaks")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    else {
        return (None, None);
    };
    let currency = pb
        .get("Currency")
        .and_then(Value::as_str)
        .map(str::to_string);
    let price = pb.get("Price").and_then(Value::as_str).and_then(|s| {
        let cleaned: String = s
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
            .collect();
        // European decimal comma → dot; drop thousands separators.
        let norm = if cleaned.matches(',').count() == 1 && !cleaned.contains('.') {
            cleaned.replace(',', ".")
        } else {
            cleaned.replace(',', "")
        };
        norm.parse::<f64>().ok()
    });
    (price, currency)
}
