use crate::listing::{Listing, Region};
use anyhow::Result;
use async_trait::async_trait;

pub mod brands;
pub mod browser;
pub mod classifieds;
pub mod flaresolverr;
pub mod html_util;
pub mod shopify;

#[async_trait]
pub trait Source: Send + Sync {
    fn name(&self) -> &'static str;
    fn region(&self) -> Region;
    async fn search(&self, query: &str) -> Result<Vec<Listing>>;
}
