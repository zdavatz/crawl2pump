//! `sensor_report` — scan electronics distributors for the
//! MovementLogger hardware BOM (`crawl2pump::sensors`) + the curated
//! USB-C / open-source-firmware pluggable modules, persist to SQLite,
//! render a categorized PDF + HTML.
//!
//! Sibling of `pumpfoil_report` but part-list driven instead of
//! query/brand driven. Distributors: ST + SparkFun (scrape, no key),
//! Mouser + DigiKey + Farnell (API, keys in `.sensors.env`; skip
//! cleanly when absent).
//!
//! Usage:
//!   ./target/release/sensor_report                  # ~/Downloads/sensors.pdf
//!   ./target/release/sensor_report --oss-only       # only OSS-firmware parts
//!   ./target/release/sensor_report --usbc-only      # only USB-C pluggable
//!   ./target/release/sensor_report --from-db        # re-render, no crawl
use anyhow::{anyhow, Context, Result};
use base64::Engine;
use chrono::{DateTime, Utc};
use clap::Parser;
use crawl2pump::db::{Db, ListingRow, StoredListing};
use crawl2pump::listing::{Condition, Listing, Region};
use crawl2pump::sensors::{bom, Connector, Feature, Part, Role};
use crawl2pump::sources::distributors::{crawl_all, load_sensors_env, SensorOffer};
use futures::stream::{self, StreamExt};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

const CHROME_MAC: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const SENSORS_DB: &str = "sqlite/sensors.db";
const THUMB_W: u32 = 600;

#[derive(Parser, Debug)]
#[command(version, about = "Scan distributors for the sensor/module BOM, persist to SQLite, render PDF")]
struct Args {
    /// PDF output path. Default: ~/Downloads/sensors.pdf
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// SQLite path. Default: ./sqlite/sensors.db
    #[arg(long, default_value = SENSORS_DB)]
    db: PathBuf,
    /// Skip the live crawl and re-render from whatever's in the DB.
    #[arg(long)]
    from_db: bool,
    /// Only parts whose firmware is fully open-source.
    #[arg(long)]
    oss_only: bool,
    /// Only USB-C (host-pluggable) modules.
    #[arg(long)]
    usbc_only: bool,
    /// Render only these BOM part keys (CSV, e.g.
    /// `lilygo-tbeam-s3-supreme,sparkfun-xm125-radar`). Post-DB
    /// filter like --oss-only: the DB keeps the full scan; only the
    /// rendered PDF narrows. Use with --output for a focused sheet.
    #[arg(long)]
    keys: Option<String>,
    /// `.sensors.env` path (Mouser/DigiKey/Farnell API keys).
    #[arg(long, default_value = ".sensors.env")]
    env_file: PathBuf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Freshness {
    New,
    Modified,
}

/// Unified render row — both the live crawl and `--from-db` collapse
/// to this so the renderer has one path.
struct Row {
    role: Role,
    part_name: String,
    oss: bool,
    connector: Connector,
    /// Buyer-facing capability checkboxes (from the BOM part).
    features: &'static [Feature],
    /// EU/CH reseller buy links (from the BOM part), usually empty.
    resellers: &'static [(&'static str, &'static str)],
    /// Physical (L, B, H) in cm (from the BOM part).
    dimensions: Option<(f32, f32, f32)>,
    /// Canonical OSS-firmware GitHub repo (from the BOM part).
    firmware_repo: Option<&'static str>,
    /// Host-MCU comparable one-liner (from the BOM part), if any.
    mcu: Option<&'static str>,
    listing: Listing,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();
    let output = args
        .output
        .clone()
        .unwrap_or_else(|| dirs_downloads().join("sensors.pdf"));

    let parts = bom();
    // name → (oss, connector, role) so the from-db path can recover
    // badge metadata that isn't stored in the generic listings table.
    let meta: HashMap<&str, (&Part,)> = parts.iter().map(|p| (p.name, (p,))).collect();

    let scan_at = Utc::now();
    let (rows, summary) = if args.from_db {
        let r = load_from_db(&args.db, &meta)?;
        (r, None)
    } else {
        let client = reqwest::Client::builder()
            .user_agent(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
                 AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
            )
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        load_sensors_env(&args.env_file);

        eprintln!("scanning {} parts across 6 distributors…", parts.len());
        let offers = crawl_all(&client, &parts).await;
        eprintln!("  → {} offers total", offers.len());

        // Persist (category = role label).
        let listing_rows: Vec<ListingRow> = offers
            .iter()
            .map(|o| ListingRow::from_listing(&o.listing, Some(o.role.label())))
            .collect();
        let mut db = Db::open(&args.db).context("open sensors db")?;
        let s = db.upsert_scan(scan_at, &listing_rows)?;
        eprintln!(
            "  db: {} new · {} modified · {} touched · {} price changes",
            s.new_count, s.modified_count, s.updated_count, s.price_changes
        );
        let rows = offers
            .into_iter()
            .map(|o| offer_to_row(o, &meta))
            .collect::<Vec<_>>();
        (rows, Some(s))
    };

    // Apply post-classification filters (DB always holds the full BOM;
    // only the rendered PDF narrows — same contract as pumpfoil_report).
    // --keys is by BOM `key`; Row carries part_name, so resolve the
    // requested keys to part names via the BOM once.
    let key_names: Option<std::collections::HashSet<String>> = args.keys.as_ref().map(|csv| {
        let want: std::collections::HashSet<&str> =
            csv.split(',').map(str::trim).filter(|s| !s.is_empty()).collect();
        bom()
            .into_iter()
            .filter(|p| want.contains(p.key))
            .map(|p| p.name.to_string())
            .collect()
    });
    let mut rows: Vec<Row> = rows
        .into_iter()
        .filter(|r| !args.oss_only || r.oss)
        .filter(|r| !args.usbc_only || r.connector == Connector::UsbC)
        .filter(|r| key_names.as_ref().is_none_or(|n| n.contains(&r.part_name)))
        .collect();

    if rows.is_empty() {
        eprintln!("no rows to render (empty DB? run without --from-db first)");
        return Ok(());
    }

    // Freshness: rows that appeared / changed in the last 7 days.
    let cutoff = scan_at - chrono::Duration::days(7);
    let freshness = {
        let db = Db::open(&args.db)?;
        let mut m = HashMap::new();
        for l in db.new_since(cutoff)? {
            m.insert(l.url, Freshness::New);
        }
        for l in db.modified_since(cutoff)? {
            m.entry(l.url).or_insert(Freshness::Modified);
        }
        m
    };
    let summary = summary.unwrap_or_else(|| crawl2pump::db::UpsertSummary {
        new_count: freshness.values().filter(|f| **f == Freshness::New).count(),
        updated_count: 0,
        modified_count: freshness
            .values()
            .filter(|f| **f == Freshness::Modified)
            .count(),
        price_changes: 0,
    });

    optimize_thumbnails(&mut rows, &args.db).await;

    // Sort: role order, then part name, then price asc (None last).
    rows.sort_by(|a, b| {
        a.role
            .order()
            .cmp(&b.role.order())
            .then_with(|| a.part_name.cmp(&b.part_name))
            .then_with(|| match (a.listing.price, b.listing.price) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
    });

    let html = render_html(&rows, &freshness, &summary, scan_at, args.oss_only, args.usbc_only);
    let html_path = output.with_extension("html");
    std::fs::write(&html_path, &html)?;
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

fn offer_to_row(o: SensorOffer, meta: &HashMap<&str, (&Part,)>) -> Row {
    let (oss, connector, features, resellers, dimensions, firmware_repo, mcu) = meta
        .get(o.part_name)
        .map(|(p,)| {
            (
                p.oss_firmware,
                p.connector,
                p.features(),
                p.resellers(),
                p.dimensions_cm(),
                p.firmware_repo(),
                p.mcu(),
            )
        })
        .unwrap_or((false, Connector::Soldered, &[][..], &[][..], None, None, None));
    Row {
        role: o.role,
        part_name: o.part_name.to_string(),
        oss,
        connector,
        features,
        resellers,
        dimensions,
        firmware_repo,
        mcu,
        listing: o.listing,
    }
}

/// Rebuild rows from the latest per-source DB snapshot. The generic
/// `listings` table doesn't carry OSS / connector — we recover those
/// from the BOM by matching the part-name prefix of the stored title
/// (`"<part.name> · <mpn> @ <distributor>"`).
fn load_from_db(path: &std::path::Path, meta: &HashMap<&str, (&Part,)>) -> Result<Vec<Row>> {
    let db = Db::open(path)?;
    let rows = db.latest_snapshot()?;
    Ok(rows
        .into_iter()
        .filter_map(|s| stored_to_row(s, meta))
        .collect())
}

fn stored_to_row(s: StoredListing, meta: &HashMap<&str, (&Part,)>) -> Option<Row> {
    let role = s.category.as_deref().and_then(Role::from_label)?;
    let part_name = s
        .title
        .split(" · ")
        .next()
        .unwrap_or(&s.title)
        .trim()
        .to_string();
    let (oss, connector, features, resellers, dimensions, firmware_repo, mcu) = meta
        .get(part_name.as_str())
        .map(|(p,)| {
            (
                p.oss_firmware,
                p.connector,
                p.features(),
                p.resellers(),
                p.dimensions_cm(),
                p.firmware_repo(),
                p.mcu(),
            )
        })
        .unwrap_or((false, Connector::Soldered, &[][..], &[][..], None, None, None));
    let listing = Listing {
        source: s.source,
        brand: s.brand,
        title: s.title,
        url: s.url,
        price: s.price,
        currency: s.currency,
        condition: Condition::New,
        available: s.available,
        location: s.location,
        description: s.description,
        image: s.image,
        region: match s.region.as_deref() {
            Some("ch") => Region::Ch,
            _ => Region::World,
        },
        fetched_at: s.last_seen,
    };
    Some(Row {
        role,
        part_name,
        oss,
        connector,
        features,
        resellers,
        dimensions,
        firmware_repo,
        mcu,
        listing,
    })
}

/// Inline every offer image as a resized base64 JPEG (cached in the
/// `image_cache` table) so the PDF is self-contained and small.
/// Mirrors pumpfoil_report's pass minus the AVIF/magick fallback —
/// distributor product photos are plain JPEG/PNG/WebP.
async fn optimize_thumbnails(rows: &mut [Row], db_path: &std::path::Path) {
    let cache = Db::open(db_path).ok();
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 sensor_report")
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .ok();
    let Some(client) = client else { return };

    let jobs: Vec<(usize, String)> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let u = r.listing.image.as_deref()?.trim();
            if u.is_empty() || u.starts_with("data:") {
                None
            } else {
                Some((i, u.to_string()))
            }
        })
        .collect();

    let results: Vec<(usize, Option<String>)> = stream::iter(jobs)
        .map(|(i, url)| {
            let client = &client;
            let cache = &cache;
            async move {
                if let Some(c) = cache {
                    if let Ok(Some(hit)) = c.get_cached_image(&url) {
                        return (i, Some(hit));
                    }
                }
                match fetch_resize_jpeg(client, &url).await {
                    Ok(data) => {
                        if let Some(c) = cache {
                            let _ = c.put_cached_image(&url, &data);
                        }
                        (i, Some(data))
                    }
                    Err(_) => (i, None),
                }
            }
        })
        .buffer_unordered(6)
        .collect()
        .await;

    for (i, data) in results {
        // Assign unconditionally — including the failure case (`None`).
        // Distributor image-by-MPN URLs sometimes return HTTP 200 with
        // an HTML "not found" page instead of a JPEG (e.g. Mouser's
        // STM32U585AII6_SPL.jpg). `image::load_from_memory` then fails,
        // and *keeping* the original URL makes Chrome render a
        // broken-image glyph at print time. Dropping it to None lets
        // the cross-offer fallback / SVG placeholder fire instead, so a
        // card is never a broken tile. Every job's image is now either
        // a verified inlined data: URL or None — no remote URLs survive
        // to the render.
        rows[i].listing.image = data;
    }
}

async fn fetch_resize_jpeg(client: &reqwest::Client, url: &str) -> Result<String> {
    let bytes = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    let img = image::load_from_memory(&bytes)?;
    let resized = img.resize(THUMB_W, u32::MAX, image::imageops::FilterType::Lanczos3);
    let rgb = if resized.color().has_alpha() {
        let rgba = resized.to_rgba8();
        let mut out = image::RgbImage::new(rgba.width(), rgba.height());
        for (x, y, p) in rgba.enumerate_pixels() {
            let [r, g, b, a] = p.0;
            let af = a as f32 / 255.0;
            let inv = 1.0 - af;
            let bl = |c: u8| (c as f32 * af + 255.0 * inv).round().clamp(0.0, 255.0) as u8;
            out.put_pixel(x, y, image::Rgb([bl(r), bl(g), bl(b)]));
        }
        out
    } else {
        resized.to_rgb8()
    };
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut std::io::Cursor::new(&mut buf), 82)
        .encode(rgb.as_raw(), rgb.width(), rgb.height(), image::ExtendedColorType::Rgb8)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&buf)
    ))
}

/// Wiring/build guide, emitted only when the rendered set contains
/// BOTH the LilyGO T-Beam S3 Supreme and the SparkFun XM125 radar —
/// i.e. the focused build sheet. Static, hand-verified against the
/// LilyGo-LoRa-Series hw doc + SparkFun Qwiic colour convention.
fn build_guide_html(rows: &[Row]) -> Option<String> {
    let has = |needle: &str| rows.iter().any(|r| r.part_name.contains(needle));
    if !(has("LilyGO T-Beam") && has("XM125")) {
        return None;
    }
    Some(
        r#"<div class="guide">
<h2 class="cat">Build &amp; Wiring — LilyGO T-Beam S3 Supreme + XM125 radar</h2>
<p class="g-h">1 · Power / battery</p>
<ul>
<li>Insert one <b>charged 18650 Li-ion cell (flat-top, ∅18 × 65 mm)</b> into the LilyGO's onboard holder — mind polarity. Not included; order separately.</li>
<li>USB-C charges the 18650 and runs the board via the onboard AXP2101 PMU. On the water run untethered off the 18650; recharge over USB-C between sessions.</li>
</ul>
<p class="g-h">2 · Radar wiring — Qwiic female-jumper (SparkFun colour code)</p>
<ul>
<li>Plug the <b>Qwiic end</b> of the female-jumper cable into the <b>XM125's Qwiic port</b>.</li>
<li>Wire the 4 female leads to the LilyGO's <b>real I²C header (GPIO17/18)</b> — <b>NOT</b> the connector labelled "QWIIC socket" (that is UART1, GPIO43/44, and will not work):</li>
</ul>
<table class="wire">
<tr><th>Lead</th><th>XM125 (Qwiic)</th><th>→ LilyGO pin</th></tr>
<tr><td>Black</td><td>GND</td><td>GND</td></tr>
<tr><td>Red</td><td>3.3 V</td><td>3V3 (peripheral rail)</td></tr>
<tr><td>Blue</td><td>SDA</td><td>GPIO 17 (SDA)</td></tr>
<tr><td>Yellow</td><td>SCL</td><td>GPIO 18 (SCL)</td></tr>
</table>
<p class="g-n">XM125 default I²C address 0x52 — no clash with the onboard QMC6310 / BME280 / OLED, so it shares the bus fine. Zero soldering: Qwiic plug one end, push-on header pins the other.</p>
<p class="g-h">3 · Firmware</p>
<ul>
<li>Firmware must <b>enable the AXP2101 peripheral/sensor power rail before any I²C access</b> (the LilyGO I²C bus hangs otherwise), init I²C on GPIO17/18, then drive the XM125 via the Acconeer A121 / SparkFun XM125 library.</li>
<li>Stock Meshtastic does <b>not</b> read the radar — use custom firmware (movement_logger_firmware or an Arduino-ESP32 sketch).</li>
</ul>
<p class="g-h">4 · Enclosure</p>
<ul>
<li>Plastic (RF-transparent) waterproof box — the radar ranges <b>through</b> the wall, so the whole stack can be fully sealed. <b>No metal/carbon</b> case (blocks radar + GPS). If you keep the LPS22DF barometer, add a Gore-type vent membrane.</li>
</ul>
</div>
"#
        .to_string(),
    )
}

fn render_html(
    rows: &[Row],
    freshness: &HashMap<String, Freshness>,
    summary: &crawl2pump::db::UpsertSummary,
    scan_at: DateTime<Utc>,
    oss_only: bool,
    usbc_only: bool,
) -> String {
    let today = scan_at.format("%Y-%m-%d %H:%M UTC").to_string();

    // Role → part_name → offers, preserving the sorted order.
    let mut roles: Vec<Role> = Vec::new();
    for r in rows {
        if !roles.contains(&r.role) {
            roles.push(r.role);
        }
    }
    roles.sort_by_key(|r| r.order());

    let mut body = String::new();
    let total = rows.len();
    // Lead with a wiring/build guide when the rendered set is the
    // LilyGO + XM125 radar build (both present). It's the answer to
    // "how do I actually connect this so it works with battery +
    // power + radar" and is the load-bearing reason --keys exists.
    if let Some(g) = build_guide_html(rows) {
        body.push_str(&g);
    }
    for role in &roles {
        let role_rows: Vec<&Row> = rows.iter().filter(|r| r.role == *role).collect();
        body.push_str(&format!(
            r#"<h2 class="cat">{} <span class="cat-count">{}</span></h2>"#,
            html_escape(role.label()),
            role_rows.len()
        ));
        // Group consecutive rows by part (they're already sorted by name).
        let mut idx = 0;
        while idx < role_rows.len() {
            let name = &role_rows[idx].part_name;
            let group: Vec<&&Row> = role_rows[idx..]
                .iter()
                .take_while(|r| &r.part_name == name)
                .collect();
            idx += group.len();
            let head = &group[0];
            let mut badges = String::new();
            if head.oss {
                badges.push_str(r#"<span class="tag oss">OSS firmware</span>"#);
            } else {
                badges.push_str(r#"<span class="tag closed">closed blob</span>"#);
            }
            if head.connector == Connector::UsbC {
                badges.push_str(r#"<span class="tag usbc">USB-C pluggable</span>"#);
            } else {
                badges.push_str(&format!(
                    r#"<span class="tag conn">{}</span>"#,
                    html_escape(head.connector.label())
                ));
            }
            body.push_str(&format!(
                r#"<h3 class="part">{} {}</h3>"#,
                html_escape(name),
                badges
            ));
            // Per-part capability checkbox row (features are a property
            // of the part, not the distributor offer — render once per
            // part, not per card). Always shows all six in fixed order
            // so columns line up visually down the page.
            let mut feats = String::from(r#"<div class="feats">"#);
            for f in Feature::ALL {
                let has = head.features.contains(&f);
                feats.push_str(&format!(
                    r#"<span class="feat {}">{} {}</span>"#,
                    if has { "yes" } else { "no" },
                    if has { "☑" } else { "☐" },
                    html_escape(f.label())
                ));
            }
            if let Some((l, b, h)) = head.dimensions {
                feats.push_str(&format!(
                    r#"<span class="dim">L×B×H {} × {} × {} cm</span>"#,
                    fmt_cm(l),
                    fmt_cm(b),
                    fmt_cm(h)
                ));
            }
            feats.push_str("</div>");
            body.push_str(&feats);
            // Host-MCU line — only parts with a user-programmable MCU
            // (boards + the STM32U585); lets a reader compare chips
            // (STM32U585 ultra-low-power vs ESP32 dual-core+WiFi).
            if let Some(mcu) = head.mcu {
                body.push_str(&format!(
                    r#"<div class="mcu"><span class="mcu-l">MCU:</span> {}</div>"#,
                    html_escape(mcu)
                ));
            }
            // OSS firmware repo for this device (every BOM part is
            // open-firmware, so this always renders). One line per
            // part, like the feature/buy rows.
            if let Some(repo) = head.firmware_repo {
                body.push_str(&format!(
                    r#"<div class="fw"><span class="fw-l">OSS firmware:</span> <a href="{0}" target="_blank" rel="noopener">{0}</a></div>"#,
                    html_escape(repo)
                ));
            }
            // EU/CH reseller buy links — only emitted for parts that
            // carry them (boards no API distributor stocks, e.g. the
            // LilyGO all-in-one). Once per part, like the feature row.
            if !head.resellers.is_empty() {
                let mut buy = String::from(r#"<div class="buy"><span class="buy-l">Buy (EU/CH):</span> "#);
                let links: Vec<String> = head
                    .resellers
                    .iter()
                    .map(|(label, url)| {
                        format!(
                            r#"<a href="{}" target="_blank" rel="noopener">{}</a>"#,
                            html_escape(url),
                            html_escape(label)
                        )
                    })
                    .collect();
                buy.push_str(&links.join(" · "));
                buy.push_str("</div>");
                body.push_str(&buy);
            }
            // Cross-offer image fallback: if any offer for this part
            // carries a real photo (after thumbnail inlining), reuse it
            // for offers that have none — typically the price-less ST
            // reference card for a bare sensor IC, which would otherwise
            // render the generic placeholder right next to a sibling
            // card showing the real part photo. The placeholder then
            // only fires when *no* distributor found a photo.
            // Only a verified-inlined data: URL is safe to share — a
            // bare remote URL in the group might itself be a dead
            // image-by-MPN link. After optimize_thumbnails, failed
            // fetches are None, so in practice the group holds only
            // data: URLs or None; this filter is the explicit contract.
            let shared_img: Option<String> = group.iter().find_map(|r| {
                r.listing
                    .image
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| s.starts_with("data:"))
                    .map(str::to_string)
            });
            for r in &group {
                let mut listing = r.listing.clone();
                let has_own = listing
                    .image
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|s| !s.is_empty());
                if !has_own {
                    listing.image = shared_img.clone();
                }
                body.push_str(&render_card(
                    &listing,
                    freshness.get(&listing.url).copied(),
                ));
            }
        }
    }

    let scope = match (oss_only, usbc_only) {
        (true, true) => " · OSS + USB-C only",
        (true, false) => " · OSS firmware only",
        (false, true) => " · USB-C pluggable only",
        _ => "",
    };

    format!(
        r#"<!doctype html>
<html lang="de">
<head>
<meta charset="utf-8">
<title>MovementLogger Sensor/Module Catalog · {today}</title>
<style>
  @page {{ size: A4; margin: 12mm 12mm 14mm; }}
  * {{ box-sizing: border-box; }}
  body {{ font-family: -apple-system, "Helvetica Neue", Arial, sans-serif; font-size: 10pt; color: #111; margin: 0; }}
  h1 {{ font-size: 16pt; margin: 0 0 2mm; }}
  h2.cat {{ font-size: 13pt; margin: 8mm 0 2mm; padding: 1mm 0; border-bottom: 2px solid #0a58ca; color: #0a58ca; page-break-after: avoid; }}
  h2.cat .cat-count {{ color: #888; font-size: 10pt; font-weight: 400; }}
  h3.part {{ font-size: 11pt; margin: 5mm 0 1mm; color: #222; page-break-after: avoid; }}
  .tag {{ display: inline-block; font-size: 7.5pt; font-weight: 700; padding: 0.5mm 1.5mm; border-radius: 1mm; margin-left: 2mm; vertical-align: 1pt; }}
  .tag.oss   {{ background: #d1f0d6; color: #0e6132; }}
  .tag.closed{{ background: #f8d7da; color: #842029; }}
  .tag.usbc  {{ background: #cfe2ff; color: #084298; }}
  .tag.conn  {{ background: #eee; color: #555; }}
  .feats {{ margin: 0 0 2mm; page-break-after: avoid; }}
  .feat {{ display: inline-block; font-size: 8pt; font-weight: 600; padding: 0.5mm 2mm; border-radius: 1mm; margin: 0 1.5mm 1mm 0; border: 1px solid #d5d5d5; }}
  .feat.yes {{ background: #d1f0d6; color: #0e6132; border-color: #9ed8ab; }}
  .feat.no  {{ background: #f6f6f6; color: #aaa; }}
  .dim {{ display: inline-block; font-size: 8pt; font-weight: 600; padding: 0.5mm 2mm; border-radius: 1mm; margin: 0 1.5mm 1mm 0; background: #e7eefc; color: #234; border: 1px solid #c5d4f3; }}
  .buy {{ font-size: 8.5pt; margin: 0 0 3mm; line-height: 1.5; }}
  .buy-l {{ font-weight: 700; color: #444; margin-right: 1mm; }}
  .buy a {{ color: #0a58ca; text-decoration: none; white-space: nowrap; }}
  .fw {{ font-size: 8.5pt; margin: 0 0 2mm; }}
  .fw-l {{ font-weight: 700; color: #0e6132; margin-right: 1mm; }}
  .fw a {{ color: #0a58ca; text-decoration: none; word-break: break-all; }}
  .mcu {{ font-size: 8.5pt; margin: 0 0 2mm; color: #222; }}
  .mcu-l {{ font-weight: 700; color: #6a1b9a; margin-right: 1mm; }}
  .guide {{ border: 1px solid #c5d4f3; background: #f5f8ff; border-radius: 2mm; padding: 3mm 4mm; margin: 0 0 6mm; break-inside: avoid; }}
  .guide h2.cat {{ margin-top: 0; }}
  .guide .g-h {{ font-weight: 700; color: #0a58ca; margin: 3mm 0 1mm; font-size: 10pt; }}
  .guide ul {{ margin: 0 0 1mm; padding-left: 5mm; }}
  .guide li {{ font-size: 9pt; margin: 0.5mm 0; line-height: 1.35; }}
  .guide .g-n {{ font-size: 8pt; color: #555; margin: 1mm 0 0; }}
  table.wire {{ border-collapse: collapse; margin: 1mm 0 1mm; font-size: 9pt; }}
  table.wire th, table.wire td {{ border: 1px solid #c5d4f3; padding: 0.8mm 3mm; text-align: left; }}
  table.wire th {{ background: #e7eefc; }}
  .sub {{ color: #555; font-size: 9pt; margin-bottom: 2mm; }}
  .diff {{ color: #444; font-size: 9pt; margin-bottom: 6mm; }}
  .diff .pill {{ display: inline-block; padding: 0.5mm 2mm; border-radius: 1mm; margin-right: 2mm; }}
  .pill-new      {{ background: #d1f0d6; color: #0e6132; }}
  .pill-modified {{ background: #fff3cd; color: #856404; }}
  .card {{ display: grid; grid-template-columns: 38mm 1fr 28mm; gap: 5mm; padding: 3mm 0; border-bottom: 1px solid #e5e5e5; break-inside: avoid; align-items: start; }}
  .badge {{ display: inline-block; font-size: 7.5pt; font-weight: 700; padding: 0.5mm 1.5mm; border-radius: 1mm; margin-right: 2mm; }}
  .badge.new      {{ background: #0e6132; color: white; }}
  .badge.modified {{ background: #f0ad4e; color: white; }}
  .thumb {{ width: 38mm; height: 30mm; object-fit: contain; border: 1px solid #ddd; border-radius: 3px; background: #fff; }}
  .title {{ display: inline-block; font-size: 10.5pt; font-weight: 600; color: #0a58ca; text-decoration: none; }}
  .meta {{ color: #666; font-size: 8.5pt; margin: 1mm 0 1.5mm; }}
  .desc {{ margin: 0; color: #222; font-size: 8.5pt; line-height: 1.3; }}
  a.url {{ display: inline-block; color: #0a58ca; font-size: 7.5pt; margin-top: 1mm; word-break: break-all; }}
  .price {{ text-align: right; font-variant-numeric: tabular-nums; font-weight: 700; font-size: 11pt; white-space: nowrap; }}
</style>
</head>
<body>
<h1>MovementLogger — Sensor &amp; Module Catalog</h1>
<div class="sub">{total} Angebote{scope} · {today} · BOM aus movement_logger_firmware · via crawl2pump</div>
<div class="diff">
  <span class="pill pill-new">{} neu (7 Tage)</span>
  <span class="pill pill-modified">{} aktualisiert</span>
  · {} touched · {} price changes
</div>
{body}
</body>
</html>
"#,
        summary.new_count, summary.modified_count, summary.updated_count, summary.price_changes,
    )
}

fn render_card(l: &Listing, freshness: Option<Freshness>) -> String {
    let price = match (l.price, l.currency.as_deref()) {
        (Some(p), Some(c)) => format!("{c} {:.2}", p),
        (Some(p), None) => format!("{:.2}", p),
        _ => "—".into(),
    };
    // "Always an image": real photo when a source found one, otherwise
    // a generated placeholder so no card ever renders a blank tile.
    let owned_img;
    let img = match l.image.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(s) => s,
        None => {
            owned_img = placeholder_data_url(&l.title);
            &owned_img
        }
    };
    let url = html_escape(&l.url);
    let title = html_escape(&l.title);
    let desc = l
        .description
        .as_deref()
        .map(|s| html_escape(&shorten(s, 240)))
        .unwrap_or_default();
    let stock = match l.available {
        Some(true) => " · in stock",
        Some(false) => " · out of stock",
        None => "",
    };
    let img_html = format!(
        r#"<a href="{url}" target="_blank" rel="noopener"><img class="thumb" src="{img}"/></a>"#
    );
    let badge = match freshness {
        Some(Freshness::New) => r#"<span class="badge new">neu</span>"#,
        Some(Freshness::Modified) => r#"<span class="badge modified">aktualisiert</span>"#,
        None => "",
    };
    format!(
        r#"<section class="card">
  {img_html}
  <div class="body">
    <a class="title" href="{url}" target="_blank" rel="noopener">{badge}{title}</a>
    <div class="meta">{}{stock}</div>
    <p class="desc">{desc}</p>
    <a class="url" href="{url}" target="_blank" rel="noopener">{url}</a>
  </div>
  <div class="price">{price}</div>
</section>"#,
        html_escape(&source_label(l))
    )
}

/// Human-readable "where this offer is from" for the card meta line.
/// `source` is the internal distributor-bucket slug — for the generic
/// scrape buckets (`vendor` = the manufacturer's own site, `st` =
/// st.com) that slug is meaningless to a reader, so lead with the
/// actual manufacturer (`brand`). API distributors get their proper
/// brand name.
fn source_label(l: &Listing) -> String {
    let brand = l.brand.as_deref().unwrap_or("");
    match l.source.as_str() {
        "vendor" => {
            if brand.is_empty() {
                "manufacturer store".into()
            } else {
                format!("{brand} (manufacturer store)")
            }
        }
        "st" => "STMicroelectronics — st.com".into(),
        "sparkfun" => "SparkFun".into(),
        "mouser" => fmt_dist(brand, "Mouser"),
        "digikey" => fmt_dist(brand, "DigiKey"),
        "farnell" => fmt_dist(brand, "Farnell"),
        other => other.to_string(),
    }
}

fn fmt_dist(brand: &str, dist: &str) -> String {
    if brand.is_empty() {
        dist.to_string()
    } else {
        format!("{brand} @ {dist}")
    }
}

fn shorten(s: &str, limit: usize) -> String {
    let cleaned = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if cleaned.chars().count() <= limit {
        return cleaned;
    }
    let mut out: String = cleaned.chars().take(limit).collect();
    if let Some(idx) = out.rfind(' ') {
        out.truncate(idx);
    }
    out.push('…');
    out
}

/// Format a cm measurement: up to 2 decimals, trailing zeros trimmed
/// (6.30 → "6.3", 4.00 → "4", 0.06 → "0.06", 2.55 → "2.55").
fn fmt_cm(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Inline SVG placeholder (base64 data URL — no network, never fails)
/// used when no source found a real product photo. Shows the part
/// name so the tile is still informative.
fn placeholder_data_url(label: &str) -> String {
    let short: String = label.chars().take(40).collect();
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="380" height="300">
<rect width="380" height="300" fill="#f1f3f5"/>
<rect x="8" y="8" width="364" height="284" fill="none" stroke="#ced4da" stroke-width="2" rx="6"/>
<text x="190" y="140" font-family="Helvetica,Arial,sans-serif" font-size="20" fill="#adb5bd" text-anchor="middle">no photo</text>
<text x="190" y="170" font-family="Helvetica,Arial,sans-serif" font-size="13" fill="#868e96" text-anchor="middle">{}</text>
</svg>"##,
        svg_escape(&short)
    );
    format!(
        "data:image/svg+xml;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(svg.as_bytes())
    )
}

fn svg_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn dirs_downloads() -> PathBuf {
    std::env::var("HOME")
        .map(|h| PathBuf::from(h).join("Downloads"))
        .unwrap_or_else(|_| PathBuf::from("."))
}
