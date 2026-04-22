# crawl2pump

Find new and second-hand pumpfoil gear across Swiss and worldwide sources.

A Rust CLI that queries pumpfoil brand shops and Swiss classifieds in one
pass, normalises the results, and prints them as a table / JSON / CSV.

## Sources

### Brand shops (new gear)

| Source | Region | Platform |
|---|---|---|
| Axis Foils | World | Shopify |
| Armstrong Foils | World | Shopify |
| Gong (gong-galaxy.com) | World | Shopify |
| Lift Foils | World | Shopify |
| Takuma | World | **URL unverified — stub** |
| Indiana (indiana-sup.ch) | Switzerland | Shopware (sitemap + JSON-LD) |

### Classifieds (second-hand)

| Source | Region | Bring-up |
|---|---|---|
| Ricardo.ch | Switzerland | headless Chrome (chromiumoxide) |
| Tutti.ch | Switzerland | FlareSolverr (Cloudflare Turnstile) |
| Anibis.ch | Switzerland | FlareSolverr (Cloudflare Turnstile) |
| Facebook Marketplace | CH city or worldwide | headless Chrome + persistent FB login |

## Install

Requires a recent Rust toolchain (`rustup`).

```bash
git clone https://github.com/zdavatz/crawl2pump
cd crawl2pump
cargo build --release
```

The release binary lands at `./target/release/crawl2pump`.

### Runtime dependencies

- **For brand shops only:** nothing extra.
- **For Ricardo:** Google Chrome installed (auto-detected on macOS / Linux).
- **For Tutti + Anibis:** a running FlareSolverr instance. The crawler
  auto-starts one on first use:
  - If Docker is available → runs the official image
    (`ghcr.io/flaresolverr/flaresolverr:latest`).
  - Else on Linux x64 / Windows x64 → downloads the standalone binary
    from GitHub releases into `.flaresolverr/`.
  - Else (e.g. macOS without Docker) → clear install hint, Tutti/Anibis skip.

## Usage

```bash
# All sources, default table output
./target/release/crawl2pump

# Only Swiss second-hand listings
./target/release/crawl2pump --region ch --condition used

# Specific brands, export JSON
./target/release/crawl2pump --sources axis,gong --format json -o gear.json

# Match a keyword against title/description
./target/release/crawl2pump --filter "beginner"

# Skip Chrome (brands only, much faster)
./target/release/crawl2pump --no-browser
```

### CLI flags

| Flag | Default | Effect |
|---|---|---|
| `--region <ch\|world\|all>` | `all` | Region filter |
| `--condition <new\|used\|all>` | `all` | Brand shops emit `new`, classifieds emit `used` |
| `--sources <csv>` | *(all)* | E.g. `axis,gong,ricardo` |
| `--query <text>` | `pumpfoil` | Classifieds search term |
| `--filter <text>` | *(none)* | Post-filter on title/description |
| `--format <table\|json\|csv>` | `table` | Output format |
| `--output <path>` | *(stdout)* | Write to file |
| `--in-stock-only` | off | Hide unavailable items where known |
| `--no-browser` | off | Skip headless Chrome (brands-only run) |
| `--headful` | off | Show Chrome window (debug anti-bot) |
| `--flaresolverr <url>` | `http://localhost:8191/v1` | FlareSolverr endpoint |
| `--no-auto-flaresolverr` | off | Don't auto-start FlareSolverr |
| `--fb-location <city>` | `zurich` | Facebook Marketplace city scope (or `worldwide`) |

Environment variables:

- `CRAWL2PUMP_FLARESOLVERR` — same as `--flaresolverr`
- `CRAWL2PUMP_DEBUG_HTML=/path/to/dir` — dump every fetched HTML file for
  selector tuning

## Notes and caveats

- **Cloudflare Turnstile** (Tutti, Anibis) cannot be beaten by plain headless
  Chrome — even in headful mode with manual clicks — because Chrome's
  automation banner is visible to the challenge. That's why we route these
  two through FlareSolverr.
- **IP throttling** on Ricardo can kick in after rapid repeat requests; wait
  10–15 minutes or use a VPN.
- **Takuma's storefront URL is unverified** — the original `takumafoils.com`
  is NXDOMAIN. Update `BASE` in `src/sources/brands/takuma.rs` once known.
- The `.chrome-profile/` directory persists Chrome state (cookies, CF
  clearance, FB login) between runs — don't delete it unless you want to
  start fresh.
- **Facebook Marketplace requires a logged-in session.** First run:
  `crawl2pump --headful --sources facebook` → log into `facebook.com` in
  the window that opens → re-run (headless is fine from then on, until FB
  expires the cookie in a few weeks/months). Using a **throwaway FB
  account** is strongly recommended since scraping violates FB's ToS and
  accounts can be flagged.

## Architecture

```
src/
├── main.rs               # bin entry
├── lib.rs                # CLI + run()
├── listing.rs            # Listing, Condition, Region
├── output.rs             # table / JSON / CSV writers
└── sources/
    ├── mod.rs            # Source trait
    ├── shopify.rs        # generic /products.json fetcher
    ├── html_util.rs      # sitemap + JSON-LD Product parser
    ├── browser.rs        # lazy shared chromiumoxide Chrome
    ├── flaresolverr.rs   # FlareSolverr client + auto-start (Docker / standalone)
    ├── brands/           # one module per brand shop
    │   ├── axis.rs
    │   ├── armstrong.rs
    │   ├── gong.rs
    │   ├── indiana.rs
    │   ├── lift.rs
    │   └── takuma.rs
    └── classifieds/
        ├── mod.rs        # shared helpers (price parser, card walk, CF detection)
        ├── ricardo.rs    # via browser
        ├── tutti.rs      # via FlareSolverr
        ├── anibis.rs     # via FlareSolverr
        └── facebook.rs   # via browser + persistent FB login
```

Each source implements `sources::Source`:

```rust
#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &'static str;
    fn region(&self) -> Region;
    async fn search(&self, query: &str) -> Result<Vec<Listing>>;
}
```

All sources run concurrently via `tokio::spawn`; results are merged,
deduplicated by URL, filtered, sorted, and emitted.

## License

GPL-3.0 — see [LICENSE](LICENSE).
