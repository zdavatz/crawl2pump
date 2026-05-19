//! Electronics-distributor sources for the sensor / module BOM
//! (`crate::sensors`). Unlike the pumpfoil brand/classified sources
//! these are **part-list driven**, not query driven: each distributor
//! is handed the whole BOM and returns one [`Listing`] per part it can
//! price (or merely confirm a product page for).
//!
//! Two kinds:
//! - **Scrape** (`st`, `sparkfun`) — no credentials, parse the public
//!   product page via [`html_util::fetch_page_product`].
//! - **API** (`mouser`, `digikey`, `farnell`) — need keys read from
//!   `.sensors.env` (gitignored) or already-exported env vars. When the
//!   keys are absent the distributor **skips with a one-line hint**
//!   (same UX as the FlareSolverr auto-start path) rather than erroring.
//!
//! `.sensors.env` schema (key=value, one per line, `#` comments):
//! ```text
//! MOUSER_API_KEY=...
//! DIGIKEY_CLIENT_ID=...
//! DIGIKEY_CLIENT_SECRET=...
//! FARNELL_API_KEY=...
//! ```

use crate::listing::Listing;
use crate::sensors::{Part, Role};
use reqwest::Client;

pub mod digikey;
pub mod farnell;
pub mod mouser;
pub mod sparkfun;
pub mod st;
pub mod vendor;

/// One distributor offer for a BOM part, carrying the part identity so
/// the report can group offers per part without a DB round-trip.
#[derive(Debug, Clone)]
pub struct SensorOffer {
    pub part_key: &'static str,
    pub part_name: &'static str,
    pub role: Role,
    pub listing: Listing,
}

/// Load `.sensors.env` into the process env (already-set vars win, so
/// CI can override without touching the file). Missing file is fine —
/// the API distributors just skip.
pub fn load_sensors_env(path: &std::path::Path) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "note: {} not found — API distributors (Mouser/DigiKey/Farnell) will skip",
                path.display()
            );
            return;
        }
        Err(e) => {
            eprintln!("warn: read {}: {e}", path.display());
            return;
        }
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if std::env::var_os(k).is_none() {
                std::env::set_var(k, v.trim());
            }
        }
    }
}

/// Run every distributor concurrently against the full BOM and return
/// the merged offers. Each distributor logs its own `[ok]/[skip]/[err]`
/// line so a missing API key is obvious but non-fatal.
pub async fn crawl_all(client: &Client, parts: &[Part]) -> Vec<SensorOffer> {
    let (st, ve, sf, mo, dk, fa) = tokio::join!(
        run("st", st::fetch(client, parts)),
        run("vendor", vendor::fetch(client, parts)),
        run("sparkfun", sparkfun::fetch(client, parts)),
        run("mouser", mouser::fetch(client, parts)),
        run("digikey", digikey::fetch(client, parts)),
        run("farnell", farnell::fetch(client, parts)),
    );
    let mut out = Vec::new();
    for v in [st, ve, sf, mo, dk, fa] {
        out.extend(v);
    }
    out
}

async fn run<F>(name: &str, fut: F) -> Vec<SensorOffer>
where
    F: std::future::Future<Output = anyhow::Result<Vec<SensorOffer>>>,
{
    let started = std::time::Instant::now();
    match fut.await {
        Ok(v) => {
            eprintln!(
                "  [ok ] {name:<9} {:>3} offer(s)  ({:?})",
                v.len(),
                started.elapsed()
            );
            v
        }
        Err(e) => {
            // A clean "skip" is signalled by the distributor returning
            // an error whose message starts with "skip:". Everything
            // else is a real error worth a louder line.
            let msg = e.to_string();
            if let Some(rest) = msg.strip_prefix("skip:") {
                eprintln!("  [skip] {name:<8}{rest}");
            } else {
                eprintln!("  [err] {name:<9} {msg}");
            }
            Vec::new()
        }
    }
}

/// Helper used by every distributor to build a `SensorOffer` with the
/// shared `Listing` shape (condition=new, region=world).
#[allow(clippy::too_many_arguments)]
pub(crate) fn offer(
    part: &Part,
    distributor: &str,
    title: String,
    url: String,
    price: Option<f64>,
    currency: Option<String>,
    available: Option<bool>,
    image: Option<String>,
    extra_desc: Option<String>,
) -> SensorOffer {
    use crate::listing::{Condition, Region};
    let mut desc = format!(
        "{} · {} · firmware: {}",
        part.note,
        part.connector.label(),
        if part.oss_firmware {
            "open-source"
        } else {
            "closed/blob"
        }
    );
    if let Some(x) = extra_desc {
        desc.push_str(" · ");
        desc.push_str(&x);
    }
    SensorOffer {
        part_key: part.key,
        part_name: part.name,
        role: part.role,
        listing: Listing {
            source: distributor.to_string(),
            brand: Some(part.manufacturer.to_string()),
            title,
            url,
            price,
            currency,
            condition: Condition::New,
            available,
            location: Some(distributor.to_string()),
            description: Some(desc),
            image,
            region: Region::World,
            fetched_at: chrono::Utc::now(),
        },
    }
}
