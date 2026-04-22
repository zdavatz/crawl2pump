//! Swiss classifieds sources (all Cloudflare-protected, fetched via headless Chrome).
//!
//! The three sites share:
//!   * Swiss price format (`CHF 1'499.-`, `Fr. 999.00`, `1'500.-`)
//!   * always-used condition
//!
//! Tutti and Anibis are the same Next.js codebase (same owner) — their search
//! URL uses an opaque binary-encoded token that we can't construct, so text
//! queries submitted via `?query=` are silently dropped. For those two we
//! load the "all recent listings" page and filter client-side against the
//! query via `tutti_anibis_cards::extract`. Ricardo still supports text
//! search directly via `/de/s/{query}`.
//!
//! Set `CRAWL2PUMP_DEBUG_HTML=./debug` to dump every fetched page before
//! parsing — handy when site HTML shifts.
pub mod anibis;
pub mod facebook;
pub mod ricardo;
pub mod tutti;
pub mod tutti_anibis_cards;

use crate::sources::browser::SharedBrowser;
use anyhow::{Context, Result};
use regex::Regex;
use scraper::ElementRef;
use std::sync::OnceLock;
use std::time::Duration;

/// Stealth patches applied to every document before navigation — the usual
/// set of overrides that defeat the public "is this headless Chrome?" tests
/// (webdriver flag, missing plugins, headless UA tail, WebGL vendor strings,
/// `window.chrome` shape, permissions API). Solves the cheap Cloudflare
/// checks but not the interactive Turnstile challenge — for that, run
/// `--headful` once to pass it manually; cookies persist in `.chrome-profile`.
const STEALTH_JS: &str = r#"
Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
Object.defineProperty(navigator, 'languages', { get: () => ['de-CH', 'de', 'en-US', 'en'] });
Object.defineProperty(navigator, 'plugins', {
  get: () => [
    { name: 'PDF Viewer' },
    { name: 'Chrome PDF Viewer' },
    { name: 'Chromium PDF Viewer' },
    { name: 'Microsoft Edge PDF Viewer' },
    { name: 'WebKit built-in PDF' },
  ],
});
Object.defineProperty(navigator, 'hardwareConcurrency', { get: () => 8 });
Object.defineProperty(navigator, 'deviceMemory', { get: () => 8 });
window.chrome = window.chrome || {
  app: { isInstalled: false },
  runtime: {},
  loadTimes: function () {},
  csi: function () {},
};
const origQuery = window.navigator.permissions && window.navigator.permissions.query;
if (origQuery) {
  window.navigator.permissions.query = (params) =>
    params.name === 'notifications'
      ? Promise.resolve({ state: Notification.permission })
      : origQuery(params);
}
// WebGL vendor/renderer spoof — Cloudflare's fingerprint includes these.
const getParam = WebGLRenderingContext.prototype.getParameter;
WebGLRenderingContext.prototype.getParameter = function (p) {
  if (p === 37445) return 'Intel Inc.';
  if (p === 37446) return 'Intel Iris OpenGL Engine';
  return getParam.call(this, p);
};
"#;

pub async fn fetch_rendered(browser: &SharedBrowser, url: &str, settle_ms: u64) -> Result<String> {
    let b = browser.get().await?;
    // Open blank, inject stealth on every new document, *then* navigate —
    // otherwise the target page runs fingerprinting before our patches land.
    let page = b
        .new_page("about:blank")
        .await
        .with_context(|| format!("new_page {url}"))?;
    let _ = page.evaluate_on_new_document(STEALTH_JS).await;
    let _ = page.goto(url).await?;
    tokio::time::sleep(Duration::from_millis(settle_ms)).await;

    // Cloudflare's JS challenge ("Just a moment…" / "Nur einen Moment…")
    // auto-redirects once solved. In headless mode that's either instant or
    // hopeless, so cap at 30s. In headful mode the user needs time to *see*
    // the window, find the Turnstile checkbox, and click — give 5 minutes
    // and print an actionable prompt. Once solved, the `cf_clearance` cookie
    // lands in `.chrome-profile/` so later headless runs can skip this.
    let mut html = page.content().await?;
    if is_cf_challenge(&html) && browser.is_headful() {
        eprintln!(
            "\n  [action] Cloudflare Turnstile challenge on {url}\n\
             \t  → switch to the Chrome window that opened and click the checkbox\n\
             \t    (\"Bestätigen Sie, dass Sie ein Mensch sind\").\n\
             \t  Waiting up to 5 minutes for you to solve it…\n"
        );
    }
    let total_budget = if browser.is_headful() {
        Duration::from_secs(300)
    } else {
        Duration::from_secs(30)
    };
    let deadline = std::time::Instant::now() + total_budget;
    while is_cf_challenge(&html) && std::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        html = page.content().await.unwrap_or(html);
    }
    if is_cf_challenge(&html) {
        eprintln!(
            "  [warn] Cloudflare JS challenge did not clear for {url}\n\
             \t  → run once with --headful and click the Turnstile checkbox in the \
               Chrome window. Cookies persist in .chrome-profile/"
        );
    } else if is_blocked(&html) {
        eprintln!(
            "  [warn] {url} returned a hard block (403 / Forbidden)\n\
             \t  → likely IP-rate-limited; wait 5-15 min and retry, or route through a VPN"
        );
    } else if browser.is_headful() {
        eprintln!("  [ok]   challenge cleared for {url}");
    }

    let _ = page.close().await;

    if let Ok(dir) = std::env::var("CRAWL2PUMP_DEBUG_HTML") {
        if !dir.is_empty() {
            let safe = url
                .replace("https://", "")
                .replace('/', "_")
                .chars()
                .take(80)
                .collect::<String>();
            let path = std::path::Path::new(&dir).join(format!("{safe}.html"));
            std::fs::create_dir_all(&dir).ok();
            std::fs::write(&path, &html).ok();
            eprintln!("  [dbg] dumped {url} -> {}", path.display());
        }
    }
    Ok(html)
}

/// Walk up N ancestors from `el` to reach the enclosing card container.
pub fn walk_up<'a>(el: ElementRef<'a>, levels: usize) -> ElementRef<'a> {
    let mut cur = el;
    for _ in 0..levels {
        match cur.parent().and_then(ElementRef::wrap) {
            Some(p) => cur = p,
            None => break,
        }
    }
    cur
}

/// Find the first descendant element whose own text (child text nodes only,
/// not recursive grandchildren) parses as a Swiss price. Used to narrow price
/// matching to the actual `<span>CHF 1'499.-</span>` leaf inside a card —
/// matching on card-concatenated text picks up prices from neighboring cards
/// when `walk_up` overshoots, or description text that happens to contain a
/// number.
pub fn find_price_in_subtree(el: ElementRef<'_>) -> Option<f64> {
    for desc in el.descendants() {
        if let Some(de) = ElementRef::wrap(desc) {
            let own: String = de
                .children()
                .filter_map(|n| n.value().as_text().map(|t| t.to_string()))
                .collect();
            let trimmed = own.trim();
            if trimmed.is_empty() || trimmed.len() > 60 {
                continue;
            }
            if let Some(p) = parse_swiss_price(trimmed) {
                return Some(p);
            }
        }
    }
    None
}

/// Parse Swiss prices from free text. Handles:
/// - `CHF 1'499.-`, `Fr. 999.00`, `1'500 CHF` (labelled)
/// - `1'199.00`, `1'500.-` (bare Swiss format — Ricardo/Tutti cards)
/// - `1.200 CHF` (European `.` thousands separator — used by FB Marketplace
///   when localized to DE/IT/CH-German)
/// - `1,200.00` (US `,` thousands, `.` decimal)
/// Bare numbers must use a thousands separator or end with `.-` / `.00` so
/// we don't match plain integers like years or IDs.
pub fn parse_swiss_price(text: &str) -> Option<f64> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?ix)
             (?:CHF|Fr\.?)\s*([0-9][0-9'.,\s]*(?:\.-)?)   # CHF 1'499.- / CHF 1.200
           | ([0-9][0-9'.,\s]*(?:\.-)?)\s*(?:CHF|Fr\.?)   # 1'499.- CHF / 1.200 CHF
           | ([0-9]{1,3}(?:'[0-9]{3})+(?:\.(?:-|[0-9]{2}))?)  # bare 1'499.00
           | ([0-9]{1,6}\.-)                              # bare 499.-
           | ([0-9]{2,6}\.[0-9]{2})                       # bare 499.00
           ",
        )
        .unwrap()
    });
    let caps = re.captures(text)?;
    let raw = (1..=5).filter_map(|i| caps.get(i)).next()?.as_str().trim();
    normalize_price_number(raw)
}

/// Decide which of `.`, `,`, `'` are thousands separators vs. decimal
/// separator, then produce an f64. The key rule: the *last* `.` or `,`
/// is the decimal separator **only if** it's followed by exactly 2 digits;
/// otherwise every `.` `,` `'` is a thousands separator.
fn normalize_price_number(raw: &str) -> Option<f64> {
    // Drop whitespace and the trailing ".-" (Swiss "whole francs" notation).
    let cleaned: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    let cleaned = cleaned.trim_end_matches(".-");
    if cleaned.is_empty() {
        return None;
    }

    // Find the candidate decimal point: rightmost `.` or `,` followed by
    // exactly 2 trailing digits (i.e. ".99" or ",99" at the end).
    let bytes = cleaned.as_bytes();
    let mut decimal_pos: Option<usize> = None;
    if bytes.len() >= 3 {
        let tail3 = &cleaned[cleaned.len() - 3..];
        let tail_bytes = tail3.as_bytes();
        if (tail_bytes[0] == b'.' || tail_bytes[0] == b',')
            && tail_bytes[1].is_ascii_digit()
            && tail_bytes[2].is_ascii_digit()
        {
            decimal_pos = Some(cleaned.len() - 3);
        }
    }

    let normalized: String = cleaned
        .char_indices()
        .filter_map(|(i, c)| match c {
            '0'..='9' => Some(c),
            '.' | ',' if Some(i) == decimal_pos => Some('.'),
            '.' | ',' | '\'' => None, // thousands separator → drop
            _ => None,
        })
        .collect();

    if normalized.is_empty() {
        return None;
    }
    normalized.parse::<f64>().ok()
}

fn is_cf_challenge(html: &str) -> bool {
    // CF's challenge pages expose themselves via the <title> tag — and only
    // those pages use it. Other pages may reference the CF beacon without
    // being the challenge interstitial, so we must NOT match on "cf_chl_opt"
    // alone or we get false positives on already-solved pages.
    let bytes = html.as_bytes();
    const TITLE_MARKERS: &[&[u8]] = &[
        b"<title>Just a moment",
        b"<title>Nur einen Moment",
        b"<title>Un instant",
    ];
    TITLE_MARKERS.iter().any(|m| memmem(bytes, m))
}

fn memmem(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn is_blocked(html: &str) -> bool {
    let bytes = html.as_bytes();
    const MARKERS: &[&[u8]] = &[
        b"<title>Forbidden</title>",
        b"<title>Access denied</title>",
        b"<title>Error 1020</title>",
        b"<title>Error 1015</title>",
    ];
    MARKERS.iter().any(|m| memmem(bytes, m))
}

pub fn absolute(href: &str, base_origin: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_string()
    } else if href.starts_with('/') {
        format!("{base_origin}{href}")
    } else {
        format!("{base_origin}/{href}")
    }
}

/// URL-encode a free-text query for search paths. Spaces → `+`; everything
/// else passes through since typical foil queries are ASCII.
pub fn encode_query(q: &str) -> String {
    q.split_whitespace().collect::<Vec<_>>().join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::Html;

    #[test]
    fn prices() {
        assert_eq!(parse_swiss_price("CHF 1'499.-"), Some(1499.0));
        assert_eq!(parse_swiss_price("Fr. 999.00"), Some(999.0));
        assert_eq!(parse_swiss_price("1'500 CHF"), Some(1500.0));
        assert_eq!(parse_swiss_price("1'199.00"), Some(1199.0));
        assert_eq!(parse_swiss_price("1'600.-"), Some(1600.0));
        assert_eq!(parse_swiss_price("499.00"), Some(499.0));
        // FB Marketplace uses European `.` thousands separator
        assert_eq!(parse_swiss_price("1.200 CHF"), Some(1200.0));
        assert_eq!(parse_swiss_price("CHF 2.500"), Some(2500.0));
        // US-style
        assert_eq!(parse_swiss_price("1,200.00 CHF"), Some(1200.0));
        assert_eq!(parse_swiss_price("no price here"), None);
    }

    #[test]
    fn query_matches_compound_noun_variants() {
        use super::tutti_anibis_cards::matches_query;
        // Single-word query should match separated-word listing and vice versa.
        assert!(matches_query("pumpfoil", "Pump Foil Board", ""));
        assert!(matches_query("pumpfoil", "Pump-Foil Board", ""));
        assert!(matches_query("pumpfoil", "Pumpfoil Board", ""));
        assert!(matches_query("pump foil", "Pumpfoil Board", ""));
        // Non-matches stay non-matches.
        assert!(!matches_query("pumpfoil", "Coffee grinder", ""));
        // Empty query matches anything.
        assert!(matches_query("", "anything", ""));
    }

    #[test]
    fn price_subtree_picks_leaf_not_aggregated() {
        // Two sibling "cards" under the same parent: without leaf-text
        // matching, a naive concat-then-regex would return the first card's
        // price for every walk that overshoots to this parent.
        let html = r#"
          <div id="grid">
            <article><a href="/a/1"><img/></a><div><span>CHF 100.-</span></div></article>
            <article><a href="/a/2"><img/></a><div><span>CHF 250.-</span></div></article>
          </div>
        "#;
        let doc = Html::parse_document(html);
        let sel = scraper::Selector::parse("article").unwrap();
        let prices: Vec<_> = doc.select(&sel).map(find_price_in_subtree).collect();
        assert_eq!(prices, vec![Some(100.0), Some(250.0)]);
    }
}
