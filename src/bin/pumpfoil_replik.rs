//! Replik von Pump Tsüri auf die Antwort des Stadtrats (GR Nr. 2026/250,
//! Beschluss 2806/2026) zum Pumpfoiling am Zürichsee.
//!
//! Scratch bin (gitignored). Same pipeline as `magnetswitch_rfq`: one
//! static HTML string → `<output>.html` → headless Chrome `--print-to-pdf`.
//! Afterwards the original Stadtrat PDF is downloaded and appended as the
//! trailing pages (via `pdfunite`, falling back to `qpdf`).
//!
//! ```bash
//! cargo run --release --bin pumpfoil_replik            # ~/Downloads/...
//! cargo run --release --bin pumpfoil_replik -- -o /tmp/replik.pdf
//! ```

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHROME_MAC: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const URL_ANFRAGE: &str =
    "https://pump.zuerich/wp-content/uploads/2026/05/2026_0250-Schriftliche-Anfrage.pdf";
const URL_ANTWORT: &str =
    "https://pump.zuerich/wp-content/uploads/2026/09/2026_0250-Antwort-Stadtrat.pdf";
const URL_BSG: &str = "https://www.fedlex.admin.ch/eli/cc/1976/725_724_724/de";
const URL_BSV: &str = "https://www.fedlex.admin.ch/eli/cc/1979/337_337_337/de";
const URL_PUMPTSUERI: &str = "https://pump.zuerich/";

fn chrome_binary() -> String {
    if let Ok(env_path) = std::env::var("CHROME") {
        return env_path;
    }
    [
        CHROME_MAC,
        "/usr/bin/google-chrome-stable",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ]
    .iter()
    .find(|p| Path::new(p).exists())
    .map(|p| p.to_string())
    .unwrap_or_else(|| CHROME_MAC.into())
}

#[derive(Parser, Debug)]
#[command(version, about = "Render the Pump Tsüri Replik PDF (GR 2026/250)")]
struct Args {
    /// PDF output path. Default: ~/Downloads/Replik-Pump-Tsueri-GR-2026-250.pdf
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Skip appending the original Stadtrat PDF.
    #[arg(long)]
    no_append: bool,
}

fn render_html() -> String {
    format!(
        r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8">
<title>Replik Pump Tsüri – GR Nr. 2026/250</title>
<style>
  @page {{ size: A4; margin: 16mm 17mm 16mm 17mm; }}
  * {{ box-sizing: border-box; }}
  body {{ font-family: "Helvetica Neue", Arial, sans-serif; color: #1a1a1a;
    font-size: 10.5pt; line-height: 1.5; margin: 0; }}
  h1 {{ font-size: 19pt; margin: 0 0 4px 0; letter-spacing: -0.2px; line-height: 1.2; }}
  h2 {{ font-size: 12.5pt; margin: 22px 0 8px 0; padding-bottom: 4px;
    border-bottom: 2px solid #0b6e99; color: #0b6e99;
    break-after: avoid; page-break-after: avoid; }}
  h3 {{ font-size: 11pt; margin: 14px 0 4px 0; break-after: avoid; }}
  .sub {{ color: #555; font-size: 10pt; margin: 0 0 12px 0; }}
  .meta {{ display: flex; gap: 18px; background: #f3f7fa; border: 1px solid #d9e4ec;
    border-radius: 6px; padding: 11px 14px; font-size: 9.5pt; margin: 12px 0 6px 0;
    break-inside: avoid; }}
  .meta .blk {{ flex: 1; }}
  .meta .lbl {{ color: #7a8a96; text-transform: uppercase; font-size: 7.5pt;
    letter-spacing: 0.6px; display: block; margin-bottom: 1px; }}
  .sec {{ break-inside: avoid; }}
  .sec.long {{ break-inside: auto; }}
  p {{ margin: 6px 0; }}
  ul, ol {{ margin: 4px 0 6px 0; padding-left: 20px; }}
  li {{ margin: 4px 0; break-inside: avoid; }}
  blockquote {{ margin: 8px 0; padding: 8px 14px; border-left: 3px solid #0b6e99;
    background: #f7f9fb; font-size: 9.8pt; color: #333; break-inside: avoid; }}
  blockquote .src {{ display: block; color: #6b7a86; font-size: 8.5pt; margin-top: 4px; }}
  table {{ border-collapse: collapse; width: 100%; font-size: 9.5pt; margin: 6px 0; }}
  th, td {{ border: 1px solid #d8dde2; padding: 5px 8px; text-align: left; vertical-align: top; }}
  th {{ background: #2a3a46; color: #fff; font-weight: 600; font-size: 8.5pt;
    text-transform: uppercase; letter-spacing: 0.4px; }}
  tr {{ break-inside: avoid; }}
  tr:nth-child(even) td {{ background: #fafbfc; }}
  .lever {{ border: 1px solid #d9e4ec; border-radius: 6px; padding: 10px 14px;
    margin: 10px 0; break-inside: avoid; }}
  .lever h3 {{ margin: 0 0 4px 0; color: #0b6e99; }}
  .lever .who {{ font-size: 8.5pt; color: #6b7a86; text-transform: uppercase;
    letter-spacing: 0.5px; }}
  .box {{ background: #fff8e6; border: 1px solid #f0d58c; border-radius: 6px;
    padding: 10px 14px; margin: 10px 0; break-inside: avoid; }}
  a {{ color: #0b6e99; text-decoration: none; }}
  .links li {{ word-break: break-all; }}
  .foot {{ margin-top: 26px; font-size: 9pt; color: #6b7a86; border-top: 1px solid #ddd;
    padding-top: 8px; }}
</style></head><body>

<h1>Replik zur Antwort des Stadtrats<br>betreffend Pumpfoiling am Zürichsee</h1>
<p class="sub">Stellungnahme von Pump Tsüri zur Schriftlichen Anfrage GR Nr. 2026/250
(Eggenschwiler, Denoth, Blättler, SP) und zum Beschluss des Stadtrats Nr. 2806/2026
vom 26. August 2026</p>

<div class="meta">
  <div class="blk"><span class="lbl">Von</span>Pump Tsüri, Zürich<br>
    <a href="{pt}">pump.zuerich</a></div>
  <div class="blk"><span class="lbl">An</span>Stadtrat der Stadt Zürich<br>
    Sportamt · Gemeinderat (SP-Fraktion)</div>
  <div class="blk"><span class="lbl">Datum</span>3. September 2026</div>
  <div class="blk"><span class="lbl">Betrifft</span>GR Nr. 2026/250 · STRB 2806/2026</div>
</div>

<div class="sec">
<h2>1. Was wir begrüssen</h2>
<p>Der Stadtrat anerkennt in seiner Antwort ausdrücklich, dass aus Sportsicht ein Bedarf
an geeigneten und sicheren Trainings- und Einstiegsmöglichkeiten besteht (Frage 1), dass
öffentlich zugängliche Infrastruktur zu einer sicheren, geordneten und konfliktarmen
Ausübung beiträgt (Frage 2) und dass das Sportamt zusammen mit Grün Stadt Zürich, dem
AWEL und der Wasserschutzpolizei konkrete Ansätze prüft: Nutzung von Stegen, Mitnutzung
von Piers, Startrampen, Flösse ausserhalb der Badesaison (Frage 4).</p>
<p>Das ist eine gute Grundlage. Rund 250 Personen trainieren heute regelmässig in Kursen,
die Community wird auf rund 500 Aktive geschätzt. Seit dem Widerruf der Bewilligung im
Mai 2025 gibt es in der Stadt Zürich keinen einzigen offiziellen Übungsplatz.</p>
</div>

<div class="sec">
<h2>2. Wo wir widersprechen</h2>
<p>Die Antwort stützt den Widerruf auf das Bundesrecht: Pumpfoils seien Schiffe, gelb
markierte Flächen seien «ganzjährig gesperrt», Lösungen müssten deshalb ausserhalb
gefunden werden. Diese Darstellung ist rechtlich unvollständig. Das Bundesrecht
definiert zwar den Begriff Schiff und die Bedeutung der gelben Bojen. <strong>Wo, wann
und für wen eine Wasserfläche gesperrt ist, entscheidet nicht der Bund, sondern die
zuständige kantonale Behörde.</strong> Der Kanton hat damit den Schlüssel selbst in der
Hand.</p>
</div>

<div class="sec long">
<h2>3. Rechtsgrundlagen (verifiziert am Verordnungstext)</h2>

<blockquote>«Schiff: ein Wasserfahrzeug oder ein anderer zur Fortbewegung auf oder unter der
Wasseroberfläche bestimmter Schwimmkörper, oder ein schwimmendes Gerät»
<span class="src">Art. 2 Abs. 1 Bst. a Ziff. 1 BSV (SR 747.201.1)</span></blockquote>
<p>Die Definition ist bewusst weit. Sie erfasst auch Segelbretter, Drachensegelbretter
(Kitesurf), Paddelboote und sogar Luftmatratzen («Strandboote», Ziff. 20). Ein Pumpfoil
ist darunter also ebenso ein «Schiff» wie ein Stand-up-Paddle. Daraus folgt kein
Wettkampf- oder Gefährdungscharakter, sondern nur, dass die Verkehrsregeln der BSV
gelten.</p>

<blockquote>«Die zuständige Behörde bestimmt, wo welche Schifffahrtszeichen angebracht oder
entfernt werden.» <span class="src">Art. 36 Abs. 2 BSV</span></blockquote>
<blockquote>«Für die Schifffahrt gesperrte Wasserflächen sind mit gelben, kugelförmigen
Schwimmkörpern gekennzeichnet. [...] Für bestimmte Schiffsarten gesperrte Wasserflächen
sind mit gelben, kugelförmigen Schwimmkörpern und mit den betreffenden Tafeln (A.2, A.3
oder A.4) gekennzeichnet.» <span class="src">Art. 37 Abs. 1 und 2 BSV</span></blockquote>
<p>Die Sperrzone entsteht durch die kantonale Signalisation, nicht durch das Gesetz.
Der Bund kennt ausdrücklich <em>teilgesperrte</em> Flächen: Sperrung nur für
Motorschiffe (A.2), nur für Wasserski (A.3), nur für Segelschiffe (A.4). Eine Sperrung
«für alle Schiffe ausser muskelbetriebenen Wassersportgeräten im Kursbetrieb ausserhalb
der Badeöffnungszeiten» ist in diesem System ohne Bundesrechtsänderung abbildbar.
Auch die zeitliche Dimension ist kantonal: Wo die Bojen im Winter entfernt werden,
besteht keine Sperrzone.</p>

<blockquote>«Bei der Bewilligung von nautischen Veranstaltungen kann die zuständige Behörde
Ausnahmen von einzelnen Bestimmungen dieser Verordnung zulassen, wenn die Sicherheit der
Schifffahrt nicht beeinträchtigt wird.» <span class="src">Art. 72 Abs. 3 BSV</span></blockquote>
<p>Das ist die ausdrückliche bundesrechtliche Ausnahmekompetenz des Kantons. Ein
betreuter Kurs in Kleingruppen (max. sechs Personen pro Lehrperson, fixes Zeitfenster,
Haftpflicht) ist eine bewilligungsfähige Veranstaltung. Die Aussage, Ausnahmen seien
«grundsätzlich nicht vorgesehen», trifft für Art. 72 Abs. 3 nicht zu.</p>

<blockquote>«In Uferzonen ist das Wakesurfen sowie das Fahren mit Wasserski oder ähnlichen
Geräten ausserhalb behördlich bewilligter Startgassen und gekennzeichneter, ausschliesslich
diesem Zweck dienender Wasserflächen verboten.» <span class="src">Art. 54 Abs. 2 BSV</span></blockquote>
<p>Für Wasserski, Wakesurf und Kitesurf hat das Bundesrecht das Instrument der
<em>behördlich bewilligten Startgasse</em> geschaffen (Tafeln E.5 und E.5ter, Art. 37
Abs. 3 und 6 BSV). Der Kanton kann solche Flächen in der Uferzone ausweisen. Für ein
Sportgerät ohne Motor, ohne Segel und ohne Schleppseil, das im Übungsbetrieb wenige
Meter vom Steg entfernt bleibt, ist dieselbe Logik anwendbar.</p>

<p>Die Gewässerhoheit liegt beim Kanton (Art. 3 BSG, SR 747.201); der Stadtrat bestätigt
dies selbst in seiner Antwort zu Frage 3. Der Kanton ist damit nicht Vollzugsgehilfe des
Bundes, sondern Träger eines eigenen Ermessens.</p>
</div>

<div class="sec long">
<h2>4. Fünf Hebel für Stadt und Kanton</h2>

<div class="lever"><div class="who">Kanton (AWEL) · sofort umsetzbar</div>
<h3>Hebel 1: Sperrzonen befristen und differenzieren</h3>
<p>Badezonen werden zum Schutz von Schwimmenden gesperrt. Ist die Badi geschlossen,
fällt der Schutzzweck weg. Das AWEL kann die Sperrung nach Art. 36 Abs. 2 und Art. 37
Abs. 2 BSV auf die Badeöffnungszeiten beschränken oder für muskelbetriebene
Wassersportgeräte im bewilligten Kursbetrieb öffnen. Keine Änderung von Bundesrecht
nötig, nur der kantonalen Signalisation.</p></div>

<div class="lever"><div class="who">Kanton (AWEL) · sofort umsetzbar</div>
<h3>Hebel 2: Kurse als nautische Veranstaltung bewilligen</h3>
<p>Art. 72 Abs. 3 BSV erlaubt dem Kanton ausdrücklich Ausnahmen von einzelnen
Bestimmungen der Verordnung. Eine Saison-Bewilligung für den Kursbetrieb auf definierten
Flössen ausserhalb der Badeöffnungszeiten, mit Auflagen (Gruppengrösse, Lehrperson,
Haftpflicht, Sichtweite zu Schwimmenden), entspricht dem Modell 2021 bis 2025.</p></div>

<div class="lever"><div class="who">Stadt (Sportamt, Grün Stadt Zürich)</div>
<h3>Hebel 3: Konzession anpassen, Startrampe ausserhalb der Zone</h3>
<p>Die Stadt ist Konzessionärin ihrer Badis, Stege und Flösse. Sie kann beim AWEL eine
Konzessionsänderung beantragen: Übungssteg oder Startrampe unmittelbar ausserhalb der
gelben Bojen, oder ein Floss, das ausserhalb der Badesaison ausserhalb der Zone
verankert wird. Das ist der Weg, den der Stadtrat in Frage 4 selbst skizziert; wir
bitten um einen Zeitplan mit Ziel Saison 2027.</p></div>

<div class="lever"><div class="who">Kanton (Regierungsrat, Kantonsrat)</div>
<h3>Hebel 4: Übungszonen in der kantonalen Schifffahrtsverordnung</h3>
<p>Analog zu den Kitesurf-Startgassen kann der Kanton Zürich in seiner
Schifffahrtsverordnung Übungszonen für muskelbetriebene Wassersportgeräte am Zürichsee
ausweisen. Damit erhält der Sport eine dauerhafte, stadtunabhängige Grundlage.</p></div>

<div class="lever"><div class="who">Bund (BAV, Bundesrat) · mittelfristig</div>
<h3>Hebel 5: Eigene Kategorie in der BSV</h3>
<p>Die BSV kennt bereits Segelbrett, Drachensegelbrett, Paddelboot und Strandboot. Eine
Kategorie «muskelbetriebenes Wassersportgerät» (Pumpfoil, Bodyboard, Schwimmbrett mit
Foil) mit eigenen, auf den Übungsbetrieb zugeschnittenen Regeln gehört in die nächste
BSV-Revision. Wege: Stellungnahme des Kantons in der Vernehmlassung, Standesinitiative,
Vorstoss im Nationalrat durch Zürcher Parlamentarier, gemeinsame Eingabe der Verbände
beim Bundesamt für Verkehr.</p></div>
</div>

<div class="sec long">
<h2>5. Warum Übungsbetrieb kein Schifffahrtsrisiko ist</h2>
<table>
<tr><th>Merkmal</th><th>Pumpfoil im Übungsbetrieb</th><th>Bezug BSV</th></tr>
<tr><td>Antrieb</td><td>Muskelkraft, kein Motor, kein Segel, kein Schleppseil</td>
  <td>Kein Motorschiff (Art. 2 Ziff. 2), kein Segelschiff (Ziff. 9)</td></tr>
<tr><td>Reichweite Anfänger</td><td>5 bis 30 m ab Steg, dann Sturz ins Wasser</td>
  <td>Innere Uferzone (Art. 53), keine Fahrlinie berührt</td></tr>
<tr><td>Geschwindigkeit</td><td>10 bis 20 km/h, Gleitphase wenige Sekunden</td>
  <td>Unter Tempolimit Uferzone (Art. 53 Abs. 1 Bst. b)</td></tr>
<tr><td>Anwesende Dritte</td><td>Ausserhalb Badeöffnungszeiten: keine Schwimmenden</td>
  <td>Schutzzweck der Sperrzone entfällt</td></tr>
<tr><td>Betreuung</td><td>Max. 6 Personen pro Lehrperson, Sichtkontakt</td>
  <td>Auflage nach Art. 72 Abs. 2 Bst. a</td></tr>
<tr><td>Emissionen</td><td>Keine Lärm-, Abgas- oder Wellenbelastung</td>
  <td>Keine Beeinträchtigung nach Art. 72 Abs. 2</td></tr>
</table>
<p>Die Verletzungsgefahr durch scharfe Foil-Kanten, die der Stadtrat anführt, ist real.
Sie besteht aber gegenüber Schwimmenden, nicht gegenüber der Schifffahrt. Genau deshalb
ist der Übungsbetrieb ausserhalb der Badeöffnungszeiten auf einem abgegrenzten Floss die
<em>sicherste</em> Variante: räumlich und zeitlich getrennt von Badenden, weit weg von
Fahrlinien. Ohne offizielle Plätze verlagert sich das Üben an ungesicherte Stellen.</p>
</div>

<div class="sec long">
<h2>6. Unsere Anliegen an Stadtrat und Kanton</h2>
<ol>
<li><strong>Übergangslösung 2027:</strong> Saisonbewilligung nach Art. 72 Abs. 3 BSV für
den Kursbetrieb auf zwei bis drei Badi-Flössen ausserhalb der Badeöffnungszeiten, mit
den bewährten Auflagen von 2021 bis 2025.</li>
<li><strong>Signalisation prüfen:</strong> Das AWEL prüft für diese Standorte eine zeitlich
befristete oder nach Schiffsart differenzierte Sperrung (Art. 36 Abs. 2, Art. 37 Abs. 2
BSV).</li>
<li><strong>Zeitplan für Infrastruktur:</strong> Das Sportamt legt bis Ende 2026 einen
Zeitplan für Startrampe oder Übungssteg ausserhalb der Sperrzonen vor, inkl. Wollishofen
und Marina Tiefenbrunnen.</li>
<li><strong>Einbezug:</strong> Pump Tsüri und Pump Foil Zürichsee werden neben ASVZ und
Supkultur formell in die Arbeitsgruppe aufgenommen (Frage 5 nennt sie nicht).</li>
<li><strong>Bundesebene:</strong> Der Kanton bringt in der nächsten BSV-Revision eine
Kategorie für muskelbetriebene Wassersportgeräte ein.</li>
</ol>
</div>

<div class="box">
<strong>Kernaussage:</strong> Nicht das Bundesgesetz verhindert Pumpfoil-Kurse auf den
Flössen. Die Sperrzone ist eine kantonale Signalisation, und Art. 72 Abs. 3 BSV gibt
dem Kanton eine ausdrückliche Ausnahmekompetenz. Was fehlt, ist der politische Wille,
sie zu nutzen.
</div>

<div class="sec">
<h2>7. Dokumente</h2>
<ul class="links">
<li>Schriftliche Anfrage GR Nr. 2026/250 (27. Mai 2026):<br>
  <a href="{anfrage}">{anfrage}</a></li>
<li>Antwort des Stadtrats, Beschluss Nr. 2806/2026 (26. August 2026), im Anhang
  vollständig beigelegt:<br><a href="{antwort}">{antwort}</a></li>
<li>Bundesgesetz über die Binnenschifffahrt (BSG, SR 747.201):<br>
  <a href="{bsg}">{bsg}</a></li>
<li>Binnenschifffahrtsverordnung (BSV, SR 747.201.1):<br>
  <a href="{bsv}">{bsv}</a></li>
</ul>
</div>

<div class="foot">Pump Tsüri · <a href="{pt}">pump.zuerich</a> · Zitate aus der BSV nach
dem konsolidierten Text auf fedlex.admin.ch. Anhang: Originalantwort des Stadtrats
(4 Seiten).</div>

</body></html>
"#,
        pt = URL_PUMPTSUERI,
        anfrage = URL_ANFRAGE,
        antwort = URL_ANTWORT,
        bsg = URL_BSG,
        bsv = URL_BSV,
    )
}

fn print_pdf(html_path: &Path, pdf_path: &Path) -> Result<()> {
    let chrome = chrome_binary();
    let status = Command::new(&chrome)
        .args([
            "--headless=new",
            "--disable-gpu",
            "--no-pdf-header-footer",
            &format!("--print-to-pdf={}", pdf_path.display()),
            &format!("file://{}", std::fs::canonicalize(html_path)?.display()),
        ])
        .status()
        .with_context(|| format!("spawn {chrome}"))?;
    if !status.success() {
        return Err(anyhow!("Chrome print-to-pdf exited {status}"));
    }
    Ok(())
}

async fn download(url: &str, to: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (pumpfoil-replik)")
        .build()?;
    let resp = client.get(url).send().await?.error_for_status()?;
    let bytes = resp.bytes().await?;
    // WordPress occasionally prepends a stray newline; pdfunite/qpdf choke
    // on anything before `%PDF`, so trim leading whitespace first.
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(0);
    let bytes = &bytes[start..];
    if !bytes.starts_with(b"%PDF") {
        return Err(anyhow!("{url} did not return a PDF"));
    }
    std::fs::write(to, bytes)?;
    Ok(())
}

fn merge(parts: &[&Path], out: &Path) -> Result<()> {
    let mut cmd = Command::new("pdfunite");
    cmd.args(parts).arg(out);
    if let Ok(st) = cmd.status() {
        if st.success() {
            return Ok(());
        }
    }
    let mut q = Command::new("qpdf");
    q.arg("--empty").arg("--pages");
    for p in parts {
        q.arg(p);
    }
    q.arg("--").arg(out);
    let st = q.status().context("neither pdfunite nor qpdf available")?;
    if !st.success() {
        return Err(anyhow!("qpdf merge exited {st}"));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let output = args.output.unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join("Downloads/Replik-Pump-Tsueri-GR-2026-250.pdf")
    });

    let html_path = output.with_extension("html");
    std::fs::write(&html_path, render_html())?;

    if args.no_append {
        print_pdf(&html_path, &output)?;
    } else {
        let body_pdf = output.with_extension("body.pdf");
        let annex_pdf = output.with_extension("annex.pdf");
        print_pdf(&html_path, &body_pdf)?;
        download(URL_ANTWORT, &annex_pdf).await?;
        merge(&[&body_pdf, &annex_pdf], &output)?;
        let _ = std::fs::remove_file(&body_pdf);
        let _ = std::fs::remove_file(&annex_pdf);
    }
    eprintln!("wrote {}", output.display());
    eprintln!("wrote {}", html_path.display());
    Ok(())
}
