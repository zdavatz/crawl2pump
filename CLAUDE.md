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

## `src/bin/` — scratch bins (gitignored, with one whitelisted exception)

Throwaway one-off binaries (ad-hoc PDF/CSV/report generators,
spelunking tools) live in `src/bin/` and are excluded from git via
`/src/bin/*` in `.gitignore`. Don't register them in `Cargo.toml`
`[[bin]]` either — that would force everyone else to ship them. If a
tool becomes useful enough to keep, promote it by adding the file to
the gitignore whitelist (`!/src/bin/<file>.rs`), registering it in
`Cargo.toml`, and committing.

The single whitelisted promotion so far is `pumpfoil_report.rs` (the
end-to-end "scan all brands, persist to SQLite, render categorized
PDF with new/modified badges" tool — see "Convenience binary" below).

## Convenience binary `pumpfoil_report`

`src/bin/pumpfoil_report.rs` is the single end-to-end tool — it wraps
crawl → curated filter → front-wing spec enrichment → SQLite upsert →
PDF render in one process. Invariants worth knowing before editing it:

- The classifier (`Category::{Sets,Boards,FoilPacks,FrontWings,
  Accessories}`) and front-wing spec extractor (`extract_from_title`,
  `extract_from_text`, `extract_from_html_table`) are **duplicated**
  between this binary and the older scratch bins
  (`listings_pdf.rs`, `enrich_frontwings.rs`). When you change rules,
  change them here — the scratch bins are kept around for one-off
  debugging only and may drift. A future cleanup is to lift these
  into `src/categorize.rs` + `src/specs.rs` library modules.
- **The classifier is intentionally brand-agnostic.** It runs a small
  set of generic keyword tests (`pack`/`set`/`kit`/`complete` for
  packs; `board`/`foilboard` + the Takoon "Pump <material>" pattern
  for boards) against every brand's titles. Some brand-specific names
  fall through the cracks — Ensis labels their foil systems "Maniac
  Stride" / "Maniac Pacer" with no generic keyword, so they land in
  Accessories rather than Foil Packs. **Don't add per-brand override
  rules to fix this** (e.g. `if source == "ensis" && title.contains
  ("maniac")`). The categorization is a navigation hint, not a filter
  — every row is in the PDF regardless of bucket, and a Cmd-F search
  on brand or model finds it instantly. Per-brand overrides drift the
  moment a brand renames a product line and aren't worth the
  maintenance tax. Only widen the generic keyword set if a *new* word
  is genuinely common across multiple brands.
- The "trusted curated sources" set
  (`{axis, onix, indiana, alpinefoil, ketos, armstrong, takoon, code,
  north, mio, starboard, naish, ensis}`) encodes which brand modules
  already filter to pump-foil gear at the source, so we skip the
  title-keyword filter for them. If you add a new pump-curated brand
  source, add it here too — otherwise its components will silently get
  dropped by the keyword filter.
- `--frontwings-only` and `--boards-only` are mutually-exclusive
  filters applied AFTER classification, AFTER spec enrichment, AFTER
  the DB upsert. So the DB always reflects the full curated catalog
  — only the rendered PDF is narrowed. That's intentional: subsequent
  `--from-db` runs can re-render any subset without re-crawling.
- Boards sort = price ascending, with no-price rows pushed to the
  bottom (override of Rust's default `Option::partial_cmp` which puts
  None first). Front-wing sort = `area_cm2` descending. Other
  categories = price ascending.
- The DB write happens **before** the PDF render. The render queries
  the DB (`new_in_scan` / `modified_in_scan`) for freshness badges, so
  the order matters.
- **Two output files per run, one render pass.** `render_html` builds
  one HTML string and `std::fs::write`s it to `<output>.html`; the same
  string is then printed to PDF via headless Chrome. There is no
  separate HTML-vs-PDF templating path. Anything you change in the
  card markup affects both. All product `<a>` tags carry
  `target="_blank" rel="noopener"` so links in the HTML open in new
  browser tabs; Chrome's print path ignores the attribute, so PDF
  behaviour is unchanged.
- **Thumbnail optimisation runs before render.** `optimize_thumbnails`
  rewrites every `Listing.image` URL in two passes:
  1. **Shopify CDN URLs** (host contains `cdn.shopify.com`) get
     `width=600` appended as a query param. Shopify resizes server-side
     before Chrome fetches, so this is free at our end — no decode, no
     re-encode. Covers ~75% of the catalog.
  2. **Everything else** (Indiana, Ketos, AlpineFoil, Code Foils, Mio,
     Ensis) gets fetched through `buffer_unordered(8)`, resized to
     600 px wide via the `image` crate (Lanczos3 + JPEG q=82), and
     embedded as `data:image/jpeg;base64,…`. On any HTTP/decode failure
     we leave the original URL so Chrome falls back to the full-size
     fetch — never blocks the render. Cost: ~3-5 s wall-clock for ~140
     non-Shopify thumbnails on a fresh `--from-db`.

  Net effect on the PDF: 244 MB → ~35 MB. Chrome's printToPDF time also
  drops because the embedded JPEGs are smaller. The 600 px target is
  derived from the card thumb size (44 mm × 34 mm at 300 DPI ≈ 520 ×
  400 px) — going lower (e.g. 400 px) is visible at zoom; going higher
  saves nothing. Don't push it past 600 without checking print quality
  on a representative card first.
- `--from-db` short-circuits the crawl + enrichment + upsert entirely
  and rebuilds `categorized` from `Db::latest_snapshot()` — the rows
  whose `last_seen` matches the most recent scan. `freshness` is empty
  on this path (no fresh scan, no diff to badge) and `summary` is
  zeros. Specs come straight from the stored `area_cm2`/`span_mm`/
  `aspect_ratio`/`chord_mm` columns; no detail-page fetches happen.
  See `load_categorized_from_db` + `stored_to_categorized` +
  `render_from_categorized` in `pumpfoil_report.rs`. Earlier code
  passed an empty `Vec` here, which silently rendered an empty PDF —
  if you touch this path, smoke-test with `--from-db --frontwings-only`
  and confirm the row count matches `SELECT COUNT(*) FROM listings
  WHERE last_seen=(SELECT MAX(last_seen) FROM listings) AND
  category='Front Wings'`.
- **Front-wing enrichment is parallel.** Three passes:
  1. Title parse + description regex (cheap, in-place, sequential).
  2. Detail-page fetch for wings still missing area or span — runs
     through `stream::iter(...).buffer_unordered(8)`, so 8 HTTP fetches
     are in flight at any time. 200+ wings finish in ~3 min instead of
     ~25 min. The merged spec only fills in fields that were `None`
     after pass 1 (we never overwrite a title-extracted value).
  3. Compute AR and chord from area + span if the regex passes didn't
     find them explicitly; drop empty `WingSpecs` so the renderer sees
     `None` for wings with no useful data.
  The fetch client uses a real Safari User-Agent — Naish (and likely
  others) gate the per-variant spec block behind a non-bot UA. A bare
  `(compatible; pumpfoil-report)` UA gets a stripped-down page that
  hides `Aspect_ratio:` / `Front wing span cm:` / `Projected surface
  area cm2:`.

## SQLite persistence (`src/db.rs`)

Schema is created on first open at `sqlite/crawl2pump.db` (overridable
via `--db`). Two tables:

- `listings(url PK, ...listing fields..., area_cm2/span_mm/aspect_ratio/chord_mm,
  category, content_hash, first_seen, last_seen, last_modified_at, scan_count)`
- `price_history(url FK, price, currency, observed_at)` — appended on
  every price change.

`Db::upsert_scan(scan_at, &rows)` is the single write entry point. It
returns `(new_count, updated_count, modified_count, price_changes)`:

- **new** = first time this URL is seen.
- **updated** = touched in this scan (touches `last_seen`); a row can
  be "updated" without being "modified".
- **modified** = `content_hash` differs from previous scan
  (touches `last_modified_at`).

The hash deliberately excludes `description` because Shopify's
`body_html` round-trips with shifting whitespace, which would mint
spurious "modified" diffs every run. Specs (area, span) are *included*
because a corrected wing spec is a real change a buyer cares about.

`category` is **not** part of the content hash — it's our internal
bucket label, not buyer-visible. But the unchanged-content branch of
`upsert_scan` still refreshes it on every scan, so when classify()
rules tighten (e.g. recognising Takoon's `Pump Wood 80` as a board
instead of an accessory) older rows pick up the new bucket without
needing the hash to bump. `last_modified_at` stays put on those
purely-classification refreshes — the buyer-visible content actually
hasn't changed.

Diff queries:

- `Db::new_in_scan(scan_at)` — newly-seen URLs.
- `Db::modified_in_scan(scan_at)` — content changed in this scan.
- `Db::stale_since_scan(scan_at)` — present in previous scans but
  missing now (delisted, sold out, etc).
- `Db::current(scan_at)` — everything that survived this scan.

The DB file is gitignored; the empty `sqlite/` directory is kept via
`.gitkeep` so first-run users have somewhere obvious to point.

### Front-wing rows in the DB

Each front-wing **variant** lives in its own row — URLs include
`?variant=<id>` for Shopify-expanded items so the primary key is
unique per size. That means:

- **Per-size price history.** Armstrong's S1 has four rows
  (1250/1550/1850/2050 cm²) and `price_history` has one stream per
  size — when Armstrong drops the 1550 from $539 to $499 only that
  row's hash flips and a new `price_history` entry appends.
- **Specs persist** even when extraction is partial. `area_cm2`
  comes from the variant title for Onix/Armstrong/Takoon. `span_mm`
  / `chord_mm` come from detail-page fetches and are NULL where the
  shop doesn't publish them. `aspect_ratio` is computed from area +
  span when both are present, even if the spec page only listed two
  of the three.
- **Content hash includes specs.** A scan that fixes a previously
  missing area (because the detail-page fetch succeeded this time)
  flips `content_hash`, marks the row **MOD** in the next PDF, and
  bumps `last_modified_at`. That's expected — the buyer-visible
  data improved.

If you add a new brand source that yields front wings without size
variants (single-SKU per wing, like Code Foils), the existing
`product_to_listing` shim still works — variants of length 1 are a
no-op for the explosion logic.

## Stale scratch bins (gitignored, but committed copies in older tags)

For completeness, the bins still living in `src/bin/` (gitignored, may
drift):

- `listings_pdf.rs` — earlier categorized PDF generator. Superseded by
  `pumpfoil_report` (which does crawl + DB + render in one step).
- `enrich_frontwings.rs` — standalone spec enricher. Superseded by the
  inline enrichment loop in `pumpfoil_report`.
- `ricardo_pdf.rs` — Ricardo-only used-gear PDF (fetches detail pages
  via FlareSolverr).
- `ricardo_via_fs.rs` — Ricardo search via FlareSolverr workaround
  (the in-tree Ricardo source still uses chromiumoxide and so fails
  under Cloudflare).
- `ricardo_probe.rs` — one-off "dump rendered HTML" probe.
- `sets_pdf.rs` — render a "foil bundles only" PDF from a
  `crawl2pump --format json` dump. Title-keyword filter for
  packs/kits/complete sets; same SharedBrowser → printToPdf path that
  `pumpfoil_report` uses. Useful when you want a *new+used* sets-only
  catalog (Tutti/Anibis/Ricardo merged in) without touching the SQLite
  pipeline.
- `surfari_rentals_pdf.rs` — fetches surfari.ch's
  `/collections/mietboards` (Shopify), filters to pumpfoil rentals via
  `looks_like_pump_foil`, prints a small PDF catalog. Surfari is a
  Zürich rental shop — not part of the new/used product crawl, so it
  lives here rather than as a brand source. Daily-rate prices land in
  `Listing.price` as-is; the renderer appends "/ Tag" in the price
  cell. Each rental entry's variants collapse to one Listing
  (rentals don't have size variants worth exploding). If Surfari ever
  adds wingfoil rentals you'd want, widen the filter to also accept
  `looks_like_front_wing` matches — the collection has 27 boards
  total, so the filter shoulder is wide enough to take a few extras
  without flooding.

If you find yourself running any of these regularly, that's the cue to
either fold the logic into `pumpfoil_report` or promote that bin into
the whitelist.

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
   `/product/*` hrefs; or `brands/mio.rs` — fetches `/c/shop/boards/foil`
   and pulls `/p/*` hrefs from a Store29 webshop with no usable sitemap,
   relying on `og:price:amount` / `og:price:currency` meta tags as
   `parse_page_product`'s price-extraction fallback path).
6. Make sure the module's `region()` is accurate — Swiss brands shipping
   from CH should return `Region::Ch`.
7. **Shopify rate-limiting fallback** — Naish 429s the
   `foil-collection` endpoint when 4+ collections fire concurrently.
   `brands/naish.rs` and `brands/starboard.rs` use a small
   `fetch_with_retry` shim: 500 ms gap between collection fetches plus
   one 2 s retry on error. Copy that pattern (rather than touching
   shared `shopify::fetch_paginated`) for any future brand whose
   Shopify backend rejects bursty access — it keeps the rate-limit
   workaround scoped to the brands that actually need it.
8. **Brand-info-only sites with no e-commerce** — Ensis (`ensis.surf`)
   ships product pages with `og:title` / `og:image` / `og:description`
   but no Product JSON-LD and no price. `parse_page_product` returns
   `price=None` for these; the listings still flow through to the DB
   and PDF (renderers display `—` for missing price). `brands/ensis.rs`
   uses a hand-curated URL allowlist instead of `looks_like_pump_foil`
   because Ensis's slugs are model names (Pacer / Stride / Maniac)
   rather than category words. Pattern is fine; just expect price
   columns to be empty across these rows.
9. **Single-product brands with no Product JSON-LD** — Pump Zürich
   (`pump.zuerich/skate/`) is one product hosted on WordPress.com /
   Atomic. No JSON-LD `Product`, no `og:price:*` meta — price lives in
   free-text inside the description ("Price without shipping is EUR
   660.-"). `brands/pumpzuerich.rs` hardcodes the single URL, calls
   `fetch_page_product` for OG metadata, then runs a small
   `EUR|CHF|USD <number>` regex over the description to recover the
   price. The `og:title` is just "Skate" (too generic) so we override
   the listing's title to "Pump Tsüri Skate" for display. Copy this
   pattern for any other one-product micro-brand we want to surface
   in the catalog — keep the regex permissive on currency so a future
   CHF/USD price would still trigger.

## Pump-foil-specific filtering

`html_util::looks_like_pump_foil(text)` is the canonical strict
keyword test — accepts `pumpfoil`/`pump foil`/`pump-foil`/`pumping`/
`dockstart`/`foilpump`/`foil pumping`. Use it instead of
`looks_like_foil_product` (which is loose — matches `wing`/`mast`/
`board`/`kit`/`set` and floods with non-pump items) when narrowing a
brand catalog at the source.

## Classifier word-boundary trap

`pumpfoil_report::classify` checks for pack/set/kit/complete keywords
in the lowercased title. **Always use word-boundary regex (`\bkit\b`
etc.), never `t.contains(" kit")`.** Mio's site tagline is
`Eco Kite und Surfshop`, which appears in every product title; the
substring ` kit` matched the leading space + first three letters of
`Kite` and silently routed every Mio board into the foil-pack "Sets"
bucket. Same shape would hit any future shop with `Kite` in the brand
line. The classifier now uses compiled regexes for `pack`/`set`/`kit`/
`complete` and falls back to plain substring only for words that
can't have inflections (`combo`, `bundle`).

Brand-pattern board-detection: Takoon labels their pump boards as
`Pump Wood 80` / `Pump Carbon` / `Pump Scoot Carbon` — neither title
nor URL has `board`, so we added a regex
`^pump\s+(wood|carbon|scoot|aluminium|alu|foam|epoxy)\b` to the
`has_board` check. The accessory_word check still routes `Pump
Backpack` / `Pump Hose Adapter` / `Pump Tips` to Accessories before
the pump-material rule fires.

Pump-skate detection: `\bskate\b` and the literal `hydroskate` are
both in the `has_board` test. Pump skates are foil-pumping land
trainers (you stand on a deck on wheels, pump for technique). Pump
Zürich's "Pump Tsüri Skate" is the current example, Indiana's
"Hydroskate" line is another. The accessory_word check absorbs the
`Hydroskate Backpack` collision before the skate rule fires, so this
widening is safe. Don't read this as a license to add per-brand
overrides though — the rule still stands: only widen the keyword set
when the new word is genuinely generic (any brand selling that kind
of product would benefit), never as a per-product hack.

## Shopify variant explosion

`shopify::product_to_listings(p, ...)` returns one `Listing` per
*size* variant for products like Armstrong S1 Front Foil
(1250/1550/1850/2050 cm²) or Onix Osprey (550-2250 cm²) — each
variant gets its own URL (`?variant=<id>`), title (with the variant
name appended), price, and DB row, so the SQLite layer can price-
track per size.

The single-Listing helper `product_to_listing` is kept as a
backwards-compat shim — new brand sources should use
`product_to_listings` and `flat_map`/`extend` the result.

The "is this a size variant" check (`looks_like_size_variant`):
- Default-Title and titles >24 chars fail (the latter knocks out
  combos like `Carbon / Black / 220mm`).
- Titles containing `/` fail — multi-axis option combos for packs
  (`1850 / 220 carve / 71` is front-wing×stab×mast, not a size).
  This was the bug that exploded Onix's pack catalog into 600+ rows.
- Otherwise, the first 3-4 digit run in the title must be in
  `[100, 2500]` — i.e. plausibly a wing area in cm² or span in mm.

If you add a new Shopify brand with size variants, run a smoke test
on its `/products.json` and confirm only foil-component products
explode (front wings, sometimes stabilizers and masts) — packs and
boards should stay collapsed.

`html_util::looks_like_front_wing(text)` is the companion test for
front-wing components. It matches `front wing`/`front-wing`/
`frontwing`/`front foil`/`front-foil`/`aile avant`/`ailes avant`
while explicitly excluding `rear wing` / `tail wing` (those are
stabilizers). All sitemap-based brand sources accept items matching
EITHER `looks_like_pump_foil` OR `looks_like_front_wing`, because
front wings are pump-foil-relevant components even when the SKU title
omits `pump` (Indiana's `Foil HP Front Wing 920 H-AR`, Ketos's
`Aile Avant 1450`, etc.). For Shopify brands without a pump-curated
collection, prefer pulling the brand's own `front-wings` / `front-foils`
collection (Onix, North, Armstrong) on top of the pump-pack
collection.

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
sort front wings by price — riders shop by area. `pumpfoil_report`
sorts the FrontWings category **descending** by `specs.area_cm2`
(largest beginner / glide wings first; smallest race / high-aspect
wings last). No-spec wings sink to the bottom of the section.

### Spec-text regex hygiene (`extract_from_text`)

Lessons from chasing wrong values across brands — change carefully:

- **Strip HTML tags before matching.** Naish (and likely others) ship
  spec labels embedded in `<strong>` tags: `<p><strong>Front wing
  span cm:</strong> 83.5</p>`. The extractor runs `<[^>]+>` →  ` `
  then collapses whitespace before applying the per-field regexes,
  so labels and values appear adjacent. Without this step the
  `[^0-9<>\n]{0,N}` gap pattern fails because `</strong>` contains
  `<` and `>`.
- **Iterate matches, range-check each.** Naish's page header carries
  a JSON state blob `"aspect_ratio":true,"img_aspect_ratio":3.698`
  that lexically precedes the human-readable `Aspect_ratio: 4.1`.
  The first regex.captures() grabs the JSON value; the range check
  rejects 1.0 / true but a single `re.captures(text)` doesn't try
  again. Use `for c in re.captures_iter(text) { ... if range_ok {
  break; } }` so out-of-range hits keep the search going.
- **Require `\s*:` after `aspect[_ ]+ratio`.** This is what disqualifies
  the JSON form `"aspect_ratio":` (where `"` separates `ratio` from
  the colon, breaking `\s*:`) and accepts the HTML form
  `Aspect_ratio: 4.1` (where `:` immediately follows). Doing this with
  Rust's regex crate is fine since we don't need lookbehind — just a
  positive `:` anchor.
- **German connectives.** Indiana's body text reads "Spannweite **von**
  1696 mm, einen Chord **von** 173 mm, eine projizierte Fläche
  **von** 2274 cm²". The label-to-number gap pattern is therefore
  `[^0-9\n]{0,15-20}` (allows "von "), not `[\s:=]*` (only colon /
  whitespace). Same fix needed across area / span / chord / AR.
- **Span values may be in cm, not mm.** Naish prints "Front wing span
  cm: 100.0". The `span_cm` regex matches before the mm regex and
  multiplies by 10 to normalize to mm. Range gate `30..=250` (cm)
  catches obvious garbage.
- **Don't widen the range gates without thinking.** Plausibility
  ranges per field: area 200–2700 cm², span 300–2500 mm, AR 3–15,
  chord 50–400 mm. These are domain bounds, not arbitrary; loosening
  them lets Naish's image aspect ratio (3.698) become "AR=3.7" again.

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
