use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Condition {
    New,
    Used,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Region {
    Ch,
    World,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub source: String,
    pub brand: Option<String>,
    pub title: String,
    pub url: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub condition: Condition,
    pub available: Option<bool>,
    pub location: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
    pub region: Region,
    pub fetched_at: DateTime<Utc>,
}

impl Listing {
    pub fn price_display(&self) -> String {
        match (self.price, &self.currency) {
            (Some(p), Some(c)) => format!("{:.0} {}", p, c),
            (Some(p), None) => format!("{:.0}", p),
            _ => "-".to_string(),
        }
    }
}
