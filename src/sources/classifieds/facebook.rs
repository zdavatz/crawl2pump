//! Facebook Marketplace — JS-rendered, login-gated, heavily fingerprinted.
//!
//! Access model: we reuse the shared chromiumoxide Chrome; the user logs in
//! manually once via `--headful --sources facebook`, and the resulting
//! session cookie persists in `.chrome-profile/` until FB expires it
//! (weeks/months). Subsequent headless runs work until then.
//!
//! Selector strategy: FB obfuscates CSS class names aggressively (rotating
//! hashes every few weeks), so we key off the one stable signal — the
//! `/marketplace/item/{id}/` href pattern — and walk up to a card container
//! for title / price / image extraction.
//!
//! ToS note: scraping Marketplace violates FB's terms. Use a throwaway
//! account, not your primary one — accounts can get flagged or suspended.
use super::{encode_query, parse_swiss_price, walk_up};
use crate::listing::{Condition, Listing, Region};
use crate::sources::browser::SharedBrowser;
use crate::sources::Source;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use scraper::{Html, Selector};
use std::collections::HashSet;
use std::sync::{Arc, OnceLock};

const ORIGIN: &str = "https://www.facebook.com";
/// FB's Marketplace item detail URLs are the most stable anchor we have —
/// class names rotate every couple of weeks but item URLs don't.
const CARD_ANCHOR: &str = r#"a[href^="/marketplace/item/"]"#;
const SETTLE_MS: u64 = 7000;

pub struct Facebook {
    browser: Arc<SharedBrowser>,
    location: String,
}

impl Facebook {
    pub fn new(browser: Arc<SharedBrowser>, location: String) -> Self {
        Self { browser, location }
    }

    fn search_url(&self, query: &str) -> String {
        let q = encode_query(query);
        if self.location.is_empty() || self.location.eq_ignore_ascii_case("worldwide") {
            format!("{ORIGIN}/marketplace/search?query={q}&exact=false")
        } else {
            format!(
                "{ORIGIN}/marketplace/{}/search?query={q}&exact=false",
                self.location.to_lowercase()
            )
        }
    }
}

#[async_trait]
impl Source for Facebook {
    fn name(&self) -> &'static str {
        "facebook"
    }
    fn region(&self) -> Region {
        // Swiss cities → Region::Ch; everything else treated as worldwide.
        const CH_CITIES: &[&str] = &[
            "zurich", "bern", "basel", "geneva", "genf", "lausanne",
            "luzern", "lucerne", "st-gallen", "stgallen", "lugano",
            "winterthur", "zug", "chur",
        ];
        let loc = self.location.to_lowercase();
        if CH_CITIES.iter().any(|c| loc.contains(c)) {
            Region::Ch
        } else {
            Region::World
        }
    }

    async fn search(&self, query: &str) -> Result<Vec<Listing>> {
        let url = self.search_url(query);
        let html = super::fetch_rendered(&self.browser, &url, SETTLE_MS)
            .await
            .with_context(|| format!("GET {url}"))?;

        if looks_logged_out(&html) {
            return Err(anyhow!(
                "Facebook login required. Run once with --headful --sources facebook, \
                 log in (use a throwaway account — scraping violates FB ToS), then re-run."
            ));
        }

        let doc = Html::parse_document(&html);
        let link_sel = Selector::parse(CARD_ANCHOR).unwrap();
        let img_sel = Selector::parse("img").unwrap();

        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for a in doc.select(&link_sel) {
            let Some(href) = a.value().attr("href") else {
                continue;
            };
            // FB hrefs are always root-relative. Trim trailing query/fragment
            // so different referrers collapse into the same dedup key.
            let clean_href = href.split('?').next().unwrap_or(href);
            let abs = format!("{ORIGIN}{clean_href}");
            if !seen.insert(abs.clone()) {
                continue;
            }

            // FB nests item cards 6–8 div levels deep under the anchor.
            let card = walk_up(a, 7);
            // `.text()` yields leaf text nodes — join with newlines so sibling
            // spans don't glom together ("1.200 CHFFoil Set" etc.).
            let card_text: String = card.text().collect::<Vec<&str>>().join("\n");

            let title = first_title(&card_text);
            if title.is_empty() {
                continue;
            }

            let price = parse_fb_price(&card_text);
            let currency = detect_currency(&card_text);
            let location = parse_fb_location(&card_text);
            let image = card
                .select(&img_sel)
                .next()
                .and_then(|i| i.value().attr("src").map(str::to_string));

            out.push(Listing {
                source: "facebook".to_string(),
                brand: None,
                title,
                url: abs,
                price,
                currency,
                condition: Condition::Used,
                available: Some(true),
                location,
                description: None,
                image,
                region: self.region(),
                fetched_at: Utc::now(),
            });
        }
        Ok(out)
    }
}

/// Heuristic: FB redirects anonymous users to `/login/...` and renders an
/// unmistakable login form. We also check the common marketplace
/// "not-available" redirect which is served before login.
fn looks_logged_out(html: &str) -> bool {
    let h = html.to_ascii_lowercase();
    h.contains(r#"id="loginbutton""#)
        || h.contains(r#"name="login""#)
        || h.contains("login_popup_screen")
        || h.contains("you must log in to continue")
        || h.contains("du musst dich anmelden")
}

fn first_title(card_text: &str) -> String {
    card_text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && l.len() > 3 && l.len() <= 200)
        .find(|l| !is_priceish(l) && !is_noise(l))
        .unwrap_or_default()
        .to_string()
}

/// A line is "priceish" (and should not be chosen as a title) if it looks
/// like a pure price — few letters, at least one digit, or parses cleanly
/// as a Swiss price. Titles typically contain real words so have many
/// alphabetic chars.
fn is_priceish(s: &str) -> bool {
    let digits = s.chars().filter(|c| c.is_ascii_digit()).count();
    let alpha = s.chars().filter(|c| c.is_alphabetic()).count();
    if digits >= 1 && alpha <= 4 {
        return true; // "175 CHF", "CHF 1.200", "2'500.-"
    }
    if super::parse_swiss_price(s).is_some() && alpha <= 6 {
        return true;
    }
    false
}

fn is_noise(s: &str) -> bool {
    let low = s.to_ascii_lowercase();
    matches!(
        low.as_str(),
        "marketplace"
            | "free"
            | "kostenlos"
            | "gratis"
            | "siehe beschreibung"
            | "see description"
    )
}

fn parse_fb_price(text: &str) -> Option<f64> {
    // Marketplace in CH localizes to CHF; try Swiss formatting first.
    if let Some(p) = parse_swiss_price(text) {
        return Some(p);
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)[$€£¥]\s*([0-9][0-9',\s.]*)").unwrap()
    });
    let caps = re.captures(text)?;
    let raw = caps.get(1)?.as_str();
    let cleaned: String = raw.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    cleaned.trim_end_matches('.').parse::<f64>().ok()
}

fn detect_currency(text: &str) -> Option<String> {
    if text.contains("CHF") || text.contains("Fr.") {
        Some("CHF".into())
    } else if text.contains('$') {
        Some("USD".into())
    } else if text.contains('€') {
        Some("EUR".into())
    } else if text.contains('£') {
        Some("GBP".into())
    } else {
        None
    }
}

fn parse_fb_location(text: &str) -> Option<String> {
    // FB renders location as its own line, often "Zürich, ZH" / "Bern · 5 km"
    // near the bottom of the card. Match on a city-like pattern.
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?m)^([A-ZÄÖÜ][\p{L}\s-]{1,40}?)(?:,\s*[A-Z]{2,3}|\s*·|\s*$)").unwrap()
    });
    for line in text.lines() {
        let t = line.trim();
        if t.len() < 3 || t.len() > 60 {
            continue;
        }
        if let Some(caps) = re.captures(t) {
            let candidate = caps.get(1)?.as_str().trim().to_string();
            if candidate.chars().any(|c| c.is_ascii_digit()) {
                continue;
            }
            return Some(candidate);
        }
    }
    None
}
