//! One-off RFQ (Request-for-Quote) PDF generator for the magnetic-switch
//! test PCB ("Versuchsleiterplatte Magnetschalter"), addressed to
//! Variosystems AG (Standort Zizers) for fabrication + assembly.
//!
//! Same pipeline as `sensor_report`: build one HTML string, base64-inline
//! the schematic + layout images, write `<output>.html`, then print to PDF
//! via headless Chrome. Scratch bin (gitignored); run with:
//!
//! ```bash
//! cargo run --release --bin magnetswitch_rfq            # ~/Downloads/...
//! cargo run --release --bin magnetswitch_rfq -- -o /tmp/rfq.pdf
//! ```
//!
//! The two source images are embedded at compile time from
//! `assets/magnetswitch_rfq/`, so the binary is self-contained.

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use clap::Parser;
use std::path::PathBuf;
use std::process::Command;

const CHROME_MAC: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

/// Resolve a usable Chrome/Chromium binary. `$CHROME` wins; otherwise probe
/// the macOS app path then the common Linux binaries, first that exists.
fn chrome_binary() -> String {
    if let Ok(env_path) = std::env::var("CHROME") {
        return env_path;
    }
    let candidates = [
        CHROME_MAC,
        "/usr/bin/google-chrome-stable",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    candidates
        .iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(|p| p.to_string())
        .unwrap_or_else(|| CHROME_MAC.into())
}

// Embedded at compile time so the bin needs no runtime asset files.
const SCHEMATIC_JPG: &[u8] = include_bytes!("../../assets/magnetswitch_rfq/schematic.jpg");
const LAYOUT_JPG: &[u8] = include_bytes!("../../assets/magnetswitch_rfq/pcb_layout.jpg");

#[derive(Parser, Debug)]
#[command(version, about = "Render the magnetic-switch PCB RFQ PDF for Variosystems")]
struct Args {
    /// PDF output path. Default: ~/Downloads/Anfrage-Magnetschalter-LP-Variosystems.pdf
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn data_url(bytes: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    format!("data:image/jpeg;base64,{b64}")
}

fn render_html() -> String {
    let sch = data_url(SCHEMATIC_JPG);
    let pcb = data_url(LAYOUT_JPG);
    format!(
        r##"<!DOCTYPE html>
<html lang="de">
<head>
<meta charset="utf-8">
<style>
  @page {{ size: A4; margin: 16mm 15mm 14mm 15mm; }}
  * {{ box-sizing: border-box; }}
  body {{
    font-family: "Helvetica Neue", Arial, sans-serif;
    color: #1a1a1a; font-size: 10.5pt; line-height: 1.45; margin: 0;
  }}
  h1 {{ font-size: 18pt; margin: 0 0 2px 0; letter-spacing: -0.2px; }}
  h2 {{
    font-size: 12pt; margin: 20px 0 7px 0; padding-bottom: 4px;
    border-bottom: 2px solid #c20a2e; color: #c20a2e;
    break-after: avoid; page-break-after: avoid;
  }}
  .sub {{ color: #555; font-size: 10pt; margin: 0 0 14px 0; }}
  .meta {{
    display: flex; justify-content: space-between; gap: 18px;
    background: #f5f5f7; border: 1px solid #e0e0e4; border-radius: 6px;
    padding: 11px 14px; font-size: 9.5pt; margin: 14px 0 4px 0;
    break-inside: avoid;
  }}
  .meta .blk {{ flex: 1; }}
  .meta .lbl {{ color: #888; text-transform: uppercase; font-size: 7.5pt;
    letter-spacing: 0.6px; display:block; margin-bottom: 1px; }}
  table {{ border-collapse: collapse; width: 100%; font-size: 9.5pt; }}
  th, td {{ border: 1px solid #d8d8dc; padding: 5px 8px; text-align: left;
    vertical-align: top; }}
  th {{ background: #2a2a2e; color: #fff; font-weight: 600; font-size: 8.5pt;
    text-transform: uppercase; letter-spacing: 0.4px; }}
  tr {{ break-inside: avoid; }}
  tr:nth-child(even) td {{ background: #fafafb; }}
  td.ref {{ font-weight: 700; white-space: nowrap; }}
  code {{ font-family: "SF Mono", Menlo, monospace; font-size: 9pt;
    background:#eef; padding: 0 3px; border-radius: 3px; }}
  ul {{ margin: 4px 0 4px 0; padding-left: 18px; break-inside: auto; }}
  li {{ margin: 3px 0; break-inside: avoid; }}
  .spec {{ display: grid; grid-template-columns: max-content 1fr;
    gap: 4px 16px; font-size: 10pt; break-inside: avoid; }}
  .spec .k {{ color: #666; }}
  .spec .v {{ font-weight: 600; }}
  .fig {{ margin: 8px 0 0 0; text-align: center; break-inside: avoid; }}
  .fig img {{ max-width: 100%; border: 1px solid #ccc; border-radius: 4px; }}
  .cap {{ font-size: 8.5pt; color: #777; margin-top: 4px; }}
  .note {{ background: #fff8e6; border-left: 3px solid #e0a800;
    padding: 8px 12px; border-radius: 0 4px 4px 0; font-size: 9.5pt;
    margin: 8px 0; break-inside: avoid; }}
  .sec {{ break-inside: avoid; }}
  .pagebreak {{ break-before: page; page-break-before: always; }}
  .foot {{ margin-top: 18px; font-size: 8pt; color: #999;
    border-top: 1px solid #e0e0e4; padding-top: 6px; }}
</style>
</head>
<body>

<h1>Anfrage: Leiterplatten-Fertigung &amp; Bestückung</h1>
<p class="sub">Versuchsleiterplatte „Magnetschalter" &nbsp;·&nbsp; Prototyp / Kleinserie &nbsp;·&nbsp; doppelseitig, ca. 10&nbsp;×&nbsp;20&nbsp;mm</p>

<div class="meta">
  <div class="blk">
    <span class="lbl">An / Fertiger</span>
    Variosystems AG<br>Standort Zizers<br>Rheinstrasse 24, 7205 Zizers (GR)<br>Schweiz
  </div>
  <div class="blk">
    <span class="lbl">Von / Auftraggeber</span>
    Zeno Davatz<br>zdavatz@gmail.com<br><br>Schaltungs-Design: Peter Schmidlin
  </div>
  <div class="blk">
    <span class="lbl">Dokument</span>
    RFQ Magnetschalter-LP<br>Datum: 15.06.2026<br>Stückzahl: siehe Abschnitt 6<br>Bestückung: SMD, beidseitig n. Bedarf
  </div>
</div>

<div class="sec">
<h2>1 · Zweck &amp; Kontext</h2>
<p>
Diese Leiterplatte ist die <b>Versuchs­leiterplatte für den Magnetschalter</b> eines
Pumpfoil-Sensor-Aufbaus. Der Magnetsensor (Hall-Schalter U1) schaltet über eine
Transistorstufe (Q2) einen P-Kanal-MOSFET (Q1), der die Versorgungsspannung zur
nachgelagerten Elektronik (ST-Sensorbox) durchschaltet — also ein berührungsloser,
magnetbetätigter Ein-/Aus-Schalter.
</p>
<p>
Es handelt sich um einen <b>Versuchsaufbau zur Validierung</b> (u.&nbsp;a. der Frage, ob
WiFi sinnvoll bzw. notwendig ist). Im nächsten Schritt sollen die Elektronik von
Sensor-Box, GPS und Magnetschalter <b>zusammengelegt und deutlich verkleinert</b> werden;
vermutlich kann dann auch die Batteriekapazität (und damit die Baugrösse) reduziert
werden. Die Elektronik der ST-Sensorbox ist eine gute Ausgangslage, für unseren Bedarf
kann jedoch einiges weggelassen werden. Für diese Anfrage zählt zunächst die hier
gezeigte Versuchs­leiterplatte.
</p>
</div>

<div class="sec">
<h2>2 · Leiterplatten-Spezifikation</h2>
<div class="spec">
  <div class="k">Abmessungen</div><div class="v">ca. 10&nbsp;×&nbsp;20&nbsp;mm</div>
  <div class="k">Lagen</div><div class="v">doppelseitig (2 Lagen, Top + Bottom Kupfer)</div>
  <div class="k">Typ</div><div class="v">Versuchs-/Prototyp-Leiterplatte</div>
  <div class="k">Bestückung</div><div class="v">SMD-Bauteile (U1, Q1, Q2, R1, R2) + Lötpads für Kabel (H1–H3)</div>
  <div class="k">Anschlüsse</div><div class="v">Kabel werden direkt angelötet (siehe Abschnitt 4)</div>
  <div class="k">Daten</div><div class="v">Vollständige Fertigungsdaten (Gerber, BOM, Pick&amp;Place) werden auf Anfrage geliefert</div>
</div>
<p class="note"><b>Hinweis:</b> Die beigefügten Bilder (Schaltplan &amp; Layout-Vorschau)
dienen der Übersicht für die Offerte. Die verbindlichen CAD-/Gerber-Daten stellen wir vor
Fertigungsfreigabe bereit.</p>
</div>

<div class="sec">
<h2>3 · Stückliste (BOM)</h2>
<table>
  <thead><tr><th>Pos.</th><th>Bauteil / Hersteller-Nr.</th><th>Funktion</th><th>Gehäuse</th><th>Wert</th></tr></thead>
  <tbody>
    <tr><td class="ref">U1</td><td><code>DRV5032FBDBZT</code> (Texas Instruments)</td><td>Hall-Effekt-Magnetschalter (Sensor)</td><td>SOT-23 (DBZ), 3-Pin</td><td>—</td></tr>
    <tr><td class="ref">Q1</td><td><code>SI2333DDS-T1-GE3</code> (Vishay)</td><td>P-Kanal MOSFET (Lastschalter VCC)</td><td>lt. Datenblatt</td><td>—</td></tr>
    <tr><td class="ref">Q2</td><td><code>MMBT3904</code> (Range 100–300)</td><td>NPN-Transistor (Treiberstufe)</td><td>SOT-23</td><td>—</td></tr>
    <tr><td class="ref">R1</td><td>Widerstand</td><td>Basis-Vorwiderstand Q2</td><td>SMD (0402/0603)</td><td>10&nbsp;kΩ</td></tr>
    <tr><td class="ref">R2</td><td>Widerstand</td><td>Gate-Pull-up Q1</td><td>SMD (0402/0603)</td><td>10&nbsp;kΩ</td></tr>
    <tr><td class="ref">H1</td><td>Lötpad (1-polig)</td><td>Anschluss (siehe Schaltplan)</td><td>1× Lötpad</td><td>—</td></tr>
    <tr><td class="ref">H2</td><td>Lötpads (3-polig)</td><td>Akku-Anschluss</td><td>3× Lötpad</td><td>—</td></tr>
    <tr><td class="ref">H3</td><td>Lötpads (3-polig)</td><td>Anschluss zur ST-PCB</td><td>3× Lötpad</td><td>—</td></tr>
  </tbody>
</table>
<p class="cap">Gehäuse-/Footprint-Angaben gemäss Hersteller-Datenblatt bzw. den noch zu liefernden CAD-Daten; Widerstands-Bauform nach Absprache.</p>
</div>

<div class="sec">
<h2>4 · Bestückungs- &amp; Anschlusshinweise</h2>
<ul>
  <li><b>H2 — Akku:</b> 3 Kabel vom Akku werden direkt auf die Leiterplatte angelötet.</li>
  <li><b>H3 — ST-PCB:</b> Kabel mit Stecker (zur ST-PCB) hier anlöten. <b>Anschluss&nbsp;1 ist bewusst NC</b> (not connected) — siehe Schaltplan (durchgestrichen).</li>
  <li><b>H1:</b> einzelner Anschluss gemäss Schaltplan.</li>
  <li>Bestückung der SMD-Bauteile (U1, Q1, Q2, R1, R2) durch den Fertiger; Kabel-Anlötung kann auf unserer Seite erfolgen — bitte beide Varianten (mit/ohne Kabelkonfektion) offerieren.</li>
</ul>
</div>

<div class="pagebreak"></div>
<div class="sec">
<h2>5 · Schaltplan</h2>
<div class="fig">
  <img src="{sch}" alt="Schaltplan Magnetschalter">
  <div class="cap">Schaltplan: U1 DRV5032FBDBZT (Hall) → R1 → Q2 MMBT3904 → R2 → Q1 SI2333DDS (P-MOSFET) schaltet VCC. H2 = Akku, H3 = ST-PCB (Pin 1 NC).</div>
</div>
</div>

<div class="sec">
<h2 style="margin-top:22px;">6 · Layout-Vorschau (Top/Bottom)</h2>
<div class="fig">
  <img src="{pcb}" alt="Leiterplatten-Layout">
  <div class="cap">Layout-Vorschau, doppelseitig, ca. 10 × 20 mm. Rot = eine Kupferlage, Blau = zweite Kupferlage, Violett = Umriss. Maßstäblich nicht massgebend — verbindliche Maße aus den Gerber-Daten.</div>
</div>
</div>

<div class="sec">
<h2>7 · Anfrage / gewünschte Offerte</h2>
<p>Wir bitten um eine Offerte für <b>Fertigung und Bestückung (PCBA)</b> dieser Versuchs­leiterplatte, idealerweise gestaffelt:</p>
<ul>
  <li>Stückzahlen: <b>5 / 10 / 25 Stück</b> (Prototyp-Mengen)</li>
  <li>Leiterplatten-Fertigung (2-Lagen, ca. 10 × 20 mm) inkl. Lötstopplack &amp; Bestückungsdruck</li>
  <li>SMD-Bestückung gemäss BOM (Abschnitt 3)</li>
  <li>Optional: Bauteil-Beschaffung durch Variosystems vs. Beistellung durch uns — bitte beide Varianten ausweisen</li>
  <li>Optional: Kabelkonfektion / Anlötung der Kabel (H2/H3)</li>
  <li>Voraussichtliche Lieferzeit ab Datenfreigabe</li>
  <li>Einmalige Kosten (NRE, Schablone/Stencil) separat ausweisen</li>
</ul>
<p>Die vollständigen Fertigungsdaten (Gerber, Bohrdaten, BOM mit Hersteller-Teilenummern,
Pick&amp;Place) liefern wir umgehend auf Wunsch nach. Für Rückfragen stehen wir gerne zur
Verfügung.</p>
</div>

<div class="foot">
  Anfrage erstellt am 15.06.2026 · Kontakt: Zeno Davatz, zdavatz@gmail.com · Design: Peter Schmidlin ·
  Projekt: Pumpfoil-Sensor / Magnetschalter-Versuchsaufbau
</div>

</body>
</html>"##
    )
}

fn main() -> Result<()> {
    let args = Args::parse();
    let output = args.output.unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join("Downloads/Anfrage-Magnetschalter-LP-Variosystems.pdf")
    });

    let html = render_html();
    let html_path = output.with_extension("html");
    std::fs::write(&html_path, &html)?;

    let chrome = chrome_binary();
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
