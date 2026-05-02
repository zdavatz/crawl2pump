# crawl2pump

Find new and second-hand pumpfoil gear across Swiss and worldwide sources.

A Rust CLI that queries pumpfoil brand shops and Swiss classifieds in one
pass, normalises the results, and prints them as a table / JSON / CSV.

## Sources

### Brand shops (new gear)

| Source | Region | Platform | Discovery |
|---|---|---|---|
| Axis Foils | World | Shopify | `/collections/all-pump/products.json` (curated 128-item pump collection) |
| Armstrong Foils | World | Shopify | `step-one-collection` + `front-foils` collection + global title-filter for `pump` |
| Gong (gong-galaxy.com) | World | Shopify | `/products.json` (filter applied downstream) |
| Lift Foils | World | Shopify | `/products.json` |
| North (northactionsports.com) | World | Shopify | `front-wings` + `foilboards` collections + global pump title-filter |
| Takuma | World | **URL unverified — stub** | — |
| Indiana (indiana-sup.ch) | Switzerland | Magento (sitemap + JSON-LD) | sitemap + `<image:title>` for pumpfoil/front-wing/stabilizer |
| AlpineFoil | France | Custom (sitemap + JSON-LD) | `/kitefoil-windfoil-shop/.../*.html` for pumpfoil + front-wing keywords |
| Ketos | France | WordPress / WooCommerce | English shop only, pumpfoil + front-wing keywords |
| Onix | France | Shopify | `combo-packs` + `foil-full-pack` + `front-wings` collections |
| Takoon | France | Shopify | `pack-foil-pump` + `foil-pump` collections + global `pump` title-filter |
| Code Foils | USA | WordPress (no per-product sitemap) | scrape `/products/` index page; no retail prices (dealer-only) |
| Mio (mioboards.com) | Switzerland | Custom (Store29 platform) | scrape `/c/shop/boards/foil` for `/p/*` product URLs; OG meta tags for price |
| Starboard (star-board.com) | World | Shopify | `foilboards` collection + strict pump-keyword filter (only the dedicated Pump Foilboard survives) |
| Naish (naish.com) | World | Shopify | `foil-collection` + `foil-completes` + `foil-boards` + `front-wings-a-la-cart`, filtered by `product_type` allowlist (front wings / masts / stabs / fuselages / semi-completes / DW + SUP foil boards) — drops wing-foil-only and kite-foil boards |
| Ensis (ensis.surf) | Switzerland | WordPress (no e-commerce, info only) | sitemap-based; URL allowlist for Pacer / Stride / Maniac line + `pumpfoil` slugs. **No prices** (Ensis pages have no Product schema). |

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
  - Else (e.g. macOS without Docker) → Tutti/Anibis skip with an install
    hint. On macOS you can still run FlareSolverr manually from source,
    see ["Running FlareSolverr on macOS"](#running-flaresolverr-on-macos)
    below.

### Running FlareSolverr on macOS

Upstream doesn't ship a macOS binary and Docker Desktop is a heavy
install. The easiest path is to run FlareSolverr from source in a Python
venv — it's a Python app and supports macOS officially:

```bash
git clone --depth 1 https://github.com/FlareSolverr/FlareSolverr.git .flaresolverr-src
cd .flaresolverr-src
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt
HEADLESS=false .venv/bin/python src/flaresolverr.py
```

`HEADLESS=false` is required on macOS — FlareSolverr's default headless
mode spawns Xvfb (X11), which macOS doesn't have. With this setting,
Chrome briefly pops up a small window while solving Cloudflare
challenges, which is fine for a local tool. `.flaresolverr-src/` is in
`.gitignore`.

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

The `--format json` output is stable across runs (each item carries `source`,
`title`, `url`, `price`, `currency`, `condition`, `image`, `region`,
`fetched_at`, …) and is meant to be piped into downstream tools — e.g. a
small local script that filters for "sets / packages / kits" and renders a
printable catalog. Such scratch tooling belongs in `src/bin/` (gitignored)
so it doesn't become part of the shipped crate. Examples that have lived
there: `sets_pdf.rs` (foil-bundle PDF from a JSON dump),
`surfari_rentals_pdf.rs` (pumpfoil rentals from surfari.ch's Shopify
`mietboards` collection — Zürich rental shop, not part of the brand crawl).
See `CLAUDE.md` for the full catalogue.

### `pumpfoil_report` — one-shot brand scan + PDF + SQLite history

A second binary wraps the full pipeline so you don't have to chain
`crawl2pump | jq | enrich | listings_pdf` by hand:

```bash
./target/release/pumpfoil_report                       # all categories → ~/Downloads/pumpfoil.pdf
./target/release/pumpfoil_report --frontwings-only     # front wings only PDF (sorted by area, descending)
./target/release/pumpfoil_report --boards-only         # boards only PDF (sorted by price, ascending; no-price rows last)
./target/release/pumpfoil_report --output /tmp/x.pdf   # custom output
./target/release/pumpfoil_report --from-db             # re-render from DB without re-crawling
./target/release/pumpfoil_report --no-spec-fetch       # skip detail-page spec enrichment
```

Each run does five things:

1. Crawls all brand shops (Axis, Armstrong, Gong, Lift, North, Mio,
   Indiana, AlpineFoil, Ketos, Onix, Takoon, Code Foils, Starboard,
   Naish, Ensis) — sources fire concurrently via `tokio::spawn` so the
   18-source crawl finishes in ~30 s.
2. Filters down to pump-foil-relevant gear (curated brand modules are
   trusted; Gong/Lift get a title-keyword filter).
3. Categorizes into **Sets · Boards · Foil Packs · Front Wings ·
   Components/Accessories**. Front Wings get spec extraction
   (area, span, AR, chord) from title patterns + JSON-LD
   `body_html` + a detail-page fallback fetch. The fallback runs in
   parallel via `buffer_unordered(8)` — 200+ wings enrich in ~3 min
   instead of ~25 min serial.
4. Persists to `sqlite/crawl2pump.db` (created on first run) with
   per-URL `first_seen` / `last_seen` / `last_modified_at` columns and
   a SHA-256 content hash for fast change detection. Price changes are
   appended to `price_history`.
5. Renders both an HTML file (next to the PDF, same basename) and a
   PDF with **NEW** / **MOD** badges on listings that appeared or
   changed since the previous scan, plus a header strip summarising
   counts. All product links in the HTML open in a new browser tab
   (`target="_blank" rel="noopener"`) so you can fan rows out into
   tabs without losing your place in the catalog. Chrome ignores
   the attribute when printing the PDF, so the PDF behaviour is
   unchanged.

   Thumbnails are optimised before rendering: Shopify CDN URLs get a
   `width=600` query param appended (server-side resize, ~75% of the
   catalog), and non-Shopify thumbnails (Indiana, Ketos, AlpineFoil,
   Code Foils, Mio, Ensis) are fetched + resized + re-encoded as 600 px
   JPEGs and inlined as `data:` URLs. Resize fetches run in parallel
   via `buffer_unordered(8)`, same pattern as front-wing enrichment.
   Net effect: PDF dropped from ~244 MB to ~35 MB with no visible
   quality loss at normal zoom (thumbnails render at 44 mm × 34 mm,
   so 600 px is already 1.15× oversampled at 300 DPI).

`--from-db` short-circuits steps 1–4 and rebuilds the categorized
list directly from the most recent scan in `sqlite/crawl2pump.db`,
including stored area / span / AR / chord. Useful for fast PDF
iteration after a real scan.

The categorization is a *navigation hint*, not a filter — every
crawled row is in the PDF regardless of which bucket it landed in.
Some brand-specific names don't match the generic `pack`/`set`/`kit`/
`complete` keywords (Ensis "Maniac Stride", for example, lands in
Accessories rather than Foil Packs). When you can't find a known
product where you expect it, **just Cmd-F the PDF for the brand or
model name** — search is the universal escape hatch.

The Front Wings section sorts by `area_cm2` **descending** — biggest
wings first (beginner / glide), smallest last (high-performance /
race). No-spec wings sink to the bottom of the section. The Boards
section sorts by **price ascending**, with no-price rows pushed to
the bottom (Rust's default `Option::partial_cmp` puts None first;
we want None last so real prices ascend cleanly without a wedge of
"—" at the top). Within each other category, items sort by price
ascending.

Front-wing coverage is broad: the strict pump-foil keyword filter is
augmented with a `looks_like_front_wing` test (`html_util.rs`) that
matches `front wing` / `front-wing` / `frontwing` / `front foil` /
`aile avant` while excluding rear/tail/stab spellings. Sitemap-based
brands (Indiana, AlpineFoil, Ketos) accept any URL or `<image:title>`
matching either filter; Shopify brands (Onix, Armstrong, North) pull
their dedicated `front-wings` / `front-foils` collection on top of
their pump-pack collections.

For Shopify brands where one product carries multiple **size variants**
(Armstrong S1 1250/1550/1850/2050, Onix Osprey 550/750/950/.../2250,
Takoon Foil Pump 1500/1700/1900) the `product_to_listings` helper
emits one Listing per size variant — each gets its own URL
(`?variant=<id>`) and price, so the SQLite layer dedupes correctly
and price-tracks per size. Variant titles that look like multi-axis
option combos (slash-separated like `1850 / 220 carve / 71`) are
left collapsed to avoid exploding pack permutations into hundreds of
rows. Latest scan: **213 front wings** across nine brands,
ranging 480 cm² → 2'700 cm².

Spec extraction handles the various ways brands present wing data:

- **Title patterns** — Axis `PNG 1300` / `BSC 970`, North `MA 950v2`,
  Naish `Jet HA 1840`, Armstrong `S1 2450` / `HA 580` (area in cm²)
  + Axis `820mm Carbon Front Wing` (span in mm).
- **English / French / German prose** — Indiana ships specs in their
  product description as "Spannweite von 1696 mm, einen Chord von
  173 mm, eine projizierte Fläche von 2274 cm², eine Aspect Ratio
  von 12.0". The extractor tolerates `von` / `of` / `:` connectives
  between label and number across all four metrics.
- **Naish per-variant spec block** — Naish detail pages carry an HTML
  `<p><strong>Aspect_ratio:</strong> 5.6</p>` / `<p><strong>Front
  wing span cm:</strong> 83.5</p>` / `<p><strong>Projected surface
  area cm2:</strong> 1250</p>` block per size variant. The extractor
  strips HTML tags before regex (so labels embedded in `<strong>`
  match), accepts the underscore-form `Aspect_ratio:` (Naish's
  spelling) alongside the regular `Aspect Ratio`, and converts the
  `span cm` value to mm. The AR regex requires a colon directly
  after `ratio` to avoid latching onto JSON state blobs like
  `"img_aspect_ratio":3.698`.
- **Computed fallbacks** — when area + span are present but AR /
  chord aren't explicit, AR = (span_cm)² / area_cm² and chord =
  (area × 100) / span_mm.

The 213 front wing rows split: 197 with area, 79 with span, 76 with
chord, 87 with AR. Coverage gaps are mostly North / Onix / Takoon
which only publish area on detail pages, and Armstrong (JS-rendered
shell with no spec text in HTML).

The SQLite path can be overridden with `--db <path>`. The database
file itself is gitignored; only the schema (in `src/db.rs`) and the
empty `sqlite/` directory are checked in.

#### Front-wing tracking in the DB

Every front-wing variant ends up as its own row in the `listings`
table, indexed by URL. Each variant URL is unique (`?variant=<id>` on
Shopify shops, full path elsewhere), so Armstrong's S1 1250 / 1550 /
1850 / 2050 are four separate rows with four separate price-history
streams. Spec columns (`area_cm2`, `span_mm`, `aspect_ratio`,
`chord_mm`) are populated where extraction succeeded; `content_hash`
covers all of them (so a corrected wing area on a later scan triggers
the **MOD** badge).

Useful queries:

```bash
# All front wings sorted by area (largest first)
sqlite3 sqlite/crawl2pump.db \
  "SELECT source, title, area_cm2, span_mm, aspect_ratio, price, currency
   FROM listings WHERE category='Front Wings'
   ORDER BY area_cm2 DESC NULLS LAST"

# Front wings new in the last scan
sqlite3 sqlite/crawl2pump.db \
  "SELECT source, title, price FROM listings
   WHERE category='Front Wings' AND first_seen = last_seen
   ORDER BY first_seen DESC"

# Price drops on any size variant
sqlite3 sqlite/crawl2pump.db \
  "SELECT l.title, h.price, h.currency, h.observed_at
   FROM price_history h JOIN listings l ON l.url = h.url
   WHERE l.category='Front Wings'
   ORDER BY h.observed_at DESC LIMIT 20"
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
- **Tutti / Anibis freetext search is not possible** — the search URL carries
  a base64url-msgpack filter token and silently drops `?query=` args. We
  sidestep this by iterating a handful of **category tokens** (sport/outdoor,
  other sports, boats, accessories, all-categories; the slug is plaintext-
  base64 inside the blob) and applying the freetext filter client-side. Net
  effect: ~130 recent listings per site instead of the ~30 you'd get from a
  single all-categories page.
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
    │   ├── alpinefoil.rs
    │   ├── armstrong.rs
    │   ├── axis.rs
    │   ├── codefoils.rs
    │   ├── gong.rs
    │   ├── indiana.rs
    │   ├── ketos.rs
    │   ├── lift.rs
    │   ├── onix.rs
    │   ├── takoon.rs
    │   └── takuma.rs
    └── classifieds/
        ├── mod.rs                  # shared helpers (price parser, card walk, CF detection)
        ├── tutti_anibis_cards.rs   # shared Next.js card extractor + category tokens
        ├── ricardo.rs              # via browser
        ├── tutti.rs                # via FlareSolverr
        ├── anibis.rs               # via FlareSolverr
        └── facebook.rs             # via browser + persistent FB login
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
