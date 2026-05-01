//! `pumpfoil_report` — one-shot brand-shop scan + categorized PDF +
//! SQLite persistence with new/modified/unchanged tracking.
//!
//! Wraps the four-step pipeline that used to be hand-chained on the
//! shell (`crawl2pump --format json | jq | enrich_frontwings |
//! listings_pdf`). On every run it persists into
//! `sqlite/crawl2pump.db` so subsequent runs can highlight what's
//! genuinely new vs what changed (price, availability, image) vs
//! what's been seen unchanged.
//!
//! Usage:
//!   ./target/release/pumpfoil_report                       # all categories, ~/Downloads/pumpfoil.pdf
//!   ./target/release/pumpfoil_report --frontwings-only     # front wings only PDF
//!   ./target/release/pumpfoil_report --output /tmp/x.pdf
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use crawl2pump::db::{Db, ListingRow};
use crawl2pump::listing::{Condition, Listing};
use crawl2pump::Cli as CrawlCli;
use regex::Regex;
use reqwest::Client;
use scraper::Selector;
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

const CHROME_MAC: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

#[derive(Parser, Debug)]
#[command(version, about = "Scan pumpfoil brand shops, persist to SQLite, render PDF")]
struct Args {
    /// PDF output path. Default: ~/Downloads/pumpfoil.pdf
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Only emit Front Wings (with full spec rows) — skips sets/boards/packs/accessories.
    #[arg(long)]
    frontwings_only: bool,
    /// SQLite path. Default: ./sqlite/crawl2pump.db
    #[arg(long, default_value = crawl2pump::db::DEFAULT_PATH)]
    db: PathBuf,
    /// Skip the live crawl and re-render from whatever's in the DB.
    /// Useful for fast PDF iteration after a real scan.
    #[arg(long)]
    from_db: bool,
    /// Disable the front-wing detail-page enrichment fallback (keeps
    /// title-extracted area where the description-regex misses).
    #[arg(long)]
    no_spec_fetch: bool,
    /// Comma-separated brands to scan. Default: all brand shops.
    #[arg(long)]
    sources: Option<String>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| dirs_downloads().join("pumpfoil.pdf"));

    let scan_at = Utc::now();
    let db = Db::open(&args.db).context("open sqlite db")?;
    drop(db); // re-open mutable below; this is just an early "does it work" check

    let listings = if args.from_db {
        eprintln!("--from-db: skipping crawl, re-rendering from {}", args.db.display());
        Vec::new()
    } else {
        let crawl_cli = CrawlCli {
            sources: args.sources.clone(),
            no_browser: true,
            ..Default::default()
        };
        eprintln!("scanning brand shops…");
        let raw = crawl2pump::crawl_listings(crawl_cli).await?;
        eprintln!("  {} raw listings", raw.len());
        raw
    };

    // Trust list: brand sources whose modules already filter to pump-foil
    // gear. Other sources (Gong/Lift) returns the global catalog and we
    // post-filter by title.
    let curated: HashSet<&'static str> = [
        "axis",
        "onix",
        "indiana",
        "alpinefoil",
        "ketos",
        "armstrong",
        "takoon",
        "code",
        "north",
    ]
    .into_iter()
    .collect();
    let pump_re = Regex::new(r"(?i)pump.?foil|pumping|dockstart").unwrap();

    let pump_listings: Vec<Listing> = listings
        .into_iter()
        .filter(|l| {
            curated.contains(l.source.as_str()) || pump_re.is_match(&l.title)
        })
        .filter(|l| matches!(l.condition, Condition::New))
        // sentinel filters: drop $0 placeholders ("Push Edito" Gong rows
        // and similar) and obviously absurd prices.
        .filter(|l| !l.title.to_lowercase().contains("push edito"))
        .filter(|l| l.price.map(|p| p > 0.0 && p < 100_000.0).unwrap_or(true))
        .collect();
    eprintln!("  {} after curated + pump filter", pump_listings.len());

    // Categorize.
    let mut categorized: Vec<(Category, Listing, Option<WingSpecs>)> = pump_listings
        .into_iter()
        .map(|l| {
            let c = classify(&l);
            (c, l, None)
        })
        .collect();

    // Front-wing spec enrichment (title parse, description regex, then
    // optional detail-page fetch).
    {
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (compatible; pumpfoil-report)")
            .build()?;
        let total_fw = categorized
            .iter()
            .filter(|(c, _, _)| *c == Category::FrontWings)
            .count();
        let mut done = 0;
        for (c, l, specs) in categorized.iter_mut() {
            if *c != Category::FrontWings {
                continue;
            }
            done += 1;
            let mut s = WingSpecs::default();
            extract_from_title(&l.title, &mut s);
            if let Some(d) = &l.description {
                extract_from_text(d, &mut s);
            }
            if !args.no_spec_fetch && (s.area_cm2.is_none() || s.span_mm.is_none()) {
                eprintln!("  fw [{done}/{total_fw}] fetch {}", l.url);
                if let Ok(r) = client.get(&l.url).send().await {
                    if r.status().is_success() {
                        if let Ok(html) = r.text().await {
                            extract_from_text(&html, &mut s);
                            extract_from_html_table(&html, &mut s);
                        }
                    }
                }
            }
            // AR / chord computed from area + span when not explicit.
            if s.aspect_ratio.is_none() {
                if let (Some(a), Some(sp)) = (s.area_cm2, s.span_mm) {
                    let span_cm = sp / 10.0;
                    if a > 0.0 {
                        s.aspect_ratio = Some((span_cm * span_cm) / a);
                    }
                }
            }
            if s.chord_mm.is_none() {
                if let (Some(a), Some(sp)) = (s.area_cm2, s.span_mm) {
                    if sp > 0.0 {
                        s.chord_mm = Some((a * 100.0) / sp);
                    }
                }
            }
            if s.area_cm2.is_some() || s.span_mm.is_some() {
                *specs = Some(s);
            }
        }
    }

    // Persist the scan to SQLite. Diff before we render so we can show
    // "what's new" / "what changed" markers in the PDF.
    let summary = {
        let mut db = Db::open(&args.db)?;
        let rows: Vec<ListingRow<'_>> = categorized
            .iter()
            .map(|(cat, l, specs)| {
                ListingRow::from_listing(l, Some(cat.label()))
                    .with_specs(
                        specs.as_ref().and_then(|s| s.area_cm2),
                        specs.as_ref().and_then(|s| s.span_mm),
                        specs.as_ref().and_then(|s| s.aspect_ratio),
                        specs.as_ref().and_then(|s| s.chord_mm),
                    )
            })
            .collect();
        if !rows.is_empty() {
            db.upsert_scan(scan_at, &rows)?
        } else {
            crawl2pump::db::UpsertSummary {
                new_count: 0,
                updated_count: 0,
                modified_count: 0,
                price_changes: 0,
            }
        }
    };
    eprintln!(
        "  db: {} new, {} updated ({} content-modified, {} price changes)",
        summary.new_count, summary.updated_count, summary.modified_count, summary.price_changes
    );

    // Mark each listing's freshness (new / modified / unchanged) for the
    // render. We re-query the DB by URL so it works in `--from-db` mode
    // too.
    let freshness: std::collections::HashMap<String, Freshness> = {
        let db = Db::open(&args.db)?;
        let mut m = std::collections::HashMap::new();
        for l in db.new_in_scan(scan_at)? {
            m.insert(l.url.clone(), Freshness::New);
        }
        for l in db.modified_in_scan(scan_at)? {
            m.entry(l.url.clone()).or_insert(Freshness::Modified);
        }
        m
    };

    // Optional `--frontwings-only` filter.
    if args.frontwings_only {
        categorized.retain(|(c, _, _)| *c == Category::FrontWings);
    }

    // Sort within categories.
    categorized.sort_by(|a, b| match a.0 {
        // Rank for category, then domain-specific intra-category sort.
        _ => a.0.order().cmp(&b.0.order()),
    });
    // Stable second pass for intra-group order.
    categorized.sort_by(|a, b| {
        a.0.order().cmp(&b.0.order()).then_with(|| match a.0 {
            Category::FrontWings => {
                // Sort by flat surface area DESCENDING — biggest wings
                // (beginner / glide) first, smallest (high-performance /
                // race) last. No-spec wings sink to the bottom.
                let ka = a.2.as_ref().and_then(|s| s.area_cm2);
                let kb = b.2.as_ref().and_then(|s| s.area_cm2);
                match (ka, kb) {
                    (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a.1.price.partial_cmp(&b.1.price).unwrap_or(std::cmp::Ordering::Equal),
                }
            }
            _ => a
                .1
                .price
                .partial_cmp(&b.1.price)
                .unwrap_or(std::cmp::Ordering::Equal),
        })
    });

    // Render HTML → PDF.
    let html = render_html(&categorized, &freshness, &summary, scan_at, args.frontwings_only);
    let html_path = output.with_extension("html");
    std::fs::write(&html_path, html)?;
    let chrome = std::env::var("CHROME").unwrap_or_else(|_| CHROME_MAC.into());
    let status = Command::new(&chrome)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-pdf-header-footer",
            &format!("--print-to-pdf={}", output.display()),
            &format!("file://{}", std::fs::canonicalize(&html_path)?.display()),
        ])
        .status()
        .with_context(|| format!("spawn {chrome}"))?;
    if !status.success() {
        return Err(anyhow!("Chrome print-to-pdf exited {status}"));
    }
    eprintln!("wrote {}", output.display());
    eprintln!("wrote {}", html_path.display());
    Ok(())
}

fn dirs_downloads() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Downloads"))
        .unwrap_or_else(|_| PathBuf::from("."))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Category {
    Sets,
    Boards,
    FoilPacks,
    FrontWings,
    Accessories,
}
impl Category {
    fn label(&self) -> &'static str {
        match self {
            Category::Sets => "Sets (Foil + Board)",
            Category::Boards => "Boards only",
            Category::FoilPacks => "Foil Packs (no Board)",
            Category::FrontWings => "Front Wings",
            Category::Accessories => "Other Components & Accessories",
        }
    }
    fn order(&self) -> u8 {
        match self {
            Category::Sets => 0,
            Category::Boards => 1,
            Category::FoilPacks => 2,
            Category::FrontWings => 3,
            Category::Accessories => 4,
        }
    }
}

fn classify(l: &Listing) -> Category {
    let t = l.title.to_lowercase();
    let u = l.url.to_lowercase();
    let size_board = Regex::new(r"\d+(?:'\d+)?\s*(?:pumpfoil|pump foil|pump-foil)\b")
        .unwrap()
        .is_match(&t);
    let has_board = t.contains("board")
        || t.contains("foilboard")
        || u.contains("/board/")
        || u.contains("-board-")
        || u.contains("-board.");
    let has_pack = t.contains("pack")
        || t.contains(" set ")
        || t.contains(" set,")
        || t.ends_with(" set")
        || t.contains("package")
        || t.contains(" kit")
        || t.contains("complete")
        || t.contains("combo")
        || t.contains("bundle");
    let is_accessory = ["backpack","bag","cover","leash","traction","pad","screw","anode","repair","valve","strap","wetsuit","hardware","t-shirt","sticker","hood","shim","spacer","antiseize","lubricant","bolt"]
        .iter().any(|w| t.contains(w));
    if is_accessory { return Category::Accessories; }
    if has_pack && has_board { return Category::Sets; }
    if has_pack { return Category::FoilPacks; }
    if has_board || size_board { return Category::Boards; }
    let is_tail = t.contains("tail") || t.contains("stab") || t.contains("rear");
    if !is_tail
        && (t.contains("front wing")
            || t.contains("frontwing")
            || t.contains("front foil")
            || t.contains("aile avant")
            || t.contains("hydrofoil wing")
            || (t.contains(" wing") && !t.contains("a-wing") && !t.contains("wing pump")))
    {
        return Category::FrontWings;
    }
    Category::Accessories
}

#[derive(Debug, Default, Clone, Serialize)]
struct WingSpecs {
    area_cm2: Option<f64>,
    span_mm: Option<f64>,
    aspect_ratio: Option<f64>,
    chord_mm: Option<f64>,
}

fn extract_from_title(title: &str, s: &mut WingSpecs) {
    if s.span_mm.is_none() {
        let re = Regex::new(r"(?i)\b(\d{3,4})\s*mm\b").unwrap();
        if let Some(c) = re.captures(title.trim()) {
            if let Ok(v) = c[1].parse::<f64>() {
                if (300.0..=2500.0).contains(&v) {
                    s.span_mm = Some(v);
                }
            }
        }
    }
    if s.area_cm2.is_none() {
        let re = Regex::new(
            r"(?i)\b(?:PNG|BSC|HPS|SP|HA|ART|PUMPING|Pumping|Aile\s+Avant|EVO|UHM|HM|F-One|FONE)\s*(\d{3,4})\b",
        )
        .unwrap();
        if let Some(c) = re.captures(title.trim()) {
            if let Ok(v) = c[1].parse::<f64>() {
                if (200.0..=2500.0).contains(&v) {
                    s.area_cm2 = Some(v);
                }
            }
        }
    }
}

fn extract_from_text(text: &str, s: &mut WingSpecs) {
    if s.area_cm2.is_none() {
        for re in [
            Regex::new(r"(?i)(?:surface(?:\s+area)?|area|aire)[\s:=]*(\d{3,4})\s*(?:cm[²2]|sq\s*cm)?")
                .unwrap(),
            Regex::new(r"(\d{3,4})\s*cm[²2]").unwrap(),
        ] {
            if let Some(c) = re.captures(text) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (200.0..=2500.0).contains(&v) {
                        s.area_cm2 = Some(v);
                        break;
                    }
                }
            }
        }
    }
    if s.span_mm.is_none() {
        for re in [
            Regex::new(r"(?i)(?:wingspan|span|envergure)[\s:=]*(\d{3,4})\s*mm").unwrap(),
            Regex::new(r"(?i)(?:wingspan|span|envergure)[\s:=]*(\d{3,4})\b").unwrap(),
            Regex::new(r"\b(\d{3,4})\s*mm\s*(?:wingspan|span)").unwrap(),
        ] {
            if let Some(c) = re.captures(text) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (300.0..=2500.0).contains(&v) {
                        s.span_mm = Some(v);
                        break;
                    }
                }
            }
        }
    }
    if s.aspect_ratio.is_none() {
        for re in [
            Regex::new(r"(?i)aspect\s+ratio[^0-9]{0,20}(\d{1,2}(?:\.\d{1,2})?)").unwrap(),
            Regex::new(r"(?i)\bAR[\s:=]+(\d{1,2}(?:\.\d{1,2})?)").unwrap(),
            Regex::new(r"(?i)(\d{1,2}(?:\.\d{1,2})?)\s*aspect\s+ratio").unwrap(),
        ] {
            if let Some(c) = re.captures(text) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (3.0..=15.0).contains(&v) {
                        s.aspect_ratio = Some(v);
                        break;
                    }
                }
            }
        }
    }
    if s.chord_mm.is_none() {
        for re in [
            Regex::new(r"(?i)chord[\s:=]*(\d{2,4})\s*mm").unwrap(),
            Regex::new(r"(?i)chord[\s:=]*(\d{2,4})\b").unwrap(),
        ] {
            if let Some(c) = re.captures(text) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (50.0..=400.0).contains(&v) {
                        s.chord_mm = Some(v);
                        break;
                    }
                }
            }
        }
    }
}

fn extract_from_html_table(html_text: &str, s: &mut WingSpecs) {
    let doc = scraper::Html::parse_document(html_text);
    let row_sel = Selector::parse("tr").unwrap();
    let cell_sel = Selector::parse("th, td").unwrap();
    for tr in doc.select(&row_sel) {
        let cells: Vec<String> = tr
            .select(&cell_sel)
            .map(|c| c.text().collect::<String>().trim().to_string())
            .collect();
        if cells.len() < 2 { continue; }
        let key = cells[0].to_lowercase();
        if let Some(n) = first_number(&cells[1]) {
            if s.area_cm2.is_none() && (key.contains("surface") || key.contains("area") || key.contains("aire")) && (200.0..=2500.0).contains(&n) {
                s.area_cm2 = Some(n);
            }
            if s.span_mm.is_none() && (key.contains("span") || key.contains("envergure") || key.contains("wingspan")) && (300.0..=2500.0).contains(&n) {
                s.span_mm = Some(n);
            }
            if s.aspect_ratio.is_none() && (key.contains("aspect") || key == "ar") && (3.0..=15.0).contains(&n) {
                s.aspect_ratio = Some(n);
            }
            if s.chord_mm.is_none() && key.contains("chord") && (50.0..=400.0).contains(&n) {
                s.chord_mm = Some(n);
            }
        }
    }
}

fn first_number(s: &str) -> Option<f64> {
    Regex::new(r"\d+(?:[.,]\d+)?")
        .unwrap()
        .find(s)
        .and_then(|m| m.as_str().replace(',', ".").parse().ok())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Freshness {
    New,
    Modified,
}

fn render_html(
    cats: &[(Category, Listing, Option<WingSpecs>)],
    freshness: &std::collections::HashMap<String, Freshness>,
    summary: &crawl2pump::db::UpsertSummary,
    scan_at: DateTime<Utc>,
    frontwings_only: bool,
) -> String {
    let today = scan_at.format("%Y-%m-%d %H:%M UTC").to_string();
    // Group cards by category for the section structure.
    let mut by_cat: Vec<(Category, Vec<&(Category, Listing, Option<WingSpecs>)>)> = Vec::new();
    for cat in [Category::Sets, Category::Boards, Category::FoilPacks, Category::FrontWings, Category::Accessories] {
        let items: Vec<_> = cats.iter().filter(|(c, _, _)| *c == cat).collect();
        if !items.is_empty() {
            by_cat.push((cat, items));
        }
    }
    let total: usize = by_cat.iter().map(|(_, v)| v.len()).sum();

    let body: String = by_cat
        .iter()
        .map(|(cat, items)| {
            let cards = items
                .iter()
                .map(|(_, l, specs)| render_card(l, specs.as_ref(), freshness.get(&l.url).copied()))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                r#"<h2 class="cat">{} <span class="cat-count">{}</span></h2>
{cards}"#,
                html_escape(cat.label()),
                items.len()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let title = if frontwings_only { "Pumpfoil Front Wings" } else { "Pumpfoil Catalog" };
    let mode = if frontwings_only { " · front wings only" } else { "" };
    format!(
        r#"<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<title>{title} · {today}</title>
<style>
  @page {{ size: A4; margin: 12mm 12mm 14mm; }}
  * {{ box-sizing: border-box; }}
  body {{ font-family: -apple-system, "Helvetica Neue", Arial, sans-serif; font-size: 10pt; color: #111; margin: 0; }}
  h1 {{ font-size: 16pt; margin: 0 0 2mm; }}
  h2.cat {{
    font-size: 13pt; margin: 8mm 0 2mm; padding: 1mm 0;
    border-bottom: 2px solid #0a58ca; color: #0a58ca;
    page-break-after: avoid; break-after: avoid;
  }}
  h2.cat .cat-count {{ color: #888; font-size: 10pt; font-weight: 400; }}
  .sub {{ color: #555; font-size: 9pt; margin-bottom: 2mm; }}
  .diff {{ color: #444; font-size: 9pt; margin-bottom: 6mm; }}
  .diff .pill {{ display: inline-block; padding: 0.5mm 2mm; border-radius: 1mm; margin-right: 2mm; }}
  .pill-new      {{ background: #d1f0d6; color: #0e6132; }}
  .pill-modified {{ background: #fff3cd; color: #856404; }}
  .card {{
    display: grid; grid-template-columns: 44mm 1fr 30mm; gap: 5mm;
    padding: 4mm 0; border-bottom: 1px solid #e5e5e5;
    break-inside: avoid; page-break-inside: avoid; align-items: start;
    position: relative;
  }}
  .badge {{
    position: absolute; top: 4mm; right: 32mm;
    font-size: 7.5pt; font-weight: 700; padding: 0.5mm 1.5mm; border-radius: 1mm;
  }}
  .badge.new      {{ background: #0e6132; color: white; }}
  .badge.modified {{ background: #f0ad4e; color: white; }}
  .thumb-link {{ display: inline-block; }}
  .thumb {{ width: 44mm; height: 34mm; object-fit: cover; border: 1px solid #ddd; border-radius: 3px; }}
  .title {{ display: inline-block; font-size: 11pt; font-weight: 600; color: #0a58ca; text-decoration: none; line-height: 1.25; }}
  .meta {{ color: #666; font-size: 8.5pt; margin: 1mm 0 2mm; }}
  .desc {{ margin: 0; color: #222; font-size: 9pt; line-height: 1.35; }}
  a.url {{ display: inline-block; color: #0a58ca; font-size: 7.5pt; margin-top: 1.5mm; word-break: break-all; text-decoration: underline; }}
  .price {{ text-align: right; font-variant-numeric: tabular-nums; font-weight: 700; font-size: 12pt; white-space: nowrap; }}
</style>
</head>
<body>
<h1>{title}</h1>
<div class="sub">{total} Angebote{mode} · {today} · via crawl2pump</div>
<div class="diff">
  <span class="pill pill-new">{} new</span>
  <span class="pill pill-modified">{} modified (price/spec/image)</span>
  · {} touched · {} price changes
</div>
{body}
</body>
</html>
"#,
        summary.new_count,
        summary.modified_count,
        summary.updated_count,
        summary.price_changes,
    )
}

fn render_card(
    l: &Listing,
    specs: Option<&WingSpecs>,
    freshness: Option<Freshness>,
) -> String {
    let price = match (l.price, l.currency.as_deref()) {
        (Some(p), Some(c)) => format!("{c} {}", fmt_thou(p)),
        (Some(p), None) => format!("CHF {}", fmt_thou(p)),
        _ => "—".into(),
    };
    let img = l.image.as_deref().unwrap_or("");
    let title = html_escape(&l.title);
    let desc = l
        .description
        .as_deref()
        .map(|s| html_escape(&shorten(s, 360)))
        .unwrap_or_default();
    let url = html_escape(&l.url);
    let specs_str = specs.and_then(format_specs);
    let meta_bits: Vec<String> = [
        Some(html_escape(&l.source)),
        l.location.as_deref().map(html_escape),
        specs_str.as_deref().map(html_escape),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect();
    let meta = meta_bits.join(" · ");
    let img_html = if img.is_empty() {
        String::new()
    } else {
        format!(r#"<a class="thumb-link" href="{url}"><img class="thumb" src="{img}" loading="lazy"/></a>"#)
    };
    let badge = match freshness {
        Some(Freshness::New) => r#"<span class="badge new">NEW</span>"#,
        Some(Freshness::Modified) => r#"<span class="badge modified">MOD</span>"#,
        None => "",
    };
    format!(
        r#"<section class="card">
  {badge}
  {img_html}
  <div class="body">
    <a class="title" href="{url}">{title}</a>
    <div class="meta">{meta}</div>
    <p class="desc">{desc}</p>
    <a class="url" href="{url}">{url}</a>
  </div>
  <div class="price">{price}</div>
</section>"#,
    )
}

fn format_specs(s: &WingSpecs) -> Option<String> {
    let mut bits = Vec::new();
    if let Some(a) = s.area_cm2 { bits.push(format!("{} cm²", a as i64)); }
    if let Some(span) = s.span_mm { bits.push(format!("{} mm span", span as i64)); }
    if let Some(ar) = s.aspect_ratio { bits.push(format!("AR {:.1}", ar)); }
    if let Some(c) = s.chord_mm { bits.push(format!("chord {} mm", c as i64)); }
    if bits.is_empty() { None } else { Some(bits.join(" · ")) }
}

fn shorten(s: &str, limit: usize) -> String {
    let cleaned = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() <= limit { return cleaned; }
    let mut out: String = cleaned.chars().take(limit).collect();
    if let Some(idx) = out.rfind(' ') { out.truncate(idx); }
    out.push('…');
    out
}

fn fmt_thou(v: f64) -> String {
    let v = v.round() as i64;
    let s = v.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.push('\''); }
        out.push(c);
    }
    out.chars().rev().collect()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}
