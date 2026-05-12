//! SQLite persistence for pumpfoil listings.
//!
//! Schema is created on first open. Each `upsert_scan` records a fresh
//! scan: rows whose `url` is new get `first_seen = scan_at`; existing
//! rows have their `last_seen`, fields, and price refreshed. A
//! `price_history` row is appended whenever the price changes for an
//! existing url.
//!
//! "What's new" = rows where `first_seen == scan_at` after the upsert
//! (returned by `new_in_scan`). "What's gone stale" = rows whose
//! `last_seen < scan_at` after the upsert (`stale_since_scan`).
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::listing::{Condition, Listing, Region};

pub const DEFAULT_PATH: &str = "sqlite/crawl2pump.db";

pub struct Db {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct StoredListing {
    pub url: String,
    pub source: String,
    pub brand: Option<String>,
    pub title: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub condition: Option<String>,
    pub available: Option<bool>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub region: Option<String>,
    pub area_cm2: Option<f64>,
    pub span_mm: Option<f64>,
    pub aspect_ratio: Option<f64>,
    pub chord_mm: Option<f64>,
    pub category: Option<String>,
    pub content_hash: Option<String>,
    pub first_seen: DateTime<Utc>,
    pub last_seen: DateTime<Utc>,
    pub last_modified_at: DateTime<Utc>,
    pub scan_count: i64,
}

impl Db {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).context("create sqlite dir")?;
            }
        }
        let conn = Connection::open(path).context("open sqlite db")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let me = Self { conn };
        me.migrate()?;
        Ok(me)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
CREATE TABLE IF NOT EXISTS listings (
    url           TEXT PRIMARY KEY,
    source        TEXT NOT NULL,
    brand         TEXT,
    title         TEXT NOT NULL,
    price         REAL,
    currency      TEXT,
    condition     TEXT,
    available     INTEGER,
    location      TEXT,
    description   TEXT,
    image         TEXT,
    region        TEXT,
    area_cm2      REAL,
    span_mm       REAL,
    aspect_ratio  REAL,
    chord_mm      REAL,
    category      TEXT,
    content_hash  TEXT,
    first_seen        TEXT NOT NULL,
    last_seen         TEXT NOT NULL,
    last_modified_at  TEXT NOT NULL,
    scan_count    INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX IF NOT EXISTS idx_listings_source ON listings(source);
CREATE INDEX IF NOT EXISTS idx_listings_last_seen ON listings(last_seen);

CREATE TABLE IF NOT EXISTS price_history (
    url          TEXT NOT NULL,
    price        REAL,
    currency     TEXT,
    observed_at  TEXT NOT NULL,
    FOREIGN KEY (url) REFERENCES listings(url) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_price_history_url ON price_history(url);

-- Resized + base64-inlined thumbnails. Keyed by the fetch URL (which
-- includes Shopify width= and any other resize params), so changing
-- the thumbnail size invalidates the cache implicitly. Brand URLs that
-- carry version tokens (`?v=...`) get a fresh cache row when the
-- thumbnail itself changes.
CREATE TABLE IF NOT EXISTS image_cache (
    url          TEXT PRIMARY KEY,
    data_url     TEXT NOT NULL,
    cached_at    TEXT NOT NULL
);
"#,
        )?;
        Ok(())
    }

    /// Look up a cached base64 data-URL for a thumbnail. Returns None
    /// on miss or if the row is somehow malformed.
    pub fn get_cached_image(&self, url: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT data_url FROM image_cache WHERE url = ?1")?;
        let mut rows = stmt.query(params![url])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    /// Store a resized thumbnail's data URL keyed by its fetch URL.
    pub fn put_cached_image(&self, url: &str, data_url: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO image_cache (url, data_url, cached_at) VALUES (?1, ?2, ?3)",
            params![url, data_url, chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Upsert all listings for a single scan. Returns counts of new
    /// rows, modified rows (content_hash changed vs previous scan), and
    /// price changes.
    pub fn upsert_scan(
        &mut self,
        scan_at: DateTime<Utc>,
        rows: &[ListingRow<'_>],
    ) -> Result<UpsertSummary> {
        let tx = self.conn.transaction()?;
        let mut new_count = 0;
        let mut updated_count = 0;
        let mut modified_count = 0;
        let mut price_changes = 0;
        let scan_iso = scan_at.to_rfc3339();

        for row in rows {
            let hash = row.content_hash();

            let existing: Option<(Option<f64>, Option<String>, String, Option<String>)> = tx
                .query_row(
                    "SELECT price, currency, first_seen, content_hash FROM listings WHERE url = ?1",
                    params![row.url],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .ok();

            match existing {
                None => {
                    tx.execute(
                        r#"INSERT INTO listings (
                            url, source, brand, title, price, currency, condition,
                            available, location, description, image, region,
                            area_cm2, span_mm, aspect_ratio, chord_mm, category,
                            content_hash, first_seen, last_seen, last_modified_at,
                            scan_count
                        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,1)"#,
                        params![
                            row.url,
                            row.source,
                            row.brand,
                            row.title,
                            row.price,
                            row.currency,
                            row.condition,
                            row.available.map(|b| b as i64),
                            row.location,
                            row.description,
                            row.image,
                            row.region,
                            row.area_cm2,
                            row.span_mm,
                            row.aspect_ratio,
                            row.chord_mm,
                            row.category,
                            hash,
                            scan_iso,
                            scan_iso,
                            scan_iso,
                        ],
                    )?;
                    if row.price.is_some() {
                        tx.execute(
                            "INSERT INTO price_history (url, price, currency, observed_at) VALUES (?1, ?2, ?3, ?4)",
                            params![row.url, row.price, row.currency, scan_iso],
                        )?;
                    }
                    new_count += 1;
                }
                Some((prev_price, _prev_currency, _first_seen, prev_hash)) => {
                    let content_changed = prev_hash.as_deref() != Some(hash.as_str());
                    let modified_iso = if content_changed { &scan_iso } else { "" };
                    if content_changed {
                        tx.execute(
                            r#"UPDATE listings SET
                                source=?2, brand=?3, title=?4, price=?5, currency=?6,
                                condition=?7, available=?8, location=?9, description=?10,
                                image=?11, region=?12, area_cm2=?13, span_mm=?14,
                                aspect_ratio=?15, chord_mm=?16, category=?17,
                                content_hash=?18, last_seen=?19, last_modified_at=?19,
                                scan_count = scan_count + 1
                              WHERE url=?1"#,
                            params![
                                row.url,
                                row.source,
                                row.brand,
                                row.title,
                                row.price,
                                row.currency,
                                row.condition,
                                row.available.map(|b| b as i64),
                                row.location,
                                row.description,
                                row.image,
                                row.region,
                                row.area_cm2,
                                row.span_mm,
                                row.aspect_ratio,
                                row.chord_mm,
                                row.category,
                                hash,
                                scan_iso,
                            ],
                        )?;
                        modified_count += 1;
                    } else {
                        // Content unchanged from the buyer's POV (price,
                        // title, image, specs all match) — touch
                        // last_seen and bump scan_count, but DON'T
                        // touch last_modified_at. Category, however,
                        // refreshes — when classify() rules tighten
                        // (e.g. Takoon's "Pump Wood 80" board moved
                        // out of Accessories), older rows shouldn't
                        // keep the stale bucket label.
                        tx.execute(
                            "UPDATE listings SET last_seen=?1, scan_count = scan_count + 1, category=?2 WHERE url=?3",
                            params![scan_iso, row.category, row.url],
                        )?;
                    }
                    let _ = modified_iso;
                    if row.price.is_some() && row.price != prev_price {
                        tx.execute(
                            "INSERT INTO price_history (url, price, currency, observed_at) VALUES (?1, ?2, ?3, ?4)",
                            params![row.url, row.price, row.currency, scan_iso],
                        )?;
                        price_changes += 1;
                    }
                    updated_count += 1;
                }
            }
        }

        tx.commit()?;
        Ok(UpsertSummary {
            new_count,
            updated_count,
            modified_count,
            price_changes,
        })
    }

    /// Listings whose content hash changed in this scan (price/title/
    /// description/image/specs differ from the previously stored row).
    pub fn modified_in_scan(&self, scan_at: DateTime<Utc>) -> Result<Vec<StoredListing>> {
        self.query_listings(
            "SELECT * FROM listings WHERE last_modified_at = ?1 AND first_seen != ?1 ORDER BY source, title",
            params![scan_at.to_rfc3339()],
        )
    }

    /// Listings that first appeared in the given scan.
    pub fn new_in_scan(&self, scan_at: DateTime<Utc>) -> Result<Vec<StoredListing>> {
        self.query_listings(
            "SELECT * FROM listings WHERE first_seen = ?1 ORDER BY source, title",
            params![scan_at.to_rfc3339()],
        )
    }

    /// Listings first seen at or after the cutoff timestamp. Use this
    /// to populate the freshness map on `--from-db` renders, where no
    /// single `scan_at` covers multiple single-source upserts.
    pub fn new_since(&self, cutoff: DateTime<Utc>) -> Result<Vec<StoredListing>> {
        self.query_listings(
            "SELECT * FROM listings WHERE first_seen >= ?1 ORDER BY source, title",
            params![cutoff.to_rfc3339()],
        )
    }

    /// Listings whose content changed at or after the cutoff (but
    /// weren't first-seen in the same window — those are already
    /// counted as `new`).
    pub fn modified_since(&self, cutoff: DateTime<Utc>) -> Result<Vec<StoredListing>> {
        self.query_listings(
            "SELECT * FROM listings \
             WHERE last_modified_at >= ?1 AND first_seen < ?1 \
             ORDER BY source, title",
            params![cutoff.to_rfc3339()],
        )
    }

    /// Listings present in this scan (by `last_seen == scan_at`).
    pub fn current(&self, scan_at: DateTime<Utc>) -> Result<Vec<StoredListing>> {
        self.query_listings(
            "SELECT * FROM listings WHERE last_seen = ?1 ORDER BY source, title",
            params![scan_at.to_rfc3339()],
        )
    }

    /// Listings from the most recent scan in the DB. Used by
    /// `pumpfoil_report --from-db` to re-render without re-crawling. The
    /// "most recent scan" is the maximum `last_seen` value across all
    /// listings; we return everything that matches it.
    /// Current state across all sources — for each `source`, the rows
    /// from its most recent scan. Uses per-source `MAX(last_seen)` so a
    /// single-source upsert (e.g. `pumpfoil_report --sources brack`)
    /// doesn't make every other brand look stale.
    pub fn latest_snapshot(&self) -> Result<Vec<StoredListing>> {
        self.query_listings(
            "SELECT l.* FROM listings l \
             WHERE l.last_seen = ( \
                SELECT MAX(last_seen) FROM listings WHERE source = l.source \
             ) \
             ORDER BY l.source, l.title",
            params![],
        )
    }

    /// Listings missing from this scan but seen in the past.
    pub fn stale_since_scan(&self, scan_at: DateTime<Utc>) -> Result<Vec<StoredListing>> {
        self.query_listings(
            "SELECT * FROM listings WHERE last_seen < ?1 ORDER BY source, title",
            params![scan_at.to_rfc3339()],
        )
    }

    fn query_listings(
        &self,
        sql: &str,
        params: impl rusqlite::Params,
    ) -> Result<Vec<StoredListing>> {
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params, |r| {
            Ok(StoredListing {
                url: r.get("url")?,
                source: r.get("source")?,
                brand: r.get("brand")?,
                title: r.get("title")?,
                price: r.get("price")?,
                currency: r.get("currency")?,
                condition: r.get("condition")?,
                available: r.get::<_, Option<i64>>("available")?.map(|n| n != 0),
                location: r.get("location")?,
                description: r.get("description")?,
                image: r.get("image")?,
                region: r.get("region")?,
                area_cm2: r.get("area_cm2")?,
                span_mm: r.get("span_mm")?,
                aspect_ratio: r.get("aspect_ratio")?,
                chord_mm: r.get("chord_mm")?,
                category: r.get("category")?,
                content_hash: r.get("content_hash")?,
                first_seen: parse_iso(&r.get::<_, String>("first_seen")?),
                last_seen: parse_iso(&r.get::<_, String>("last_seen")?),
                last_modified_at: parse_iso(&r.get::<_, String>("last_modified_at")?),
                scan_count: r.get("scan_count")?,
            })
        })?;
        rows.map(|r| r.map_err(Into::into)).collect()
    }
}

fn parse_iso(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[derive(Debug, Clone)]
pub struct UpsertSummary {
    pub new_count: usize,
    /// Total listings touched (new + already-existing). Ratio of
    /// `modified_count / updated_count` is "what fraction of the catalog
    /// changed between scans".
    pub updated_count: usize,
    pub modified_count: usize,
    pub price_changes: usize,
}

/// Borrowed view of a listing for upsert. Keeps the writer code close to
/// what `Listing` already produces while adding the spec/category fields
/// the DB layer cares about.
#[derive(Debug, Clone, Copy)]
pub struct ListingRow<'a> {
    pub url: &'a str,
    pub source: &'a str,
    pub brand: Option<&'a str>,
    pub title: &'a str,
    pub price: Option<f64>,
    pub currency: Option<&'a str>,
    pub condition: Option<&'a str>,
    pub available: Option<bool>,
    pub location: Option<&'a str>,
    pub description: Option<&'a str>,
    pub image: Option<&'a str>,
    pub region: Option<&'a str>,
    pub area_cm2: Option<f64>,
    pub span_mm: Option<f64>,
    pub aspect_ratio: Option<f64>,
    pub chord_mm: Option<f64>,
    pub category: Option<&'a str>,
}

impl<'a> ListingRow<'a> {
    pub fn from_listing(l: &'a Listing, category: Option<&'a str>) -> Self {
        ListingRow {
            url: &l.url,
            source: &l.source,
            brand: l.brand.as_deref(),
            title: &l.title,
            price: l.price,
            currency: l.currency.as_deref(),
            condition: Some(condition_label(l.condition)),
            available: l.available,
            location: l.location.as_deref(),
            description: l.description.as_deref(),
            image: l.image.as_deref(),
            region: Some(region_label(l.region)),
            area_cm2: None,
            span_mm: None,
            aspect_ratio: None,
            chord_mm: None,
            category,
        }
    }

    pub fn with_specs(
        mut self,
        area_cm2: Option<f64>,
        span_mm: Option<f64>,
        aspect_ratio: Option<f64>,
        chord_mm: Option<f64>,
    ) -> Self {
        self.area_cm2 = area_cm2;
        self.span_mm = span_mm;
        self.aspect_ratio = aspect_ratio;
        self.chord_mm = chord_mm;
        self
    }

    /// SHA-256 over the buyer-visible fields. Two scans of the same URL
    /// hash identically iff title/price/currency/image/condition/
    /// availability/specs are identical. Description is intentionally
    /// excluded — Shopify body_html sometimes round-trips with shifting
    /// whitespace, which would manufacture spurious "modified" hits.
    pub fn content_hash(&self) -> String {
        let mut h = Sha256::new();
        let mut field = |label: &str, value: &str| {
            h.update(label.as_bytes());
            h.update(b"=");
            h.update(value.as_bytes());
            h.update(b"\n");
        };
        field("title", self.title);
        field(
            "price",
            &self.price.map(|p| format!("{p:.2}")).unwrap_or_default(),
        );
        field("currency", self.currency.unwrap_or(""));
        field("image", self.image.unwrap_or(""));
        field("condition", self.condition.unwrap_or(""));
        field(
            "available",
            match self.available {
                Some(true) => "1",
                Some(false) => "0",
                None => "",
            },
        );
        field(
            "area",
            &self.area_cm2.map(|v| format!("{v:.1}")).unwrap_or_default(),
        );
        field(
            "span",
            &self.span_mm.map(|v| format!("{v:.1}")).unwrap_or_default(),
        );
        format!("{:x}", h.finalize())
    }
}

fn condition_label(c: Condition) -> &'static str {
    match c {
        Condition::New => "new",
        Condition::Used => "used",
        Condition::Unknown => "unknown",
    }
}

fn region_label(r: Region) -> &'static str {
    match r {
        Region::Ch => "ch",
        Region::World => "world",
    }
}
