pub mod db;
pub mod listing;
pub mod output;
pub mod sources;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use listing::{Condition, Region};
use sources::Source;

#[derive(Parser, Debug, Default)]
#[command(
    name = "crawl2pump",
    about = "Crawl the web for new and second-hand pumpfoil sets (Switzerland + worldwide)"
)]
pub struct Cli {
    /// Region filter.
    #[arg(long, value_enum, default_value_t = RegionFilter::All)]
    pub region: RegionFilter,

    /// Only show listings in this condition. Brand shops emit "new";
    /// classifieds (Ricardo / Tutti / Anibis / FB Marketplace) emit "used".
    #[arg(long, value_enum, default_value_t = ConditionFilter::All)]
    pub condition: ConditionFilter,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Table)]
    pub format: Format,

    /// Output file path (stdout if omitted).
    #[arg(long, short)]
    pub output: Option<String>,

    /// Comma-separated source names to include (e.g. "axis,gong"). All if omitted.
    #[arg(long)]
    pub sources: Option<String>,

    /// Only show items whose title/description contain this keyword (case-insensitive).
    #[arg(long)]
    pub filter: Option<String>,

    /// Hide sold-out / unavailable items where availability is known.
    #[arg(long)]
    pub in_stock_only: bool,

    /// Free-form search query (used by classifieds sources).
    #[arg(long, default_value = "pumpfoil")]
    pub query: String,

    /// Skip launching headless Chrome. Classifieds sources will be omitted.
    #[arg(long)]
    pub no_browser: bool,

    /// Run Chrome with a visible window (useful for debugging anti-bot issues
    /// or logging into Facebook Marketplace later).
    #[arg(long)]
    pub headful: bool,

    /// FlareSolverr endpoint (used for Tutti / Anibis, which sit behind
    /// Cloudflare Turnstile and can't be reached via plain headless Chrome).
    /// Set to empty string to disable — Tutti/Anibis will then skip.
    #[arg(
        long,
        env = "CRAWL2PUMP_FLARESOLVERR",
        default_value = "http://localhost:8191/v1"
    )]
    pub flaresolverr: String,

    /// If the FlareSolverr endpoint is unreachable, don't try to auto-start
    /// it via Docker or download the standalone binary. Tutti/Anibis will
    /// then skip with an install hint.
    #[arg(long)]
    pub no_auto_flaresolverr: bool,

    /// Facebook Marketplace city scope. Use "worldwide" for no city filter.
    /// Common CH options: zurich, bern, basel, geneva, lausanne, luzern.
    #[arg(long, default_value = "zurich")]
    pub fb_location: String,
}

#[derive(Copy, Clone, Debug, ValueEnum, Default)]
pub enum RegionFilter {
    Ch,
    World,
    #[default]
    All,
}

#[derive(Copy, Clone, Debug, ValueEnum, Default)]
pub enum ConditionFilter {
    New,
    Used,
    #[default]
    All,
}

#[derive(Copy, Clone, Debug, ValueEnum, Default)]
pub enum Format {
    #[default]
    Table,
    Json,
    Csv,
}

pub fn build_sources(
    client: reqwest::Client,
    browser: Option<std::sync::Arc<sources::browser::SharedBrowser>>,
    flaresolverr: Option<std::sync::Arc<sources::flaresolverr::FlareSolverrClient>>,
    fb_location: String,
) -> Vec<Box<dyn Source>> {
    let mut v: Vec<Box<dyn Source>> = vec![
        Box::new(sources::brands::axis::AxisFoils::new(client.clone())),
        Box::new(sources::brands::armstrong::ArmstrongFoils::new(client.clone())),
        Box::new(sources::brands::gong::GongSurfboards::new(client.clone())),
        Box::new(sources::brands::lift::LiftFoils::new(client.clone())),
        Box::new(sources::brands::takuma::TakumaFoils::new(client.clone())),
        Box::new(sources::brands::indiana::IndianaSup::new(client.clone())),
        Box::new(sources::brands::alpinefoil::AlpineFoil::new(client.clone())),
        Box::new(sources::brands::ketos::Ketos::new(client.clone())),
        Box::new(sources::brands::onix::OnixFoils::new(client.clone())),
        Box::new(sources::brands::takoon::Takoon::new(client.clone())),
        Box::new(sources::brands::codefoils::CodeFoils::new(client.clone())),
        Box::new(sources::brands::north::North::new(client.clone())),
        Box::new(sources::brands::mio::Mio::new(client.clone())),
        Box::new(sources::brands::starboard::Starboard::new(client.clone())),
        Box::new(sources::brands::naish::Naish::new(client.clone())),
        Box::new(sources::brands::ensis::Ensis::new(client.clone())),
        Box::new(sources::brands::brack::Brack::new(client.clone())),
        Box::new(sources::brands::galaxus::Galaxus::new(client.clone())),
        Box::new(sources::brands::secondhand::SecondHand::new(client.clone())),
        Box::new(sources::brands::pumpzuerich::PumpZuerich::new(client)),
    ];
    if let Some(b) = browser {
        v.push(Box::new(sources::classifieds::ricardo::Ricardo::new(b.clone())));
        v.push(Box::new(sources::classifieds::facebook::Facebook::new(
            b,
            fb_location,
        )));
    }
    // Tutti / Anibis always go through FlareSolverr (headless Chrome can't
    // beat Turnstile). We always register them; if FlareSolverr isn't
    // configured they'll error with a clear install hint at call time.
    v.push(Box::new(sources::classifieds::tutti::Tutti::new(
        flaresolverr.clone(),
    )));
    v.push(Box::new(sources::classifieds::anibis::Anibis::new(
        flaresolverr,
    )));
    v
}

pub async fn run(cli: Cli) -> Result<()> {
    let format = cli.format;
    let output_path = cli.output.clone();
    let listings = crawl_listings(cli).await?;
    output::write(&listings, format, output_path.as_deref())?;
    Ok(())
}

/// The crawl pipeline without output formatting. Returns the deduped,
/// filtered, sorted listings — exactly what `run` would have written
/// to stdout. New binaries (e.g. `pumpfoil_report`) compose on top of
/// this.
pub async fn crawl_listings(cli: Cli) -> Result<Vec<listing::Listing>> {
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
             AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15",
        )
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let browser = if cli.no_browser {
        None
    } else {
        Some(sources::browser::SharedBrowser::new(
            sources::browser::BrowserOptions {
                headful: cli.headful,
            },
        ))
    };

    // FlareSolverr: skip entirely if the user passed an empty string; else
    // try ping first, then escalate to Docker / standalone-binary auto-start
    // unless --no-auto-flaresolverr was set.
    let flaresolverr = if cli.flaresolverr.trim().is_empty() {
        None
    } else {
        match sources::flaresolverr::FlareSolverrClient::new(cli.flaresolverr.clone()) {
            Ok(fs) => {
                let ready = if cli.no_auto_flaresolverr {
                    fs.ping().await
                } else {
                    sources::flaresolverr::ensure_running(&fs).await
                };
                match ready {
                    Ok(_) => Some(std::sync::Arc::new(fs)),
                    Err(e) => {
                        eprintln!("flaresolverr: {e}\n\t  → Tutti/Anibis will skip");
                        None
                    }
                }
            }
            Err(e) => {
                eprintln!("flaresolverr: build failed ({e}) — Tutti/Anibis will skip");
                None
            }
        }
    };

    let all_sources = build_sources(client, browser, flaresolverr, cli.fb_location.clone());

    let selected: Option<Vec<String>> = cli.sources.as_ref().map(|s| {
        s.split(',')
            .map(|x| x.trim().to_lowercase())
            .filter(|x| !x.is_empty())
            .collect()
    });

    let region_ok = |r: Region| match cli.region {
        RegionFilter::All => true,
        RegionFilter::Ch => matches!(r, Region::Ch),
        RegionFilter::World => matches!(r, Region::World),
    };

    let filtered: Vec<Box<dyn Source>> = all_sources
        .into_iter()
        .filter(|s| {
            let name_ok = selected
                .as_ref()
                .map(|sel| sel.iter().any(|n| n == s.name()))
                .unwrap_or(true);
            name_ok && region_ok(s.region())
        })
        .collect();

    if filtered.is_empty() {
        eprintln!("no sources selected");
        return Ok(Vec::new());
    }

    eprintln!(
        "running {} source(s): {}",
        filtered.len(),
        filtered
            .iter()
            .map(|s| s.name())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let query = cli.query.clone();
    let handles: Vec<_> = filtered
        .into_iter()
        .map(|src| {
            let q = query.clone();
            tokio::spawn(async move {
                let name = src.name();
                let started = std::time::Instant::now();
                match src.search(&q).await {
                    Ok(ls) => {
                        eprintln!(
                            "  [ok ] {name:<10} {:>4} listing(s)  ({:?})",
                            ls.len(),
                            started.elapsed()
                        );
                        ls
                    }
                    Err(e) => {
                        eprintln!("  [err] {name:<10} {e}");
                        Vec::new()
                    }
                }
            })
        })
        .collect();

    let mut all_listings = Vec::new();
    for h in handles {
        if let Ok(ls) = h.await {
            all_listings.extend(ls);
        }
    }

    // Dedup by URL.
    all_listings.sort_by(|a, b| a.url.cmp(&b.url));
    all_listings.dedup_by(|a, b| a.url == b.url);

    // Optional keyword filter. Matches as both a literal substring AND
    // against a compacted (spaces+hyphens stripped) haystack so
    // `--filter pumpfoil` catches titles like "Pump Foil Board".
    if let Some(kw) = &cli.filter {
        let kw_low = kw.to_lowercase();
        let kw_compact: String = kw_low.chars().filter(|c| *c != ' ' && *c != '-').collect();
        all_listings.retain(|l| {
            let hay = format!(
                "{}\n{}",
                l.title.to_lowercase(),
                l.description.as_deref().unwrap_or("").to_lowercase()
            );
            let hay_compact: String = hay.chars().filter(|c| *c != ' ' && *c != '-').collect();
            hay.contains(&kw_low) || hay_compact.contains(&kw_compact)
        });
    }

    if cli.in_stock_only {
        all_listings.retain(|l| l.available.unwrap_or(true));
    }

    match cli.condition {
        ConditionFilter::All => {}
        ConditionFilter::New => all_listings.retain(|l| matches!(l.condition, Condition::New)),
        ConditionFilter::Used => all_listings.retain(|l| matches!(l.condition, Condition::Used)),
    }

    // Sort: brand, then price ascending.
    all_listings.sort_by(|a, b| {
        a.brand.cmp(&b.brand).then_with(|| {
            a.price
                .partial_cmp(&b.price)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    Ok(all_listings)
}
