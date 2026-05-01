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

/// One row from a sitemap: the canonical URL plus any sitemap-image
/// titles associated with it. Magento-style sitemaps use SKU-only slugs
/// (`/de_ch/3615sq-3615sq.html`) and stuff the human-readable product
/// name into `<image:title>` instead, so we keep those for downstream
/// keyword filtering.
#[derive(Debug, Default, Clone)]
pub struct SitemapEntry {
    pub loc: String,
    pub titles: Vec<String>,
}

/// Fetch every `<loc>` URL reachable from a sitemap, transparently
/// following `<sitemapindex>` references one level deep.
pub async fn fetch_sitemap_urls(client: &Client, entry: &str) -> Result<Vec<String>> {
    Ok(fetch_sitemap_entries(client, entry)
        .await?
        .into_iter()
        .map(|e| e.loc)
        .collect())
}

/// Same traversal as [`fetch_sitemap_urls`] but also captures the
/// `<image:title>` text inside each `<url>` block.
pub async fn fetch_sitemap_entries(
    client: &Client,
    entry: &str,
) -> Result<Vec<SitemapEntry>> {
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
        if body.contains("<sitemapindex") {
            // Sitemap of sitemaps — only need the inner <loc>s for traversal.
            for cap in body.split("<loc>").skip(1) {
                if let Some(end) = cap.find("</loc>") {
                    queue.push(cap[..end].trim().to_string());
                }
            }
            continue;
        }
        // Per-url sitemap: walk each <url>...</url> block so we can pair
        // <loc> with its sibling <image:title> values.
        for block in body.split("<url>").skip(1) {
            let Some(end) = block.find("</url>") else {
                continue;
            };
            let inner = &block[..end];
            let loc = match inner.split_once("<loc>") {
                Some((_, rest)) => match rest.split_once("</loc>") {
                    Some((l, _)) => l.trim().to_string(),
                    None => continue,
                },
                None => continue,
            };
            let titles = inner
                .split("<image:title>")
                .skip(1)
                .filter_map(|c| c.split_once("</image:title>").map(|(t, _)| t.trim().to_string()))
                .collect();
            collected.push(SitemapEntry { loc, titles });
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
        // Some shops (Alpinefoil, observed) ship JSON-LD with raw \r\n
        // inside string values, which serde_json strict-rejects. Replace
        // unescaped control chars with spaces — safe both inside strings
        // (just collapses whitespace) and between tokens.
        let sanitized: String = text
            .trim()
            .chars()
            .map(|c| if c.is_control() && c != '\n' { ' ' } else { c })
            .collect();
        let sanitized = sanitized.replace('\n', " ");
        if let Ok(v) = serde_json::from_str::<Value>(&sanitized) {
            extract_jsonld(&v, &mut pp);
        }
    }

    // OpenGraph / product meta fallback.
    if pp.title.is_none() {
        pp.title = meta_content(&doc, "og:title").as_deref().map(clean_html_text);
    }
    if pp.description.is_none() {
        pp.description = meta_content(&doc, "og:description")
            .as_deref()
            .map(clean_html_text);
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
                pp.title = obj
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(clean_html_text);
            }
            if pp.description.is_none() {
                pp.description = obj
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(clean_html_text);
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
                        // schema.org/AggregateOffer carries `lowPrice`/
                        // `highPrice` instead of `price` — fall back so
                        // shops like Ketos / Alpinefoil that bundle
                        // configurable pumpfoil packs still report a
                        // number to sort by.
                        pp.price = o
                            .get("price")
                            .or_else(|| o.get("lowPrice"))
                            .and_then(|p| match p {
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

/// Decode HTML entities and strip tags from a JSON-LD string by
/// re-parsing it as HTML and taking the text content. Runs twice to
/// undo double-encoding seen in the wild (Alpinefoil ships
/// `&amp;ccedil;u` instead of `&ccedil;u` for `çu`; Indiana ships
/// `&#039;` inside the JSON name field). The second pass is also what
/// turns `&lt;p&gt;hello&lt;/p&gt;` into `hello` rather than the
/// literal `<p>hello</p>`.
pub fn clean_html_text(s: &str) -> String {
    fn pass(s: &str) -> String {
        Html::parse_fragment(s)
            .root_element()
            .text()
            .collect::<String>()
    }
    pass(&pass(s))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strict pumpfoil keyword test — for narrowing brand-shop catalogs to
/// the actual pumpfoil/dockstart discipline rather than wing/kite/wake
/// adjacencies. Different brands spell it differently:
/// - "pumpfoil" / "pump foil" / "pump-foil" (Indiana, Alpinefoil, Duotone)
/// - "pumping" (Ketos, Alpinefoil category)
/// - "foil pumping" / "foilpump" (Ketos category, marketing copy)
/// - "dockstart" (Alpinefoil, Indiana — same discipline, different verb)
pub fn looks_like_pump_foil(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("pumpfoil")
        || t.contains("pump foil")
        || t.contains("pump-foil")
        || t.contains("pump_foil")
        || t.contains("foilpump")
        || t.contains("foil pumping")
        || t.contains("foil-pumping")
        || t.contains("pumping")
        || t.contains("dockstart")
        || t.contains("dock start")
        || t.contains("dock-start")
}
