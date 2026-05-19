//! DigiKey Product Information API v4. Needs `DIGIKEY_CLIENT_ID` +
//! `DIGIKEY_CLIENT_SECRET` (OAuth2 client-credentials app at
//! developer.digikey.com). One token, then one keyword search per
//! part. Skips cleanly when creds are absent.

use super::{offer, SensorOffer};
use crate::sensors::Part;
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};

const TOKEN_URL: &str = "https://api.digikey.com/v1/oauth2/token";
const SEARCH_URL: &str = "https://api.digikey.com/products/v4/search/keyword";

pub async fn fetch(client: &Client, parts: &[Part]) -> Result<Vec<SensorOffer>> {
    let id = std::env::var("DIGIKEY_CLIENT_ID")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("skip: DIGIKEY_CLIENT_ID not set (.sensors.env)"))?;
    let secret = std::env::var("DIGIKEY_CLIENT_SECRET")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow!("skip: DIGIKEY_CLIENT_SECRET not set (.sensors.env)"))?;

    // OAuth2 client-credentials token.
    let tok: Value = client
        .post(TOKEN_URL)
        .form(&[
            ("client_id", id.as_str()),
            ("client_secret", secret.as_str()),
            ("grant_type", "client_credentials"),
        ])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let access = tok
        .get("access_token")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("digikey: no access_token in token response"))?
        .to_string();

    let mut out = Vec::new();
    for p in parts {
        let Some(mpn) = p.mpns.first() else { continue };
        let body = json!({ "Keywords": mpn, "Limit": 1, "Offset": 0 });
        let resp = client
            .post(SEARCH_URL)
            .bearer_auth(&access)
            .header("X-DIGIKEY-Client-Id", &id)
            .header("X-DIGIKEY-Locale-Site", "CH")
            .header("X-DIGIKEY-Locale-Currency", "CHF")
            .json(&body)
            .send()
            .await;
        let v: Value = match resp {
            Ok(r) => match r.error_for_status() {
                Ok(r) => r.json().await.unwrap_or(Value::Null),
                Err(e) => {
                    eprintln!("    digikey: {} HTTP {e}", p.key);
                    continue;
                }
            },
            Err(e) => {
                eprintln!("    digikey: {} {e}", p.key);
                continue;
            }
        };
        let Some(prod) = v
            .get("Products")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
        else {
            continue;
        };
        let price = prod.get("UnitPrice").and_then(Value::as_f64).filter(|x| *x > 0.0);
        let avail = prod
            .get("QuantityAvailable")
            .and_then(Value::as_i64)
            .map(|q| q > 0);
        let url = prod
            .get("ProductUrl")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://www.digikey.ch/en/products/result?keywords={mpn}"));
        let image = prod
            .get("PhotoUrl")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let title = format!("{} · {} @ DigiKey", p.name, mpn);
        out.push(offer(
            p,
            "digikey",
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
