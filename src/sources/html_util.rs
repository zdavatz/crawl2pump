//! Tiny shared helpers for HTML/sitemap-based brand scrapers.
//!
//! Strategy: hit `/sitemap.xml` to enumerate product URLs, then for each
//! candidate URL fetch the page and parse JSON-LD `Product` (plus OpenGraph
//! fallback) to extract title / price / image. This is far more robust than
//! guessing CSS class names that brands rename quarterly.
use anyhow::{Context, Result};
use reqwest::Client;
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::HashSet;

/// Fetch every `<loc>` URL reachable from a sitemap, transparently
/// following `<sitemapindex>` references one level deep.
pub async fn fetch_sitemap_urls(client: &Client, entry: &str) -> Result<Vec<String>> {
    let mut collected = Vec::new();
    let mut queue = vec![entry.to_string()];
    let mut seen: HashSet<String> = HashSet::new();
    while let Some(u) = queue.pop() {
        if !seen.insert(u.clone()) {
            continue;
        }
        if seen.len() > 40 {
            break;
        }
        let resp = match client.get(&u).send().await {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !resp.status().is_success() {
            continue;
        }
        let body = match resp.text().await {
            Ok(b) => b,
            Err(_) => continue,
        };
        let is_index = body.contains("<sitemapindex");
        for cap in body.split("<loc>").skip(1) {
            if let Some(end) = cap.find("</loc>") {
                let s = cap[..end].trim().to_string();
                if is_index {
                    queue.push(s);
                } else {
                    collected.push(s);
                }
            }
        }
    }
    Ok(collected)
}

#[derive(Debug, Default, Clone)]
pub struct PageProduct {
    pub title: Option<String>,
    pub description: Option<String>,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub image: Option<String>,
    pub available: Option<bool>,
}

pub async fn fetch_page_product(client: &Client, url: &str) -> Result<PageProduct> {
    let html_text = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()?
        .text()
        .await?;
    Ok(parse_page_product(&html_text))
}

pub fn parse_page_product(html_text: &str) -> PageProduct {
    let doc = Html::parse_document(html_text);
    let mut pp = PageProduct::default();

    // JSON-LD Product preferred — most ecommerce platforms emit it.
    let script_sel = Selector::parse(r#"script[type="application/ld+json"]"#).unwrap();
    for s in doc.select(&script_sel) {
        let text: String = s.text().collect();
        if let Ok(v) = serde_json::from_str::<Value>(text.trim()) {
            extract_jsonld(&v, &mut pp);
        }
    }

    // OpenGraph / product meta fallback.
    if pp.title.is_none() {
        pp.title = meta_content(&doc, "og:title");
    }
    if pp.description.is_none() {
        pp.description = meta_content(&doc, "og:description");
    }
    if pp.image.is_none() {
        pp.image = meta_content(&doc, "og:image");
    }
    if pp.price.is_none() {
        pp.price = meta_content(&doc, "product:price:amount").and_then(|s| s.parse().ok());
    }
    if pp.currency.is_none() {
        pp.currency = meta_content(&doc, "product:price:currency");
    }
    pp
}

fn extract_jsonld(v: &Value, pp: &mut PageProduct) {
    match v {
        Value::Array(arr) => {
            for item in arr {
                extract_jsonld(item, pp);
            }
        }
        Value::Object(obj) => {
            if let Some(graph) = obj.get("@graph") {
                extract_jsonld(graph, pp);
            }
            let ty = obj.get("@type");
            let is_product = match ty {
                Some(Value::String(s)) => s.eq_ignore_ascii_case("Product"),
                Some(Value::Array(a)) => a
                    .iter()
                    .any(|x| x.as_str().is_some_and(|s| s.eq_ignore_ascii_case("Product"))),
                _ => false,
            };
            if !is_product {
                return;
            }
            if pp.title.is_none() {
                pp.title = obj.get("name").and_then(|v| v.as_str()).map(str::to_string);
            }
            if pp.description.is_none() {
                pp.description = obj
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            if pp.image.is_none() {
                pp.image = match obj.get("image") {
                    Some(Value::String(s)) => Some(s.clone()),
                    Some(Value::Array(a)) => a.first().and_then(|x| x.as_str().map(str::to_string)),
                    Some(Value::Object(o)) => o.get("url").and_then(|v| v.as_str()).map(str::to_string),
                    _ => None,
                };
            }
            if let Some(offers) = obj.get("offers") {
                let offer = match offers {
                    Value::Array(a) => a.first(),
                    _ => Some(offers),
                };
                if let Some(o) = offer.and_then(|v| v.as_object()) {
                    if pp.price.is_none() {
                        pp.price = o.get("price").and_then(|p| match p {
                            Value::String(s) => s.parse().ok(),
                            Value::Number(n) => n.as_f64(),
                            _ => None,
                        });
                    }
                    if pp.currency.is_none() {
                        pp.currency = o
                            .get("priceCurrency")
                            .and_then(|v| v.as_str())
                            .map(str::to_string);
                    }
                    if pp.available.is_none() {
                        pp.available = o
                            .get("availability")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_lowercase().contains("instock"));
                    }
                }
            }
        }
        _ => {}
    }
}

fn meta_content(doc: &Html, property: &str) -> Option<String> {
    for attr in ["property", "name"] {
        let sel_str = format!(r#"meta[{}="{}"]"#, attr, property);
        let Ok(sel) = Selector::parse(&sel_str) else {
            continue;
        };
        if let Some(el) = doc.select(&sel).next() {
            if let Some(c) = el.value().attr("content") {
                return Some(c.to_string());
            }
        }
    }
    None
}

/// URL-keyword filter: keep URLs that look like foil-gear product pages.
pub fn looks_like_foil_product(url: &str) -> bool {
    let u = url.to_lowercase();
    const KW: &[&str] = &[
        "foil", "pump", "mast", "fuselage", "front-wing", "stab", "wing",
        "kit", "package", "set", "board",
    ];
    KW.iter().any(|k| u.contains(k))
}
