# CLAUDE.md

Project-specific guidance for Claude Code sessions in this repo. The
top-level `~/software/CLAUDE.md` also applies — this file overrides where
the two disagree.

## What this is

A Rust CLI that crawls pumpfoil gear listings — new gear from brand
Shopify / Shopware shops, second-hand gear from Swiss classifieds
(Ricardo, Tutti, Anibis). See `README.md` for user-facing docs.

## Build / run

```bash
cargo build --release
./target/release/crawl2pump --help

# Brands only (fast, no browser needed)
./target/release/crawl2pump --no-browser

# Full run (launches Chrome for Ricardo + FlareSolverr for Tutti/Anibis)
./target/release/crawl2pump --region ch --condition used

# Dump rendered HTML for selector tuning
CRAWL2PUMP_DEBUG_HTML=/tmp/debug ./target/release/crawl2pump --sources ricardo
```

Tests: `cargo test --release` (currently one: Swiss-price regex).

## Architecture cheatsheet

- Every source is a `Source` impl living under `src/sources/`. The trait
  is `name()` + `region()` + `async search(query) -> Vec<Listing>`.
- `sources::shopify` is a generic `/products.json` client used by
  `brands/{axis,armstrong,gong,lift,takuma}.rs` — those modules are
  mostly constants (base URL, currency, region).
- `sources::html_util` covers non-Shopify shops via sitemap + JSON-LD
  (`brands/indiana.rs`).
- `sources::browser` is a lazy-launched shared `chromiumoxide` Chrome
  instance. Classifieds sources accept an `Arc<SharedBrowser>`.
- `sources::flaresolverr` is the FlareSolverr client **and** the
  auto-start logic (Docker first, standalone binary second).

Sources run concurrently via `tokio::spawn` inside `lib.rs::run`.

## Adding a new brand shop

1. Check if the shop is Shopify: `curl -I https://DOMAIN/products.json`.
2. **If Shopify** — create `src/sources/brands/<brand>.rs` by copying
   `axis.rs`. Update `BASE`, `BRAND`, `CURRENCY`. Register the module in
   `brands/mod.rs` and construct it in `lib.rs::build_sources`.
3. **If not Shopify** — try sitemap-based scraping via
   `html_util::fetch_sitemap_urls` + `fetch_page_product` (see
   `brands/indiana.rs` for a working example).
4. Make sure the module's `region()` is accurate — Swiss brands shipping
   from CH should return `Region::Ch`.

## Known caveats (read before debugging)

- **Takuma URL is unverified.** `takumafoils.com` is NXDOMAIN; the
  module intentionally errors at runtime. Fix by setting `BASE` in
  `src/sources/brands/takuma.rs` once the real storefront is known.
- **Cloudflare Turnstile on Tutti/Anibis** defeats headless Chrome even
  in `--headful` mode — the `--enable-automation` flag chromiumoxide
  sets is visible to the challenge. That's why those two sources
  route through FlareSolverr instead. Do not try to "fix" this by
  adding more stealth patches to `classifieds/mod.rs` — it won't work.
- **Ricardo** works via chromiumoxide but IP-throttles after ~5 rapid
  requests. If you see `<title>Forbidden</title>` in the debug dump,
  back off and retry after 10–15 min.
- **macOS can't run the standalone FlareSolverr binary** — upstream
  only ships Linux x64 / Windows x64 builds. macOS users need Docker
  for Tutti/Anibis.
- **`.chrome-profile/`** persists Chrome state between runs (CF
  clearance cookies etc). It's in `.gitignore`. Don't nuke it lightly.

## Don'ts

- Don't add more "stealth patches" to `classifieds/mod.rs::STEALTH_JS`
  hoping to beat Turnstile — it's a dead end, use FlareSolverr.
- Don't remove `.chrome-profile/` cleanup of stale `SingletonLock` in
  `browser.rs::launch`; it prevents the "second instance" error after
  a Ctrl-C.
- Don't run all three classifieds concurrently against the same IP —
  triggers rate limiting. Prefer running them one at a time when
  testing.
