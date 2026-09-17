//! "Pumpfoilen in der gelben Zone mit Bewilligung — rechtliches Vorgehen".
//! Memo von Pump Tsüri: Rechtslage (BSV/BSG), drei Stufen des Vorgehens,
//! Formulierungsvorschläge. Statischer deutscher Text.
//!
//! Scratch bin (gitignored). Gleiche Pipeline wie `pumpfoil_replik`:
//! HTML-String → `<output>.html` → headless Chrome `--print-to-pdf`.
//!
//! ```bash
//! cargo run --release --bin pumpfoil_rechtsweg
//! cargo run --release --bin pumpfoil_rechtsweg -- -o /tmp/rechtsweg.pdf
//! ```

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHROME_MAC: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

const URL_BSG: &str = "https://www.fedlex.admin.ch/eli/cc/1976/725_724_724/de";
const URL_BSV: &str = "https://www.fedlex.admin.ch/eli/cc/1979/337_337_337/de";
const URL_ANTWORT: &str =
    "https://pump.zuerich/wp-content/uploads/2026/09/2026_0250-Antwort-Stadtrat.pdf";
const URL_REPLIK: &str =
    "https://pump.zuerich/wp-content/uploads/2026/09/2026_0250-Replik-Pump-Tsueri.pdf";
const URL_PT: &str = "https://pump.zuerich/";

fn chrome_binary() -> String {
    if let Ok(p) = std::env::var("CHROME") {
        return p;
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
#[command(version, about = "Render the Pump Tsüri legal-route memo PDF")]
struct Args {
    /// PDF output path. Default: ~/Downloads/Pumpfoil-gelbe-Zone-Rechtsweg.pdf
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn render_html() -> String {
    format!(
        r#"<!doctype html>
<html lang="de"><head><meta charset="utf-8">
<title>Pumpfoilen in der gelben Zone – rechtliches Vorgehen</title>
<style>
  @page {{ size: A4; margin: 16mm 17mm 16mm 17mm; }}
  * {{ box-sizing: border-box; }}
  body {{ font-family: "Helvetica Neue", Arial, sans-serif; color: #1a1a1a;
    font-size: 10.5pt; line-height: 1.5; margin: 0; }}
  h1 {{ font-size: 19pt; margin: 0 0 4px 0; letter-spacing: -0.2px; line-height: 1.2; }}
  h2 {{ font-size: 12.5pt; margin: 20px 0 8px 0; padding-bottom: 4px;
    border-bottom: 2px solid #0b6e99; color: #0b6e99; break-after: avoid; }}
  h3 {{ font-size: 11pt; margin: 0 0 4px 0; color: #0b6e99; break-after: avoid; }}
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
  .step {{ border: 1px solid #d9e4ec; border-radius: 6px; padding: 10px 14px;
    margin: 10px 0; break-inside: auto; }}
  .step .who {{ font-size: 8.5pt; color: #6b7a86; text-transform: uppercase; letter-spacing: 0.5px; }}
  .draft {{ background: #f4f8f4; border: 1px dashed #8fb08f; border-radius: 6px;
    padding: 8px 12px; margin: 8px 0; font-size: 9.8pt; break-inside: avoid; }}
  .draft .lbl {{ display:block; font-size: 8pt; color:#5b7a5b; text-transform: uppercase;
    letter-spacing: .5px; margin-bottom: 2px; }}
  .box {{ background: #fff8e6; border: 1px solid #f0d58c; border-radius: 6px;
    padding: 10px 14px; margin: 10px 0; break-inside: avoid; }}
  a {{ color: #0b6e99; text-decoration: none; }}
  .links li {{ word-break: break-all; }}
  .foot {{ margin-top: 22px; font-size: 9pt; color: #6b7a86; border-top: 1px solid #ddd; padding-top: 8px; }}
</style></head><body>

<h1>Pumpfoilen in der gelben Zone mit Bewilligung</h1>
<p class="sub">Wie muss das Recht angepasst werden, und wie geht man rechtlich vor?
Memo zur Schriftlichen Anfrage GR Nr. 2026/250 und zum Beschluss des Stadtrats
Nr. 2806/2026 vom 26. August 2026</p>

<div class="meta">
  <div class="blk"><span class="lbl">Von</span>Pump Tsüri, Zürich<br><a href="{pt}">pump.zuerich</a></div>
  <div class="blk"><span class="lbl">Für</span>Vereine, Gemeinderat, Sportamt</div>
  <div class="blk"><span class="lbl">Datum</span>17. September 2026</div>
  <div class="blk"><span class="lbl">Status</span>Arbeitspapier, keine Rechtsberatung</div>
</div>

<div class="box"><strong>Kernaussage:</strong> Das Bundesrecht muss nicht geändert werden.
Die gelbe Zone ist keine Bundesvorschrift, sondern eine kantonale Anordnung. Wer eine
Sperre verfügt, kann sie auch anders ausgestalten. Der schnellste Weg führt über ein
formelles Gesuch mit anfechtbarer Verfügung und parallel über eine Anpassung der
kantonalen Sperre.</div>

<div class="sec long">
<h2>1. Rechtslage</h2>

<h3>Die Sperre ist kantonales Recht</h3>
<blockquote>«Die Kantone vollziehen diese Verordnung. Sie dokumentieren raumbezogen die
Einschränkungen und die Verbote, die sie in Anwendung von Artikel 3 Absatz 2 BSG für die
Schifffahrt erlassen haben.» <span class="src">Art. 165 Abs. 1 und 1bis BSV (SR 747.201.1)</span></blockquote>
<p>Die BSV sagt damit selbst, woher die gelben Zonen kommen: Es sind Verbote, die der
<em>Kanton</em> gestützt auf Art. 3 Abs. 2 BSG erlassen hat. Das Bundesrecht regelt nur,
wie eine solche Fläche gekennzeichnet wird (gelbe Bojen, Art. 37 Abs. 1 BSV) und dass die
zuständige Behörde bestimmt, wo Zeichen angebracht oder entfernt werden (Art. 36 Abs. 2 BSV).</p>

<h3>Worauf sich das AWEL vermutlich stützt</h3>
<blockquote>«Die zuständige Behörde kann Ausnahmen zulassen von den Bestimmungen in:
a. Artikel 53 Absatz 1 Buchstabe a [...]; b. Artikel 54 Absätze 5 und 6; das Schleppen von
mehr als zwei Wasserskifahrern sowie von Fluggeräten kann zu Trainingszwecken auf
bestimmten Gewässerabschnitten gestattet werden; c. Artikel 70 [...]»
<span class="src">Art. 163 Abs. 1 BSV (Auszug)</span></blockquote>
<p>Art. 163 zählt abschliessend auf, von welchen <em>Bundesbestimmungen</em> die Behörde
Ausnahmen machen darf. Gesperrte Wasserflächen stehen nicht in dieser Liste. Daraus
leitet das AWEL ab, Ausnahmen seien «grundsätzlich nicht vorgesehen».</p>

<h3>Warum das nicht das letzte Wort ist</h3>
<ul>
<li>Art. 163 regelt Ausnahmen von Bestimmungen der BSV. Die konkrete Sperre einer
Badezone ist aber eine kantonale Anordnung. Der Kanton braucht keine Bundesausnahme,
um seine eigene Anordnung zeitlich oder sachlich zu begrenzen.</li>
<li>Für bewilligte Veranstaltungen besteht eine eigene Ausnahmekompetenz:</li>
</ul>
<blockquote>«Wettfahrten, Festlichkeiten auf dem Wasser und sonstige Veranstaltungen, die zu
Ansammlungen von Schiffen oder zu Verkehrsbehinderungen führen können, bedürfen der
Bewilligung der zuständigen Behörde. [...] Bei der Bewilligung von nautischen
Veranstaltungen kann die zuständige Behörde Ausnahmen von einzelnen Bestimmungen dieser
Verordnung zulassen, wenn die Sicherheit der Schifffahrt nicht beeinträchtigt wird.»
<span class="src">Art. 72 Abs. 1 und 3 BSV</span></blockquote>
<ul>
<li>Ein betreuter Kurs ist eine «sonstige Veranstaltung». Ein Wettkampf ist nicht
vorausgesetzt. Bei der Schulung finden keine Wettkämpfe statt.</li>
<li>Die BSV kennt bereits eine Ausnahme «zu Trainingszwecken» (Art. 163 Abs. 1 Bst. b,
Wasserski). Der Gedanke ist dem Verordnungsgeber also vertraut.</li>
<li>Der Schutzzweck der Badezone ist der Schutz von Badenden. Ausserhalb der
Badeöffnungszeiten ist niemand im Wasser.</li>
</ul>
</div>

<div class="sec long">
<h2>2. Vorgehen in drei Stufen</h2>

<div class="step"><div class="who">Stufe 1 · ohne Rechtsänderung · Wochen bis Monate</div>
<h3>Gesuch ans AWEL und Rechtsweg</h3>
<ul>
<li>Formelles Gesuch um Bewilligung des Kursbetriebs als nautische Veranstaltung nach
Art. 72 BSV, mit Ausnahme nach Abs. 3, für konkret bezeichnete Flösse, Wochentage und
Zeitfenster ausserhalb der Badeöffnungszeiten.</li>
<li>Beilagen: Sicherheitskonzept (max. sechs Personen pro Lehrperson, Sichtkontakt,
Abbruchkriterien), Haftpflichtnachweis, Zustimmung der Stadt als Eigentümerin der Flösse,
Erfahrungsbericht 2021 bis 2025.</li>
<li>Ausdrücklich eine <strong>anfechtbare Verfügung mit Rechtsmittelbelehrung</strong>
verlangen. Bisher liegen nur Auskünfte vor, gegen die kein Rechtsmittel möglich ist.</li>
<li>Bei Ablehnung: Rekurs an die vorgesetzte Direktion, danach Beschwerde ans
Verwaltungsgericht des Kantons Zürich. Ein Gericht klärt dann verbindlich, ob der Kanton
den Spielraum hat, den er bestreitet.</li>
<li>Gesuchsteller: ein Verein (ASVZ oder Pump Tsüri), idealerweise gemeinsam mit dem
Sportamt.</li>
</ul></div>

<div class="step"><div class="who">Stufe 2 · kantonales Recht · Monate bis ein Jahr</div>
<h3>Die kantonale Sperre anpassen</h3>
<ul>
<li>Antrag an den Kanton, die Sperrverfügung der städtischen Badezonen zu ändern:
zeitlich auf die Badeöffnungszeiten beschränkt, oder mit Zusatztafel unter dem
Verbotszeichen. Anhang 4 BSV sieht Schilder mit ergänzenden Erklärungen ausdrücklich vor.</li>
<li>Politisch im Kantonsrat: Postulat oder Motion. Oder eine <strong>Einzelinitiative</strong>,
die im Kanton Zürich jede stimmberechtigte Person einreichen kann; sie braucht die
vorläufige Unterstützung von 60 Mitgliedern des Kantonsrats.</li>
<li>Ziel ist ein Satz in der kantonalen Schifffahrtsverordnung:</li>
</ul>
<div class="draft"><span class="lbl">Formulierungsvorschlag kantonale Verordnung</span>
«Die zuständige Direktion kann das Befahren gesperrter Wasserflächen bei Badeanlagen
ausserhalb der Betriebszeiten für bewilligte Schulungen mit muskelbetriebenen
Wassersportgeräten gestatten. Sie verbindet die Bewilligung mit Auflagen zur Sicherheit.»</div>
<div class="draft"><span class="lbl">Formulierungsvorschlag Zusatztafel</span>
«Ausgenommen bewilligte Schulung ausserhalb der Badezeiten»</div>
</div>

<div class="step"><div class="who">Stufe 3 · Bundesebene · mehrere Jahre</div>
<h3>BSV ergänzen, falls der Kanton blockiert</h3>
<ul>
<li>Die BSV ist eine Verordnung des Bundesrats, kein Gesetz. Es braucht keinen
Parlamentsbeschluss, sondern eine Verordnungsrevision, federführend das Bundesamt für
Verkehr (BAV).</li>
<li>Sauberste Änderung: Art. 163 Abs. 1 um einen Buchstaben ergänzen.</li>
<li>Wege: Motion im Nationalrat durch Zürcher Parlamentarier, Standesinitiative des
Kantons, gemeinsame Eingabe der Verbände bei der nächsten BSV-Vernehmlassung.</li>
</ul>
<div class="draft"><span class="lbl">Formulierungsvorschlag Art. 163 Abs. 1 BSV (neuer Buchstabe)</span>
«Artikel 37 Absatz 1; sie kann das Befahren von für die Schifffahrt gesperrten
Wasserflächen bei Badeanlagen ausserhalb der Badezeiten für bewilligte Schulungen mit
muskelbetriebenen Wassersportgeräten gestatten, wenn die Sicherheit gewährleistet ist.»</div>
</div>
</div>

<div class="sec">
<h2>3. Übersicht</h2>
<table>
<tr><th>Stufe</th><th>Was</th><th>Wer entscheidet</th><th>Wer stösst an</th><th>Dauer</th></tr>
<tr><td>1</td><td>Bewilligung nach Art. 72 BSV, anfechtbare Verfügung</td><td>AWEL, danach Rekursinstanz und Verwaltungsgericht</td><td>Verein mit Sportamt</td><td>Wochen bis Monate</td></tr>
<tr><td>2a</td><td>Sperrverfügung zeitlich begrenzen, Zusatztafel</td><td>Kanton (AWEL / Baudirektion)</td><td>Stadt als Konzessionärin, Vereine</td><td>Monate</td></tr>
<tr><td>2b</td><td>Kantonale Schifffahrtsverordnung ergänzen</td><td>Regierungsrat</td><td>Kantonsrat, Einzelinitiative</td><td>bis ein Jahr</td></tr>
<tr><td>3</td><td>Art. 163 BSV ergänzen</td><td>Bundesrat (BAV)</td><td>Nationalrat, Kanton, Verbände</td><td>mehrere Jahre</td></tr>
</table>
</div>

<div class="sec">
<h2>4. Empfehlung</h2>
<ol>
<li>Stufe 1 und Stufe 2 parallel starten. Das Postulat im Gemeinderat erzeugt den
politischen Druck, das Gesuch mit anfechtbarer Verfügung den rechtlichen.</li>
<li>Vor dem Gesuch ein kurzes Gutachten einer Fachperson für Verwaltungsrecht einholen,
insbesondere zur Abgrenzung von Art. 163 und Art. 72 Abs. 3 BSV und zum genauen
Instanzenzug im Kanton Zürich.</li>
<li>Stufe 3 über die nationalen Verbände vorbereiten, damit das Anliegen bei der nächsten
BSV-Revision auf dem Tisch liegt.</li>
</ol>
</div>

<div class="sec">
<h2>5. Quellen</h2>
<ul class="links">
<li>Binnenschifffahrtsverordnung (BSV, SR 747.201.1):<br><a href="{bsv}">{bsv}</a></li>
<li>Bundesgesetz über die Binnenschifffahrt (BSG, SR 747.201):<br><a href="{bsg}">{bsg}</a></li>
<li>Antwort des Stadtrats, Beschluss Nr. 2806/2026:<br><a href="{antwort}">{antwort}</a></li>
<li>Replik Pump Tsüri zu GR Nr. 2026/250:<br><a href="{replik}">{replik}</a></li>
</ul>
</div>

<div class="foot">Pump Tsüri · <a href="{pt}">pump.zuerich</a> · Die BSV-Zitate stammen aus
der konsolidierten Fassung und sind vor einer Eingabe gegen den aktuellen Stand auf
fedlex.admin.ch zu prüfen. Der Instanzenzug im Kanton Zürich ist nicht am kantonalen
Verfahrensrecht verifiziert. Dieses Arbeitspapier ersetzt keine Rechtsberatung.</div>

</body></html>
"#,
        pt = URL_PT,
        bsv = URL_BSV,
        bsg = URL_BSG,
        antwort = URL_ANTWORT,
        replik = URL_REPLIK,
    )
}

fn main() -> Result<()> {
    let args = Args::parse();
    let output = args.output.unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join("Downloads/Pumpfoil-gelbe-Zone-Rechtsweg.pdf")
    });
    let html_path = output.with_extension("html");
    std::fs::write(&html_path, render_html())?;

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
