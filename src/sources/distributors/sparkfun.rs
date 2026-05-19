//! SparkFun scrape source. Parts carrying a `sparkfun_pid` get
//! `https://www.sparkfun.com/products/<pid>` fetched — SparkFun ships
//! JSON-LD `Product` with an `Offer` price + image, so
//! `fetch_page_product` recovers price/currency/image cleanly.

use super::{offer, SensorOffer};
use crate::sensors::Part;
use crate::sources::html_util::fetch_page_product;
use anyhow::Result;
use reqwest::Client;

pub async fn fetch(client: &Client, parts: &[Part]) -> Result<Vec<SensorOffer>> {
    let mut out = Vec::new();
    for p in parts.iter().filter(|p| p.sparkfun_pid.is_some()) {
        let pid = p.sparkfun_pid.unwrap();
        let url = format!("https://www.sparkfun.com/products/{pid}");
        match fetch_page_product(client, &url).await {
            Ok(pp) => {
                let title = format!("{} · SparkFun {pid} @ sparkfun.com", p.name);
                out.push(offer(
                    p,
                    "sparkfun",
                    title,
                    url.clone(),
                    pp.price,
                    pp.currency.or_else(|| Some("USD".into())),
                    pp.available,
                    pp.image,
                    None,
                ));
            }
            Err(e) => eprintln!("    sparkfun: {} ({e})", p.key),
        }
    }
    Ok(out)
}
