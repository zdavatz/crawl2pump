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
use base64::Engine;
use chrono::{DateTime, Utc};
use clap::Parser;
use crawl2pump::db::{Db, ListingRow, StoredListing};
use crawl2pump::listing::{Condition, Listing, Region};
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
    /// Only emit Boards. Sorted by volume (litres) ascending, falling
    /// back to price ascending where volume isn't published.
    #[arg(long)]
    boards_only: bool,
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

    // `--from-db` short-circuits the crawl + enrichment + upsert — we
    // rebuild the categorized list directly from the most recent scan's
    // rows in the DB. The render path below treats it the same as a
    // freshly-crawled list.
    if args.from_db {
        eprintln!(
            "--from-db: skipping crawl, re-rendering from {}",
            args.db.display()
        );
        let categorized = load_categorized_from_db(&args.db)?;
        eprintln!("  {} listings loaded from DB", categorized.len());
        return render_from_categorized(&args, &output, scan_at, categorized).await;
    }

    let listings = {
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
        "mio",
        "starboard",
        "naish",
        "ensis",
        "pumpzuerich",
        "gong",
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

    // Front-wing spec enrichment runs in three passes. Pass 1 is cheap
    // and in-place (title parse + description regex). Pass 2 fetches
    // detail pages for wings still missing area/span — that's the slow
    // part, ~10 s per wing × 213 wings serially. We collect them into a
    // `buffer_unordered(8)` stream so 8 fetches are in-flight at any
    // moment; brings real runs from ~25 min to ~3 min. Pass 3 computes
    // AR / chord from area + span.
    {
        // Use the same UA the crawler does — Naish (and likely others)
        // serve a stripped-down page to a `(compatible; ...)` UA that
        // hides the per-variant spec block (`Aspect_ratio:` / `Front
        // wing span cm:` / `Projected surface area cm2:`). A regular
        // Safari UA gets the full page.
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                 AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
            )
            .build()?;

        // Pass 1: cheap title + description regex.
        for (c, l, specs) in categorized.iter_mut() {
            if *c != Category::FrontWings {
                continue;
            }
            let mut s = WingSpecs::default();
            extract_from_title(&l.title, &mut s);
            if let Some(d) = &l.description {
                extract_from_text(d, &mut s);
            }
            *specs = Some(s);
        }

        // Pass 2: parallel detail-page fetch for wings still missing
        // area or span.
        if !args.no_spec_fetch {
            let to_fetch: Vec<(usize, String)> = categorized
                .iter()
                .enumerate()
                .filter(|(_, (c, _, specs))| {
                    *c == Category::FrontWings
                        && specs
                            .as_ref()
                            .map(|s| s.area_cm2.is_none() || s.span_mm.is_none())
                            .unwrap_or(true)
                })
                .map(|(i, (_, l, _))| (i, l.url.clone()))
                .collect();
            let total = to_fetch.len();
            eprintln!("  enriching {total} front wing(s) in parallel (concurrency=8)…");
            use futures::stream::{self, StreamExt};
            let fetched: Vec<(usize, WingSpecs)> = stream::iter(to_fetch)
                .map(|(idx, url)| {
                    let client = client.clone();
                    async move {
                        let mut s = WingSpecs::default();
                        if let Ok(r) = client.get(&url).send().await {
                            if r.status().is_success() {
                                if let Ok(html) = r.text().await {
                                    extract_from_text(&html, &mut s);
                                    extract_from_html_table(&html, &mut s);
                                }
                            }
                        }
                        (idx, s)
                    }
                })
                .buffer_unordered(8)
                .collect()
                .await;
            for (idx, fetched_s) in fetched {
                if let Some(existing) = categorized[idx].2.as_mut() {
                    existing.area_cm2 = existing.area_cm2.or(fetched_s.area_cm2);
                    existing.span_mm = existing.span_mm.or(fetched_s.span_mm);
                    existing.aspect_ratio = existing.aspect_ratio.or(fetched_s.aspect_ratio);
                    existing.chord_mm = existing.chord_mm.or(fetched_s.chord_mm);
                }
            }
        }

        // Pass 3: compute AR + chord from area + span; drop empty specs
        // so the renderer sees `None` for wings with no useful data.
        for (c, _, specs) in categorized.iter_mut() {
            if *c != Category::FrontWings {
                continue;
            }
            if let Some(s) = specs.as_mut() {
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
            }
            let drop_specs = specs
                .as_ref()
                .map(|s| s.area_cm2.is_none() && s.span_mm.is_none())
                .unwrap_or(false);
            if drop_specs {
                *specs = None;
            }
        }

        // Board-spec pass: volume_l + length_cm from the title (and
        // body_html where the title doesn't carry a unit). No
        // detail-page fetch — boards rarely have spec tables and the
        // title regex is reliable enough.
        for (c, l, specs) in categorized.iter_mut() {
            if *c != Category::Boards {
                continue;
            }
            let mut s = specs.clone().unwrap_or_default();
            extract_board_specs(&l.title, l.description.as_deref(), &mut s);
            if s.volume_l.is_some() || s.length_cm.is_some() {
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

    sort_filter_render(&args, &output, scan_at, categorized, freshness, summary).await
}

/// Apply the `--frontwings-only` / `--boards-only` filter, sort within
/// each category by the canonical key (front wings = area DESC, boards
/// = price ASC with None last, others = price ASC), and render HTML →
/// PDF. Shared between the live-crawl path and `--from-db`.
async fn sort_filter_render(
    args: &Args,
    output: &PathBuf,
    scan_at: DateTime<Utc>,
    mut categorized: Vec<(Category, Listing, Option<WingSpecs>)>,
    freshness: std::collections::HashMap<String, Freshness>,
    summary: crawl2pump::db::UpsertSummary,
) -> Result<()> {
    if args.frontwings_only {
        categorized.retain(|(c, _, _)| *c == Category::FrontWings);
    }
    if args.boards_only {
        categorized.retain(|(c, _, _)| *c == Category::Boards);
    }

    categorized.sort_by(|a, b| {
        a.0.order().cmp(&b.0.order()).then_with(|| match a.0 {
            Category::FrontWings => {
                let ka = a.2.as_ref().and_then(|s| s.area_cm2);
                let kb = b.2.as_ref().and_then(|s| s.area_cm2);
                match (ka, kb) {
                    (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => a
                        .1
                        .price
                        .partial_cmp(&b.1.price)
                        .unwrap_or(std::cmp::Ordering::Equal),
                }
            }
            Category::Boards => match (a.1.price, b.1.price) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
            _ => a
                .1
                .price
                .partial_cmp(&b.1.price)
                .unwrap_or(std::cmp::Ordering::Equal),
        })
    });

    optimize_thumbnails(&mut categorized).await;

    let html = render_html(
        &categorized,
        &freshness,
        &summary,
        scan_at,
        if args.frontwings_only {
            Some("front-wings")
        } else if args.boards_only {
            Some("boards")
        } else {
            None
        },
    );
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

/// Thumbnails render at 44 mm × 34 mm (~520 × 400 px at 300 DPI). The
/// CDN originals are routinely 1500-2500 px, so embedding them as-is
/// pushes the PDF past 200 MB. Two strategies, one render pass:
///
/// 1. Shopify CDN (`cdn.shopify.com`) supports server-side resize via a
///    `width=` query param — we rewrite the URL in place. Chrome then
///    fetches a small JPEG instead of the original.
/// 2. For the brands that don't (Indiana, Ketos, AlpineFoil, Code, Mio,
///    Ensis), we fetch + resize + re-encode locally and inline the
///    result as a `data:image/jpeg;base64,…` URL. Fetches run through
///    `buffer_unordered(8)`, same pattern as front-wing enrichment.
///
/// On fetch / decode failure we leave the original URL in place — Chrome
/// falls back to fetching the full-size original, so the PDF is no
/// worse than it was before this step existed.
const THUMBNAIL_WIDTH: u32 = 600;

async fn optimize_thumbnails(categorized: &mut [(Category, Listing, Option<WingSpecs>)]) {
    let mut shopify_count = 0u32;
    for (_, l, _) in categorized.iter_mut() {
        if let Some(img) = l.image.as_deref() {
            if is_shopify_cdn(img) {
                l.image = Some(shopify_resize_url(img, THUMBNAIL_WIDTH));
                shopify_count += 1;
            }
        }
    }

    let to_fetch: Vec<(usize, String)> = categorized
        .iter()
        .enumerate()
        .filter_map(|(i, (_, l, _))| {
            l.image.as_ref().and_then(|u| {
                if u.is_empty() || is_shopify_cdn(u) || u.starts_with("data:") {
                    None
                } else {
                    Some((i, u.clone()))
                }
            })
        })
        .collect();

    eprintln!(
        "optimizing thumbnails: {} via Shopify URL transform, {} via local resize…",
        shopify_count,
        to_fetch.len()
    );
    if to_fetch.is_empty() {
        return;
    }

    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15")
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("warn: thumbnail client build failed: {e}");
            return;
        }
    };

    use futures::stream::{self, StreamExt};
    let results: Vec<(usize, Option<String>)> = stream::iter(to_fetch.into_iter())
        .map(|(i, url)| {
            let client = client.clone();
            async move {
                let res = fetch_and_resize_jpeg(&client, &url, THUMBNAIL_WIDTH).await;
                (i, res.ok())
            }
        })
        .buffer_unordered(8)
        .collect()
        .await;

    let mut ok = 0u32;
    let mut fail = 0u32;
    for (i, opt) in results {
        if let Some(data_url) = opt {
            categorized[i].1.image = Some(data_url);
            ok += 1;
        } else {
            fail += 1;
        }
    }
    eprintln!("  resized {ok} thumbnails, {fail} fallback to original URL");
}

fn is_shopify_cdn(url: &str) -> bool {
    url.contains("cdn.shopify.com")
}

fn shopify_resize_url(url: &str, width: u32) -> String {
    if Regex::new(r"[?&]width=\d+").unwrap().is_match(url) {
        return url.to_string();
    }
    let sep = if url.contains('?') { '&' } else { '?' };
    format!("{url}{sep}width={width}")
}

async fn fetch_and_resize_jpeg(
    client: &reqwest::Client,
    url: &str,
    width: u32,
) -> Result<String> {
    let bytes = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let img = image::load_from_memory(&bytes)
        .with_context(|| format!("decode {url}"))?;
    let resized = img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3);
    // Composite over white before dropping alpha — Indiana (and any
    // other shop using transparent PNGs) would otherwise get black
    // backgrounds in the JPEG since RGB under fully-transparent pixels
    // is undefined and often happens to be zero.
    let rgb = if resized.color().has_alpha() {
        let rgba = resized.to_rgba8();
        let (w, h) = (rgba.width(), rgba.height());
        let mut out = image::RgbImage::new(w, h);
        for (x, y, p) in rgba.enumerate_pixels() {
            let [r, g, b, a] = p.0;
            let af = a as f32 / 255.0;
            let inv = 1.0 - af;
            let blend = |c: u8| (c as f32 * af + 255.0 * inv).round().clamp(0.0, 255.0) as u8;
            out.put_pixel(x, y, image::Rgb([blend(r), blend(g), blend(b)]));
        }
        out
    } else {
        resized.to_rgb8()
    };
    let mut buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut buf);
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut cursor, 82)
        .encode(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)
        .with_context(|| format!("encode {url}"))?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}

/// Rebuild a categorized list from the most recent scan in the DB. Used
/// by `--from-db` to render the latest snapshot without re-crawling.
/// Spec columns (area / span / AR / chord) come straight from the DB so
/// no enrichment fetch is needed.
fn load_categorized_from_db(
    path: &std::path::Path,
) -> Result<Vec<(Category, Listing, Option<WingSpecs>)>> {
    let db = Db::open(path)?;
    let rows = db.latest_snapshot()?;
    Ok(rows.into_iter().map(stored_to_categorized).collect())
}

fn stored_to_categorized(s: StoredListing) -> (Category, Listing, Option<WingSpecs>) {
    let cat = s
        .category
        .as_deref()
        .and_then(Category::from_label)
        .unwrap_or(Category::Accessories);
    let region = match s.region.as_deref() {
        Some("ch") => Region::Ch,
        _ => Region::World,
    };
    let condition = match s.condition.as_deref() {
        Some("used") => Condition::Used,
        _ => Condition::New,
    };
    let listing = Listing {
        source: s.source,
        brand: s.brand,
        title: s.title,
        url: s.url,
        price: s.price,
        currency: s.currency,
        condition,
        available: s.available,
        location: s.location,
        description: s.description,
        image: s.image,
        region,
        fetched_at: s.last_seen,
    };
    let specs = if s.area_cm2.is_some()
        || s.span_mm.is_some()
        || s.aspect_ratio.is_some()
        || s.chord_mm.is_some()
    {
        Some(WingSpecs {
            area_cm2: s.area_cm2,
            span_mm: s.span_mm,
            aspect_ratio: s.aspect_ratio,
            chord_mm: s.chord_mm,
            volume_l: None,
            length_cm: None,
        })
    } else {
        None
    };
    (cat, listing, specs)
}

async fn render_from_categorized(
    args: &Args,
    output: &PathBuf,
    scan_at: DateTime<Utc>,
    categorized: Vec<(Category, Listing, Option<WingSpecs>)>,
) -> Result<()> {
    let summary = crawl2pump::db::UpsertSummary {
        new_count: 0,
        updated_count: 0,
        modified_count: 0,
        price_changes: 0,
    };
    let freshness = std::collections::HashMap::new();
    sort_filter_render(args, output, scan_at, categorized, freshness, summary).await
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
    fn from_label(s: &str) -> Option<Self> {
        match s {
            "Sets (Foil + Board)" => Some(Category::Sets),
            "Boards only" => Some(Category::Boards),
            "Foil Packs (no Board)" => Some(Category::FoilPacks),
            "Front Wings" => Some(Category::FrontWings),
            "Other Components & Accessories" => Some(Category::Accessories),
            _ => None,
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
        || u.contains("-board.")
        // Takoon's pump boards have no `board` token anywhere — naming
        // convention is `Pump Wood 80` / `Pump Carbon` / `Pump Scoot
        // Carbon`. Match `Pump <material>` at the START of the title
        // and not "Pump <foil-component>" (Foil Pump..., Pump Backpack,
        // Pump Hose Adapter — accessory_word check above handles those).
        || Regex::new(r"^pump\s+(wood|carbon|scoot|aluminium|alu|foam|epoxy)\b")
            .unwrap()
            .is_match(&t)
        // `skate` covers pump-skates (foil-pumping land trainers) — Pump
        // Zürich's "Pump Tsüri Skate", Indiana's "Hydroskate" line, and
        // any future pump-skate from another brand. The accessory_word
        // check fires first for things like the "Hydroskate Backpack",
        // so this only catches the actual board.
        || Regex::new(r"\bskate\b").unwrap().is_match(&t)
        || t.contains("hydroskate");
    // Match `kit` only as a whole word — `" kit"` would otherwise match
    // "Eco Kite" (Mio's shop tagline) and misclassify every Mio board as
    // a foil-pack set. Same for the others where a stray substring
    // could collide with a real word ("packing", "setting", "completed
    // freestyle").
    let kit_re = Regex::new(r"\b(?:kit|kits)\b").unwrap();
    let pack_re = Regex::new(r"\b(?:pack|packs|package)\b").unwrap();
    let set_re = Regex::new(r"\b(?:set|sets)\b").unwrap();
    let complete_re = Regex::new(r"\bcomplete\b").unwrap();
    let has_pack = pack_re.is_match(&t)
        || set_re.is_match(&t)
        || complete_re.is_match(&t)
        || kit_re.is_match(&t)
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
    /// Board volume in litres (Axis Pump Foilboard 24L, etc.). Only
    /// extracted for items classified as `Boards`; meaningless for
    /// front wings.
    volume_l: Option<f64>,
    /// Board length in cm — Indiana labels boards by length not
    /// volume ("Indiana 95 Pump Foil" → 95cm long).
    length_cm: Option<f64>,
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
        // Brand-prefix series whose number is the cm² area:
        //   Axis: PNG / BSC / HPS / SP / HA / ART
        //   Ketos: PUMPING / Aile Avant / EVO / UHM / HM
        //   North: MA, HA, SF, P, DW, UHA (Sonar series, sometimes
        //         followed by `v2`: "MA950v2")
        //   Armstrong: HA, MA, UHA, S1, CF, MK
        let re = Regex::new(
            r"(?i)\b(?:PNG|BSC|HPS|SP|HA|UHA|ART|MA|SF|P|DW|S1|CF|MK|PUMPING|Pumping|Aile\s+Avant|EVO|UHM|HM|F-One|FONE)\s*(\d{3,4})\b",
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
    // Variant-suffix pattern: when our Shopify expander appended a
    // bare numeric variant title to the product name (e.g. "OSPREY -
    // front wing 1850"), the trailing 3-4 digit token IS the area.
    if s.area_cm2.is_none() {
        let re = Regex::new(r"\b(\d{3,4})(?:v\d)?\s*$").unwrap();
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
    // Strip HTML tags first so labels embedded in markup
    // (`<strong>Front wing span cm:</strong> 83.5`) reduce to plain
    // "Front wing span cm: 83.5" text. JSON state blobs that ship
    // values like `"aspect_ratio":1` survive stripping, so the per-
    // pattern loop iterates matches and prefers values inside the
    // expected sanity range.
    let cleaned: String = {
        let no_tags = Regex::new(r"<[^>]+>").unwrap().replace_all(text, " ");
        // Collapse whitespace runs introduced by tag removal so `]{0,8}`
        // gaps still match `</strong> ` → ` `.
        Regex::new(r"\s+")
            .unwrap()
            .replace_all(&no_tags, " ")
            .to_string()
    };
    let text = cleaned.as_str();

    if s.area_cm2.is_none() {
        for re in [
            // Labelled (English / French / German). Allow up to 15
            // non-digit chars between label and number so connectives
            // like "von " / "of " / ": " all work. Naish prints
            // "Projected surface area cm2:" — matches via `projected`
            // prefix + `surface area`.
            Regex::new(
                r"(?i)(?:projected\s+|projizierte\s+)?(?:fl[aä]che|surface(?:\s+area)?|area|aire)[^0-9\n]{0,20}(\d{3,4})\s*(?:cm[²2]|sq\s*cm)?",
            )
            .unwrap(),
            // Bare "NNN cm²" — used as a last-resort heuristic.
            Regex::new(r"(\d{3,4})\s*cm[²2]").unwrap(),
        ] {
            for c in re.captures_iter(text) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (200.0..=2700.0).contains(&v) {
                        s.area_cm2 = Some(v);
                        break;
                    }
                }
            }
            if s.area_cm2.is_some() {
                break;
            }
        }
    }
    if s.span_mm.is_none() {
        // First try: explicit "span cm" or "Front wing span cm" — Naish
        // prints span in centimetres (e.g. `Front wing span cm: 83.5`),
        // so we capture the float, convert to mm.
        let span_cm_re = Regex::new(
            r"(?i)(?:front\s+wing\s+)?(?:wing\s*)?span\s*cm[^0-9\n]{0,15}(\d{2,3}(?:\.\d{1,2})?)",
        )
        .unwrap();
        for c in span_cm_re.captures_iter(text) {
            if let Ok(v) = c[1].parse::<f64>() {
                if (30.0..=250.0).contains(&v) {
                    s.span_mm = Some(v * 10.0);
                    break;
                }
            }
        }
    }
    if s.span_mm.is_none() {
        for re in [
            // Labelled with mm explicit — most reliable signal.
            Regex::new(
                r"(?i)(?:wingspan|spannweite|envergure|span)[^0-9\n]{0,20}(\d{3,4})\s*mm",
            )
            .unwrap(),
            // Labelled without mm — slightly riskier (could match a
            // chord) but bounded to 300–2500.
            Regex::new(
                r"(?i)(?:wingspan|spannweite|envergure)[^0-9\n]{0,20}(\d{3,4})\b",
            )
            .unwrap(),
            // Reverse form: "1696 mm wingspan".
            Regex::new(r"\b(\d{3,4})\s*mm\s*(?:wingspan|span|spannweite)").unwrap(),
        ] {
            for c in re.captures_iter(text) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (300.0..=2500.0).contains(&v) {
                        s.span_mm = Some(v);
                        break;
                    }
                }
            }
            if s.span_mm.is_some() {
                break;
            }
        }
    }
    if s.aspect_ratio.is_none() {
        for re in [
            // Naish prints `Aspect_ratio: 5.6` with an underscore — the
            // `[\s_]+` clause catches both that and the regular "aspect
            // ratio" with a space. Require a colon (or end-of-label
            // whitespace then digit) immediately after `ratio` so we
            // don't latch onto Naish's JSON state blob `"aspect_ratio":
            // true, "img_aspect_ratio": 3.698` — that form has a `"`
            // between `ratio` and the colon, which the `\s*:` clause
            // rejects.
            Regex::new(r"(?i)\baspect[\s_]+ratio\s*:[^0-9\n]{0,10}(\d{1,2}(?:\.\d{1,2})?)").unwrap(),
            Regex::new(r"(?i)\baspect[\s_]+ratio\s+of\s+(\d{1,2}(?:\.\d{1,2})?)").unwrap(),
            Regex::new(r"(?i)\bAR[\s:=]+(\d{1,2}(?:\.\d{1,2})?)").unwrap(),
            Regex::new(r"(?i)(\d{1,2}(?:\.\d{1,2})?)\s*aspect[\s_]+ratio").unwrap(),
        ] {
            for c in re.captures_iter(text) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (3.0..=15.0).contains(&v) {
                        s.aspect_ratio = Some(v);
                        break;
                    }
                }
            }
            if s.aspect_ratio.is_some() {
                break;
            }
        }
    }
    if s.chord_mm.is_none() {
        for re in [
            // Indiana: "Chord von 173 mm". The 0–20 char gap absorbs
            // "von "/"of "/": "/etc.
            Regex::new(r"(?i)chord[^0-9\n]{0,20}(\d{2,4})\s*mm").unwrap(),
            Regex::new(r"(?i)chord[^0-9\n]{0,20}(\d{2,4})\b").unwrap(),
        ] {
            for c in re.captures_iter(text) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (50.0..=400.0).contains(&v) {
                        s.chord_mm = Some(v);
                        break;
                    }
                }
            }
            if s.chord_mm.is_some() {
                break;
            }
        }
    }
}

/// Pull volume (litres) and length (cm) from a board's title /
/// description. Patterns:
/// - `Pump Foilboard 24L`, `Froth Carbon Foilboard 45L` (Axis)
/// - `24 litres`, `90 liter` (description prose)
/// - `Indiana 95 Pump Foil`, `Indiana 105 Pump Foil` — bare 2-3 digit
///   number = length in cm (no L suffix)
/// - `Pocket Pro carbone 78 x 42` (Alpinefoil) — first number = length
fn extract_board_specs(title: &str, description: Option<&str>, s: &mut WingSpecs) {
    let texts: Vec<&str> = std::iter::once(title)
        .chain(description.into_iter())
        .collect();

    if s.volume_l.is_none() {
        for t in &texts {
            // Direct N{2,3}L / N{2,3} L (e.g. "24L", "45 L")
            let re = Regex::new(r"\b(\d{2,3})\s?[Ll](?:itres?|iters?)?\b").unwrap();
            if let Some(c) = re.captures(t) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (10.0..=300.0).contains(&v) {
                        s.volume_l = Some(v);
                        break;
                    }
                }
            }
            // "Volume: 24" / "volume 24" (no unit)
            let re2 = Regex::new(r"(?i)volume\s*[:=]?\s*(\d{2,3})\b").unwrap();
            if let Some(c) = re2.captures(t) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (10.0..=300.0).contains(&v) {
                        s.volume_l = Some(v);
                        break;
                    }
                }
            }
        }
    }

    if s.length_cm.is_none() {
        // Indiana "Indiana 95 Pump Foil" / Ketos "Board Pumping
        // Dockstart 90". Match a bare 2-3 digit number followed by
        // `pump`, `foil`, or end-of-token. Avoid matching a 4-digit
        // wing area we may have left in the title.
        for t in &texts {
            let re = Regex::new(r"\b(\d{2,3})\s+(?:pump|foil|dockstart|carbon|board)").unwrap();
            if let Some(c) = re.captures(&t.to_lowercase()) {
                if let Ok(v) = c[1].parse::<f64>() {
                    if (40.0..=200.0).contains(&v) {
                        s.length_cm = Some(v);
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
    only: Option<&str>,
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

    let title = match only {
        Some("front-wings") => "Pumpfoil Front Wings",
        Some("boards") => "Pumpfoil Boards",
        _ => "Pumpfoil Catalog",
    };
    let mode = match only {
        Some("front-wings") => " · front wings only",
        Some("boards") => " · boards only",
        _ => "",
    };
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
        format!(r#"<a class="thumb-link" href="{url}" target="_blank" rel="noopener"><img class="thumb" src="{img}" loading="lazy"/></a>"#)
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
    <a class="title" href="{url}" target="_blank" rel="noopener">{title}</a>
    <div class="meta">{meta}</div>
    <p class="desc">{desc}</p>
    <a class="url" href="{url}" target="_blank" rel="noopener">{url}</a>
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
    if let Some(v) = s.volume_l { bits.push(format!("{} L", v as i64)); }
    if let Some(l) = s.length_cm { bits.push(format!("{} cm", l as i64)); }
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
