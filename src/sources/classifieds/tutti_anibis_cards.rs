//! Shared listing-card extractor for Tutti.ch and Anibis.ch.
//!
//! The two sites run identical Next.js frontends (same employer) and render
//! each listing inside a `div[data-private-srp-listing-item-id="<id>"]`
//! container. The URL/title/price/thumbnail live at predictable positions
//! inside that container.
//!
//! ### Category tokens
//!
//! The `/de/q/suche/<token>` URL carries an msgpack-ish base64url-encoded
//! filter blob. Freetext queries aren't accepted (silently dropped), but the
//! token for *category* navigation turns out to embed the slug in plain
//! base64 — e.g. `Ak8Cuc3BvcnRzT3V0ZG9vcnOUwMDAwA` decodes to a frame
//! containing the literal string `sportsOutdoors`. We use this to scrape
//! sport-specific recent listings (~30 per category) alongside the
//! generic all-recent feed, giving ~3x the foil hitrate.
//!
//! DOM:
//!
//! ```text
//!   <div data-private-srp-listing-item-id="ID">
//!     <a href="/de/vi/.../ID">
//!       <img src="https://c.{tutti,anibis}.ch/thumbnail/...jpg" alt="...">
//!     </a>
//!     <h2><a href="/de/vi/...">Title</a></h2>
//!     <span>Description snippet…</span>
//!     <div><span>1'499.-</span></div>   ← price is a leaf <span> inside a div
//!   </div>
//! ```
//!
//! The search URL on both sites **ignores free-text queries** (they use an
//! opaque binary token for query state), so the caller must filter
//! client-side — this module exposes `matches_query` for that.
use super::{absolute, find_price_in_subtree, parse_swiss_price};
use crate::listing::{Condition, Listing, Region};
use chrono::Utc;
use scraper::{Html, Selector};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

static LISTING_NODE_RE: OnceLock<regex::Regex> = OnceLock::new();

/// Base64-msgpack state tokens for the `/de/q/suche/<token>` URL. Stable
/// across Next.js builds as of Apr 2026 — the category slug is literally
/// base64-encoded inside the blob, so these are trivially re-derivable.
///
/// Order matters: sportsOutdoors comes first because it's the highest-
/// signal category for foils, and we dedupe by URL at the end so earlier
/// entries "win" when the same listing shows up under multiple filters.
pub const CATEGORY_TOKENS: &[(&str, &str)] = &[
    ("sportsOutdoors", "Ak8Cuc3BvcnRzT3V0ZG9vcnOUwMDAwA"),
    ("otherSports", "Ak8Crb3RoZXJTcG9ydHOUwMDAwA"),
    ("boats", "Ak8ClYm9hdHOUwMDAwA"),
    ("accessories", "Ak8CrYWNjZXNzb3JpZXOUwMDAwA"),
    // Fallback: the generic all-categories feed (still useful when a foil
    // is listed under an unexpected category like "others").
    ("all", "Ak8DAlMDAwMA"),
];

pub struct Extracted {
    pub url: String,
    pub title: String,
    pub price: Option<f64>,
    pub image: Option<String>,
    pub body: String,
}

/// Parse the Next.js SSR dehydrated state in the page. For each `"node":{…}`
/// entry we pair the `listingID` with its `thumbnail.normalRendition.src`.
/// This is the only reliable source of image URLs below the fold — the
/// rendered `<img>` for most cards is a `data:image/gif…` placeholder that
/// only gets swapped for the real URL after client-side hydration, and
/// Anibis doesn't even emit a `<noscript>` fallback with the real URL.
fn extract_image_map(html: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let re = LISTING_NODE_RE.get_or_init(|| {
        regex::Regex::new(
            r#""listingID":"(\d+)"[\s\S]{0,4000}?"normalRendition":\{"src":"([^"]+)""#,
        )
        .unwrap()
    });
    for cap in re.captures_iter(html) {
        let id = cap[1].to_string();
        let url = cap[2].to_string();
        out.entry(id).or_insert(url);
    }
    out
}

pub fn parse_cards(html: &str, origin: &str) -> Vec<Extracted> {
    let image_map = extract_image_map(html);

    let doc = Html::parse_document(html);
    let card_sel = Selector::parse("div[data-private-srp-listing-item-id]").unwrap();
    let link_sel = Selector::parse(r#"a[href*="/de/vi/"]"#).unwrap();
    let img_sel = Selector::parse("img[src]").unwrap();
    let title_sel = Selector::parse("h2 a").unwrap();

    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for card in doc.select(&card_sel) {
        let Some(link) = card.select(&link_sel).next() else {
            continue;
        };
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        let url = absolute(href, origin);
        if !seen.insert(url.clone()) {
            continue;
        }

        // Title priority: <h2><a> text → <img alt> → first link's text.
        let title = card
            .select(&title_sel)
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                card.select(&img_sel)
                    .next()
                    .and_then(|i| i.value().attr("alt"))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_default();
        if title.is_empty() || title.len() > 200 {
            continue;
        }

        let price = find_price_in_subtree(card);
        // Image lookup order (see `extract_image_map` docstring):
        //   1. Next.js dehydrated state blob, keyed by listingID.
        //   2. DOM `<img src>` if it's a real URL (not a data: placeholder).
        //   3. Give up (some listings genuinely have no image).
        let card_id = card
            .value()
            .attr("data-private-srp-listing-item-id")
            .unwrap_or("");
        let image = image_map
            .get(card_id)
            .cloned()
            .or_else(|| {
                card.select(&img_sel)
                    .filter_map(|i| i.value().attr("src"))
                    .find(|s| !s.starts_with("data:"))
                    .map(str::to_string)
            });

        // Description snippet lives in a sibling span; collecting all card
        // text is noisy but fine for keyword matching (we don't store it).
        let body = card.text().collect::<String>();

        out.push(Extracted {
            url,
            title,
            price,
            image,
            body,
        });
    }

    out
}

/// Case-insensitive substring match of `query` tokens against title or body.
/// Empty query matches everything. Splits on whitespace and requires each
/// token to match somewhere — so `"pumpfoil board"` matches a listing whose
/// title contains `pumpfoil` and description contains `board`.
///
/// We also test against a compacted copy of the haystack (spaces and hyphens
/// stripped) so `pumpfoil` matches a listing titled "Pump Foil Board" or
/// "Pump-Foil". Swiss classifieds freely mix the compound-noun and
/// separated spellings for the same gear.
pub fn matches_query(query: &str, title: &str, body: &str) -> bool {
    let q = query.trim();
    if q.is_empty() {
        return true;
    }
    let haystack = format!("{title}\n{body}").to_lowercase();
    let compact: String = haystack.chars().filter(|c| *c != ' ' && *c != '-').collect();
    q.split_whitespace().all(|t| {
        let needle = t.to_lowercase();
        let needle_compact: String = needle.chars().filter(|c| *c != ' ' && *c != '-').collect();
        haystack.contains(&needle) || compact.contains(&needle_compact)
    })
}

pub fn to_listing(src: &'static str, ex: Extracted) -> Listing {
    Listing {
        source: src.to_string(),
        brand: None,
        title: ex.title,
        url: ex.url,
        price: ex.price,
        currency: Some("CHF".to_string()),
        condition: Condition::Used,
        available: Some(true),
        location: None,
        description: None,
        image: ex.image,
        region: Region::Ch,
        fetched_at: Utc::now(),
    }
}

// Silence unused-import warning on feature-flagged builds.
#[allow(dead_code)]
fn _force_use() {
    let _ = parse_swiss_price;
}
