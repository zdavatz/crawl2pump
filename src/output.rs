use crate::listing::Listing;
use crate::Format;
use anyhow::Result;
use std::io::Write;

pub fn write(listings: &[Listing], format: Format, path: Option<&str>) -> Result<()> {
    let mut out: Box<dyn Write> = match path {
        Some(p) => Box::new(std::fs::File::create(p)?),
        None => Box::new(std::io::stdout().lock()),
    };
    match format {
        Format::Json => write_json(listings, &mut out),
        Format::Csv => write_csv(listings, &mut out),
        Format::Table => write_table(listings, &mut out),
    }
}

fn write_json(listings: &[Listing], w: &mut dyn Write) -> Result<()> {
    serde_json::to_writer_pretty(&mut *w, listings)?;
    writeln!(w)?;
    Ok(())
}

fn write_csv(listings: &[Listing], w: &mut dyn Write) -> Result<()> {
    let mut wtr = csv::Writer::from_writer(w);
    for l in listings {
        wtr.serialize(l)?;
    }
    wtr.flush()?;
    Ok(())
}

fn write_table(listings: &[Listing], w: &mut dyn Write) -> Result<()> {
    writeln!(
        w,
        "{:<12} {:<10} {:<6} {:<14} {:<55} {}",
        "SOURCE", "BRAND", "COND", "PRICE", "TITLE", "URL"
    )?;
    writeln!(w, "{}", "-".repeat(140))?;
    for l in listings {
        let source = truncate(&l.source, 12);
        let brand = truncate(l.brand.as_deref().unwrap_or("-"), 10);
        let cond = match l.condition {
            crate::listing::Condition::New => "new",
            crate::listing::Condition::Used => "used",
            crate::listing::Condition::Unknown => "?",
        };
        let price = l.price_display();
        let title = truncate(&l.title, 55);
        writeln!(
            w,
            "{:<12} {:<10} {:<6} {:<14} {:<55} {}",
            source, brand, cond, price, title, l.url
        )?;
    }
    writeln!(w)?;
    writeln!(w, "total: {} listings", listings.len())?;
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    if s.chars().count() > max {
        out.pop();
        out.pop();
        out.push_str("..");
    }
    out
}
