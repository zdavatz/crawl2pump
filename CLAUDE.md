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

## `src/bin/` — scratch bins (gitignored)

Throwaway one-off binaries (ad-hoc PDF/CSV/report generators,
spelunking tools) can live in `src/bin/` and are excluded from git.
Don't register them in `Cargo.toml` `[[bin]]` either — that would force
everyone else to ship them. If a tool becomes useful enough to keep,
promote it by moving the file out of `src/bin/`, checking it in, and
adding the `[[bin]]` entry.

## Adding a new brand shop

1. Check if the shop is Shopify: `curl -I https://DOMAIN/products.json`.
   If 200, it is.
2. **If Shopify** — `curl https://DOMAIN/collections.json` and look for
   a pump-foil-named collection (`all-pump`, `combo-packs`, `foil-pump`,
   `pack-foil-pump`, `step-one-collection`, `pumping-packs`, etc.).
   Strongly prefer fetching the curated collection via
   `shopify::fetch_collection_products(client, BASE, "<handle>")` over
   the global `/products.json`. Brands curate pump-foil items into
   collections; the global list mixes wing/wake/SUP gear that uses no
   "pump" in the title and would silently slip past any title-keyword
   filter. See `brands/axis.rs` (single curated collection) and
   `brands/onix.rs` / `brands/takoon.rs` (multiple collections).
3. **If Shopify but no pump collection** — fall back to
   `fetch_all_products` and apply a title-substring filter at the
   source (see `brands/armstrong.rs` and `brands/takoon.rs` for the
   `pump` keyword pattern). Don't push that filter downstream — keep
   sources strict so the multi-source merge in `lib.rs::run` stays
   pump-foil-only without per-caller knowledge.
4. **If not Shopify** — try sitemap-based scraping via
   `html_util::fetch_sitemap_entries` (returns `<loc>` + `<image:title>`
   pairs) + `fetch_page_product` (see `brands/indiana.rs` for a Magento
   example, `brands/alpinefoil.rs` for a custom-XML example,
   `brands/ketos.rs` for WordPress/WooCommerce). Filter via
   `looks_like_pump_foil` against both URL and image titles —
   Magento-style SKU-only URLs (e.g. `3615sq-3615sq.html`) carry the
   real product name in `<image:title>` only, so URL-keyword filters
   would miss real sets like Indiana's Condor XL Complete.
5. **If no sitemap** — last resort, scrape an index page for product
   links (see `brands/codefoils.rs` — fetches `/products/` and pulls
   `/product/*` hrefs).
6. Make sure the module's `region()` is accurate — Swiss brands shipping
   from CH should return `Region::Ch`.

## Pump-foil-specific filtering

`html_util::looks_like_pump_foil(text)` is the canonical strict
keyword test — accepts `pumpfoil`/`pump foil`/`pump-foil`/`pumping`/
`dockstart`/`foilpump`/`foil pumping`. Use it instead of
`looks_like_foil_product` (which is loose — matches `wing`/`mast`/
`board`/`kit`/`set` and floods with non-pump items) when narrowing a
brand catalog at the source.

## JSON-LD parsing gotchas (seen in the wild)

The shared parser at `html_util::parse_page_product` handles three
real-world quirks; don't undo any of them:

- **Raw control characters in JSON-LD strings** — Alpinefoil ships
  `body_html` descriptions with literal `\r\n` inside JSON string
  values, which strict `serde_json::from_str` rejects. We sanitize
  control bytes to spaces before parsing.
- **`AggregateOffer.lowPrice` instead of `Offer.price`** —
  Alpinefoil and Ketos use AggregateOffer for variant-priced packs.
  Our parser falls back to `lowPrice` when `price` is absent.
- **Double-encoded HTML in `name`/`description`** — Indiana ships
  `Indiana 3&#039;7 Pump Foil "Le Doigt"`, Alpinefoil ships
  `&lt;p&gt;...&amp;ccedil;u...&lt;/p&gt;`. We pass titles and
  descriptions through `html_util::clean_html_text`, which re-parses
  as HTML twice (handles both single and double-encoding) and strips
  tags. If a future shop needs another decode pass, do it in that
  helper rather than at the call site.

## Front-wing spec extraction

`src/bin/enrich_frontwings.rs` is a scratch bin that reads a
crawl2pump JSON dump, finds front-wing listings (using the same
classifier rule as `listings_pdf.rs::classify`), and adds a `specs:
{ area_cm2, span_mm, aspect_ratio, chord_mm }` field via three passes:

1. **Title parse** — model name encodes the headline number for most
   brands: Axis `PNG 1300` / `BSC 970` / `HPS 700` / `SP 660` /
   `HA 900` / `ART 999` (area in cm²), Axis `820mm Carbon Front Wing`
   (span in mm), Ketos `PUMPING 1570` / `Aile Avant 1450` / `Pump EVO
   133` (area in cm²).
2. **Description regex** — Shopify `body_html` is already in the
   listing as `description`; regex for `area`, `wingspan`, `aspect
   ratio` near a 3-4-digit number.
3. **Detail-page fetch** — last resort for items still missing both
   area and span. Walks `<table>` th/td pairs and looks for explicit
   `Surface area: NNNN cm²` labels.

Aspect ratio is computed from area + span when not explicit
(`AR = (span_cm)² / area_cm²`); chord is computed similarly. Don't
sort front wings by price — riders shop by area. `listings_pdf`
sorts the FrontWings category ascending by `specs.area_cm2`.

## Known caveats (read before debugging)

- **Takuma URL is unverified.** `takumafoils.com` is NXDOMAIN; the
  module intentionally errors at runtime. Fix by setting `BASE` in
  `src/sources/brands/takuma.rs` once the real storefront is known.
- **Cloudflare Turnstile on Tutti/Anibis** defeats headless Chrome even
  in `--headful` mode — the `--enable-automation` flag chromiumoxide
  sets is visible to the challenge. That's why those two sources
  route through FlareSolverr instead. Do not try to "fix" this by
  adding more stealth patches to `classifieds/mod.rs` — it won't work.
- **Facebook Marketplace requires login cookies** in
  `.chrome-profile/`. First-time setup: `--headful --sources facebook`,
  user logs in manually. FB cookies live alongside CF ones in the same
  profile dir, they don't collide. FB selectors rotate — we key on the
  `/marketplace/item/{id}/` href pattern (stable) and walk up ~7 levels
  for the card container. Do not hardcode CSS class names; they'll
  break within weeks.
- **Tutti/Anibis ignore `?query=`** — their URL path carries an opaque
  base64url-msgpack filter token; query-string args are dropped
  server-side. The **category** slug is plaintext-base64 inside the
  blob though (e.g. `Ak8Cuc3BvcnRzT3V0ZG9vcnOUwMDAwA` → "sportsOutdoors"),
  so we iterate a hand-picked list of foil-relevant category tokens
  in `classifieds/tutti_anibis_cards.rs::CATEGORY_TOKENS` and filter
  the free-text query client-side via `matches_query`. Net effect:
  ~130 recent listings per site instead of the old ~30 all-recent.
  Freetext tokens would still need reverse-engineering of the msgpack
  encoder — not done.
- **Tutti/Anibis card images aren't in the DOM** — the rendered
  `<img src>` is a `data:image/gif…` placeholder that only swaps for
  the real CDN URL after client-side hydration. Tutti hides the real
  URL inside a `<noscript>` fallback (which `html5ever`/`scraper`
  treats as raw text when scripting is enabled, so DOM queries miss it);
  Anibis doesn't even have the noscript fallback. Solution:
  `tutti_anibis_cards::extract_image_map` regexes the Next.js
  dehydrated-state JSON blob for `listingID → thumbnail.normalRendition.src`
  pairs and looks each card up by its
  `data-private-srp-listing-item-id` attribute. Hits ~99% of Tutti
  cards and ~97% of Anibis cards; don't "simplify" it back to a
  `card.select("img")` query.
- **Ricardo's 403 is Cloudflare, not IP throttling.** Plain curl /
  chromiumoxide hit a CF challenge that returns 403; FlareSolverr
  solves it cleanly. The historical "wait 10-15 min and retry"
  advice was wrong — the block clears only when the same client
  passes the challenge. The in-tree Ricardo source still uses
  chromiumoxide and so will fail under CF; the workaround
  is `src/bin/ricardo_via_fs.rs` (scratch) which routes search +
  detail-page fetches through FlareSolverr. Promoting Ricardo to
  FlareSolverr-by-default is a clear architectural improvement, not
  yet done.
- **macOS can't auto-start FlareSolverr** — upstream only ships Linux
  x64 / Windows x64 PyInstaller binaries, and Docker isn't assumed to
  be installed. But FlareSolverr itself is pure Python and officially
  supports macOS, so running it from source works fine. The README
  documents the venv recipe; the key trick is `HEADLESS=false` (macOS
  has no Xvfb, which the default headless path tries to spawn). Clone
  into `.flaresolverr-src/` — that path is in `.gitignore`.
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
- Don't commit the `.chrome-profile/` directory — it contains the user's
  FB/CF session cookies. Already gitignored; keep it that way.
