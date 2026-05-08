//! Meta poster — publishes catalog rows from `sqlite/crawl2pump.db` to
//! the Pump Tsüri Facebook Page. Scratch bin, gitignored — promote to
//! a real bin via `Cargo.toml` + `.gitignore` whitelist if it earns its
//! keep.
//!
//! Reads credentials from `.meta.env` in the project root (or env vars
//! if already exported). Posts photo + caption + URL per row, paced with
//! a configurable gap. Logs each post_id so they can be deleted later
//! with the same tool (`--delete <log_path>` not yet wired — manual
//! `curl -X DELETE` for now).
//!
//! Usage:
//!   cargo run --release --bin meta_post -- --source onix
//!   cargo run --release --bin meta_post -- --source indiana --gap-secs 20
//!   cargo run --release --bin meta_post -- --source onix --overview --dry-run
//!
//! No second-hand listings — `condition='New'` filter is applied so we
//! only post fresh-from-the-brand catalog rows.

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use reqwest::Client;
use rusqlite::Connection;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;

const GRAPH_BASE: &str = "https://graph.facebook.com/v21.0";
const DEFAULT_DB: &str = "sqlite/crawl2pump.db";
const DEFAULT_ENV: &str = ".meta.env";

#[derive(Parser, Debug)]
#[command(version, about = "Post brand catalog rows from SQLite to a Facebook Page")]
struct Args {
    /// Brand source to publish (e.g. `onix`, `indiana`, `axis`).
    #[arg(long)]
    source: String,
    /// Also post a one-paragraph overview message before the products.
    #[arg(long)]
    overview: bool,
    /// Free-form overview text (used with --overview). Falls back to a
    /// generic summary built from the row counts if absent.
    #[arg(long)]
    overview_message: Option<String>,
    /// Link to attach to the overview post (e.g. brand shop URL).
    #[arg(long)]
    overview_link: Option<String>,
    /// Seconds between successive posts. FB's anti-spam tolerates 10–30 s
    /// for a brand-new page; lower at your own risk.
    #[arg(long, default_value_t = 15)]
    gap_secs: u64,
    /// Cap the number of product posts (useful for testing).
    #[arg(long)]
    limit: Option<usize>,
    /// Print what would be posted but make no API calls.
    #[arg(long)]
    dry_run: bool,
    /// SQLite path.
    #[arg(long, default_value = DEFAULT_DB)]
    db: PathBuf,
    /// `.meta.env` path (key=value lines for META_PAGE_ID + META_PAGE_TOKEN).
    #[arg(long, default_value = DEFAULT_ENV)]
    env_file: PathBuf,
}

#[derive(Debug)]
struct Row {
    title: String,
    url: String,
    image: Option<String>,
    price: Option<f64>,
    currency: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PhotoResp {
    #[serde(default)]
    post_id: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FeedResp {
    id: String,
}

fn load_env_file(path: &std::path::Path) -> Result<()> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("note: {} not found, relying on already-exported env vars", path.display());
            return Ok(());
        }
        Err(e) => return Err(e).with_context(|| format!("read {}", path.display())),
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        // Don't overwrite vars the caller already set.
        if std::env::var_os(k).is_none() {
            std::env::set_var(k, v.trim());
        }
    }
    Ok(())
}

fn fetch_rows(db: &std::path::Path, source: &str) -> Result<Vec<Row>> {
    let conn = Connection::open(db).with_context(|| format!("open {}", db.display()))?;
    let mut stmt = conn.prepare(
        "SELECT title, url, image, price, currency, description
         FROM listings
         WHERE source = ?1
           AND (condition IS NULL OR LOWER(condition) = 'new')
           AND last_seen = (SELECT MAX(last_seen) FROM listings WHERE source = ?1)
         ORDER BY category, price",
    )?;
    let raw: Vec<Row> = stmt
        .query_map([source], |r| {
            Ok(Row {
                title: r.get(0)?,
                url: r.get(1)?,
                image: r.get(2)?,
                price: r.get(3)?,
                currency: r.get(4)?,
                description: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    // Defensive dedup by (title, price) — earlier crawls produced
    // multiple variant rows with identical titles when a product had
    // attribute axes the source code didn't reflect in the label
    // (e.g. Ketos Pack Kahuna with size+deck axes only surfaced the
    // size in the title). The brand source has since been fixed, but
    // older rows may still be in the DB; this keeps the FB feed clean.
    let mut seen = std::collections::HashSet::new();
    let rows: Vec<Row> = raw
        .into_iter()
        .filter(|r| {
            let price_key = r.price.map(|p| (p * 100.0) as i64);
            seen.insert((r.title.clone(), price_key))
        })
        .collect();
    Ok(rows)
}

/// Strip HTML tags, decode the few HTML entities Shopify/WordPress use in
/// `body_html`, collapse whitespace, and truncate. Captions over ~700 chars
/// get the "See more…" cut on FB's mobile UI which is fine but unsightly.
fn clean_description(raw: &str, max_chars: usize) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let decoded = out
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">");
    let collapsed: String = decoded
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let truncated: String = collapsed.chars().take(max_chars).collect();
        // backtrack to last sentence boundary if we cut mid-word
        if let Some(last_dot) = truncated.rfind(['.', '!', '?', '…']) {
            if last_dot > max_chars / 2 {
                return format!("{}…", &truncated[..=last_dot]);
            }
        }
        format!("{truncated}…")
    }
}

/// Shopify CDN URLs accept `width=` + `format=jpg` query params for
/// server-side resize + transcode. Saves Chrome a fetch and dodges any
/// .webp incompatibility on FB's image ingest.
fn normalise_image_url(url: &str) -> String {
    if url.contains("cdn.shopify.com") {
        let sep = if url.contains('?') { '&' } else { '?' };
        format!("{url}{sep}width=1200&format=jpg")
    } else {
        url.to_string()
    }
}

fn build_caption(r: &Row) -> String {
    let price_line = match (r.price, r.currency.as_deref()) {
        (Some(p), Some(cur)) if !cur.is_empty() => format!("{p:.0} {cur}\n\n"),
        (Some(p), _) => format!("{p:.0}\n\n"),
        _ => String::new(),
    };
    let desc_block = r
        .description
        .as_deref()
        .and_then(extract_caption_block)
        .map(|d| format!("{d}\n\n"))
        .unwrap_or_default();
    format!("{}\n\n{}{}🔗 {}", r.title, price_line, desc_block, r.url)
}

/// Pick what to surface from `description` in the FB caption. Priority:
///   1. The variant spec line (Ketos appends "Surface area: NNN cm² …"
///      to variant descriptions). Always preferred — short, factual,
///      and exactly what a buyer scans for.
///   2. Otherwise, the first ~250 chars of cleaned prose.
fn extract_caption_block(desc: &str) -> Option<String> {
    let cleaned = clean_description(desc, usize::MAX);
    if cleaned.is_empty() {
        return None;
    }
    // Variant spec line (Ketos pattern). Match on "Surface area:" — that's
    // what `brands/ketos.rs` writes; the master-product table uses the
    // different "Surface cm2" header which doesn't read well in plain text.
    for marker in ["Surface area:", "Surface area :"] {
        if let Some(pos) = cleaned.find(marker) {
            let tail: String = cleaned[pos..].chars().take(300).collect();
            return Some(tail);
        }
    }
    Some(clean_description(desc, 250))
}

async fn post_photo(
    client: &Client,
    page_id: &str,
    page_token: &str,
    image_url: &str,
    caption: &str,
) -> Result<String> {
    let resp = client
        .post(format!("{GRAPH_BASE}/{page_id}/photos"))
        .form(&[
            ("url", image_url),
            ("caption", caption),
            ("access_token", page_token),
        ])
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        bail!("FB API {}: {}", status, text);
    }
    let r: PhotoResp = serde_json::from_str(&text).with_context(|| text.clone())?;
    r.post_id.or(r.id).ok_or_else(|| anyhow!("no id in response: {text}"))
}

async fn post_feed(
    client: &Client,
    page_id: &str,
    page_token: &str,
    message: &str,
    link: Option<&str>,
) -> Result<String> {
    let mut form = vec![
        ("message", message.to_string()),
        ("access_token", page_token.to_string()),
    ];
    if let Some(l) = link {
        form.push(("link", l.to_string()));
    }
    let resp = client
        .post(format!("{GRAPH_BASE}/{page_id}/feed"))
        .form(&form)
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        bail!("FB API {}: {}", status, text);
    }
    let r: FeedResp = serde_json::from_str(&text).with_context(|| text.clone())?;
    Ok(r.id)
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    load_env_file(&args.env_file)?;

    let page_id = std::env::var("META_PAGE_ID").context("META_PAGE_ID missing")?;
    let page_token = std::env::var("META_PAGE_TOKEN").context("META_PAGE_TOKEN missing")?;

    let mut rows = fetch_rows(&args.db, &args.source)?;
    if let Some(lim) = args.limit {
        rows.truncate(lim);
    }
    if rows.is_empty() {
        bail!("no rows for source={}", args.source);
    }
    eprintln!("source={}: {} row(s) to post", args.source, rows.len());

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    if args.overview {
        let msg = args.overview_message.clone().unwrap_or_else(|| {
            format!("Pump Tsüri — {} catalog ({} Produkte).", args.source, rows.len())
        });
        if args.dry_run {
            eprintln!("[dry-run] overview: {msg}");
        } else {
            match post_feed(
                &client,
                &page_id,
                &page_token,
                &msg,
                args.overview_link.as_deref(),
            )
            .await
            {
                Ok(id) => println!("overview OK  id={id}"),
                Err(e) => eprintln!("overview FAIL {e}"),
            }
            tokio::time::sleep(Duration::from_secs(args.gap_secs)).await;
        }
    }

    let mut ok = 0usize;
    let mut fail = 0usize;
    for (i, r) in rows.iter().enumerate() {
        let Some(image) = r.image.as_deref() else {
            eprintln!("  [{:2}/{}] SKIP (no image)  {}", i + 1, rows.len(), trunc(&r.title, 55));
            fail += 1;
            continue;
        };
        let image_url = normalise_image_url(image);
        let caption = build_caption(r);
        if args.dry_run {
            eprintln!("[dry-run] [{:2}/{}] {}", i + 1, rows.len(), trunc(&r.title, 55));
            continue;
        }
        match post_photo(&client, &page_id, &page_token, &image_url, &caption).await {
            Ok(id) => {
                println!("  [{:2}/{}] OK   post_id={}  {}", i + 1, rows.len(), id, trunc(&r.title, 55));
                ok += 1;
            }
            Err(e) => {
                eprintln!("  [{:2}/{}] FAIL  {}\n           {}", i + 1, rows.len(), trunc(&r.title, 55), e);
                fail += 1;
            }
        }
        if i + 1 < rows.len() {
            tokio::time::sleep(Duration::from_secs(args.gap_secs)).await;
        }
    }
    eprintln!("\nDone: {ok} posted, {fail} failed (out of {}).", rows.len());
    Ok(())
}

fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
