//! Ketos — ketos-foil.com — French foil maker (carbon foils since 2009).
//!
//! WordPress with WooCommerce. The English shop lives under `/en/...`
//! with category slugs `pumping-en`, `pumping-board`, `pumping-front-wing`,
//! `pumping-packs`. We restrict to `/en/` to keep titles in English and
//! narrow with `looks_like_pump_foil`.
//!
//! ### WooCommerce variant explosion
//!
//! Some Ketos products (e.g. the Kobun front-wing line) ship with size
//! variants encoded in a `data-product_variations` JSON blob plus a
//! per-size spec table. We detect both, parse the table by column header,
//! match each variant to a row by the first contiguous digit run in the
//! variant attribute (e.g. "コブン 99" → "99" → row "KOBUN コブン 99"),
//! and emit one `Listing` per size with the spec line baked into the
//! description so the downstream `extract_from_text` enricher picks it up
//! without a second HTTP fetch. Products without a spec table or without
//! variants fall through to the original single-Listing path.
use crate::listing::{Condition, Listing, Region};
use crate::sources::html_util::{
    fetch_sitemap_urls, looks_like_front_wing, looks_like_pump_foil, parse_page_product,
};
use crate::sources::Source;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use futures::stream::{self, StreamExt};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use std::collections::HashMap;
use std::sync::OnceLock;
use url::form_urlencoded;

const SITEMAP: &str = "https://www.ketos-foil.com/product-sitemap.xml";
const BRAND: &str = "Ketos";
const CONCURRENCY: usize = 6;
const MAX_PRODUCTS: usize = 60;

pub struct Ketos {
    client: Client,
}

impl Ketos {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Source for Ketos {
    fn name(&self) -> &'static str {
        "ketos"
    }
    fn region(&self) -> Region {
        Region::World
    }
    async fn search(&self, _query: &str) -> Result<Vec<Listing>> {
        let urls = fetch_sitemap_urls(&self.client, SITEMAP).await?;
        let candidates: Vec<String> = urls
            .into_iter()
            .filter(|u| !u.ends_with(".xml"))
            // Skip t-shirts, screws, and the FR mirror — keep English shop only.
            .filter(|u| u.contains("/en/"))
            .filter(|u| !u.contains("t-shirt") && !u.contains("/screws-"))
            .filter(|u| looks_like_pump_foil(u) || looks_like_front_wing(u))
            .take(MAX_PRODUCTS)
            .collect();

        let client = &self.client;
        let listings: Vec<Listing> = stream::iter(candidates)
            .map(|url| async move {
                fetch_product(client, &url).await.unwrap_or_default()
            })
            .buffer_unordered(CONCURRENCY)
            .flat_map(stream::iter)
            .collect()
            .await;

        Ok(listings)
    }
}

async fn fetch_product(client: &Client, url: &str) -> Result<Vec<Listing>> {
    let html = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(build_listings(url, &html))
}

fn build_listings(url: &str, html: &str) -> Vec<Listing> {
    let pp = parse_page_product(html);
    let Some(base_title) = pp.title.clone() else {
        return Vec::new();
    };

    let variants = parse_wc_variants(html);
    let specs = parse_wing_spec_table(html);

    // Explode WC variants into per-variant Listings, capped at 8 to suppress
    // board configurators (e.g. the KOBUN DW 85 board has 24 finish×size
    // combinations — exploding those would just clutter the catalog). At ≤8
    // we comfortably cover front-wing size sets (Kobun: 4) and modular kit
    // options (Split: 5×CORE/TIPS bundles). Specs are attached per variant
    // only where `size_key` matches a row in the spec table.
    let should_explode = !variants.is_empty() && variants.len() <= 8;

    if !should_explode {
        return vec![Listing {
            source: "ketos".to_string(),
            brand: Some(BRAND.to_string()),
            title: base_title,
            url: url.to_string(),
            price: pp.price,
            currency: pp.currency.or_else(|| Some("EUR".to_string())),
            condition: Condition::New,
            available: pp.available,
            location: Some("France".to_string()),
            description: pp.description,
            image: pp.image,
            region: Region::World,
            fetched_at: Utc::now(),
        }];
    }

    variants
        .iter()
        .map(|v| {
            let var_url = build_variant_url(url, v);
            let title = format!("{base_title} — {}", v.label);
            let mut desc = pp.description.clone().unwrap_or_default();
            if let Some(s) = specs.get(&v.size_key) {
                desc.push_str("\n\n");
                if let Some(a) = s.area_cm2 {
                    desc.push_str(&format!("Surface area: {} cm² ", fmt_num(a)));
                }
                if let Some(sp) = s.span_mm {
                    desc.push_str(&format!("Wingspan: {} mm ", sp as i64));
                }
                if let Some(ar) = s.aspect_ratio {
                    desc.push_str(&format!("Aspect ratio: {} ", fmt_num(ar)));
                }
                if let Some(ch) = s.chord_mm {
                    desc.push_str(&format!("Chord: {} mm", ch as i64));
                }
            }
            Listing {
                source: "ketos".to_string(),
                brand: Some(BRAND.to_string()),
                title,
                url: var_url,
                price: v.price.or(pp.price),
                currency: pp.currency.clone().or_else(|| Some("EUR".to_string())),
                condition: Condition::New,
                available: pp.available,
                location: Some("France".to_string()),
                description: Some(desc),
                image: v.image.clone().or_else(|| pp.image.clone()),
                region: Region::World,
                fetched_at: Utc::now(),
            }
        })
        .collect()
}

fn build_variant_url(url: &str, v: &WcVariant) -> String {
    let mut q = form_urlencoded::Serializer::new(String::new());
    for (k, val) in &v.attrs {
        q.append_pair(k, val);
    }
    let qs = q.finish();
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}{qs}")
}

fn fmt_num(n: f64) -> String {
    if (n - n.round()).abs() < 0.005 {
        format!("{}", n.round() as i64)
    } else {
        format!("{:.2}", n).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[derive(Debug, Default)]
struct WcVariant {
    label: String,
    size_key: String,
    price: Option<f64>,
    image: Option<String>,
    attrs: Vec<(String, String)>,
}

fn parse_wc_variants(html: &str) -> Vec<WcVariant> {
    let needle = "data-product_variations=\"";
    let Some(start) = html.find(needle) else {
        return Vec::new();
    };
    let after = &html[start + needle.len()..];
    let Some(end) = after.find('"') else {
        return Vec::new();
    };
    let raw = &after[..end];
    let unescaped = raw
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    let v: serde_json::Value = match serde_json::from_str(&unescaped) {
        Ok(v) => v,
        _ => return Vec::new(),
    };
    let Some(arr) = v.as_array() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for item in arr {
        let attrs_obj = item
            .get("attributes")
            .and_then(|x| x.as_object())
            .cloned()
            .unwrap_or_default();
        let mut attrs: Vec<(String, String)> = Vec::new();
        for (k, val) in &attrs_obj {
            if let Some(s) = val.as_str() {
                attrs.push((k.clone(), s.trim().to_string()));
            }
        }
        let label = attrs
            .first()
            .map(|(_, v)| v.clone())
            .unwrap_or_default();
        if label.is_empty() {
            continue;
        }
        let size_key = first_digit_run(&label).unwrap_or_default();
        let price = item
            .get("display_price")
            .and_then(|x| x.as_f64().or_else(|| x.as_u64().map(|n| n as f64)));
        let image = item
            .get("image")
            .and_then(|x| x.get("url"))
            .and_then(|x| x.as_str())
            .map(str::to_string);
        out.push(WcVariant {
            label,
            size_key,
            price,
            image,
            attrs,
        });
    }
    out
}

fn first_digit_run(s: &str) -> Option<String> {
    let mut started = false;
    let mut buf = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            buf.push(c);
            started = true;
        } else if started {
            break;
        }
    }
    if buf.is_empty() {
        None
    } else {
        Some(buf)
    }
}

#[derive(Debug, Default, Clone)]
struct WingSpecRow {
    area_cm2: Option<f64>,
    span_mm: Option<f64>,
    aspect_ratio: Option<f64>,
    chord_mm: Option<f64>,
}

/// Walk every `<table>` looking for one whose header row labels columns as
/// Surface / WingSpan / AR / Chord, then yield `model_key → spec_row` for
/// data rows where the first cell carries a digit run we can use to match
/// up with a variant's size key.
fn parse_wing_spec_table(html: &str) -> HashMap<String, WingSpecRow> {
    let mut out = HashMap::new();
    let doc = Html::parse_document(html);
    let table_sel = Selector::parse("table").unwrap();
    let row_sel = Selector::parse("tr").unwrap();
    let cell_sel = Selector::parse("th, td").unwrap();

    for table in doc.select(&table_sel) {
        let mut col_kind: Vec<&'static str> = Vec::new();
        let mut found_header = false;
        let mut local: HashMap<String, WingSpecRow> = HashMap::new();
        for tr in table.select(&row_sel) {
            let cells: Vec<String> = tr
                .select(&cell_sel)
                .map(|c| c.text().collect::<String>().trim().to_string())
                .collect();
            if cells.is_empty() {
                continue;
            }
            if !found_header {
                col_kind.clear();
                let mut has_surface = false;
                for cell in &cells {
                    let lc = cell.to_lowercase();
                    let kind = if lc.contains("surface") || lc.contains("area") || lc.contains("aire") {
                        has_surface = true;
                        "area"
                    } else if lc.contains("wingspan") || lc.contains("envergure")
                        || (lc.contains("span") && !lc.contains("aspect"))
                    {
                        "span"
                    } else if lc == "ar" || lc.contains("aspect") {
                        "ar"
                    } else if lc.contains("chord") {
                        "chord"
                    } else {
                        ""
                    };
                    col_kind.push(kind);
                }
                if has_surface {
                    found_header = true;
                }
                continue;
            }
            let Some(model_key) = first_digit_run(&cells[0]) else {
                continue;
            };
            let mut row = WingSpecRow::default();
            for (idx, cell) in cells.iter().enumerate().skip(1) {
                if idx >= col_kind.len() {
                    break;
                }
                let Some(n) = first_number_in(cell) else {
                    continue;
                };
                match col_kind[idx] {
                    "area" if (200.0..=2700.0).contains(&n) => row.area_cm2 = Some(n),
                    // WingSpan column is typically in cm on Ketos pages
                    // (e.g. 99 means 990 mm). Multiply if the value is in
                    // the cm range; otherwise treat as mm.
                    "span" if (30.0..=250.0).contains(&n) => row.span_mm = Some(n * 10.0),
                    "span" if (300.0..=2500.0).contains(&n) => row.span_mm = Some(n),
                    "ar" if (3.0..=15.0).contains(&n) => row.aspect_ratio = Some(n),
                    "chord" if (50.0..=400.0).contains(&n) => row.chord_mm = Some(n),
                    _ => {}
                }
            }
            if row.area_cm2.is_some() || row.span_mm.is_some() {
                local.insert(model_key, row);
            }
        }
        if !local.is_empty() {
            out.extend(local);
            return out;
        }
    }
    out
}

fn first_number_in(s: &str) -> Option<f64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\d+(?:[.,]\d+)?").unwrap())
        .find(s)
        .and_then(|m| m.as_str().replace(',', ".").parse().ok())
}
