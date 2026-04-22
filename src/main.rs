use anyhow::Result;
use clap::Parser;
use crawl2pump::{run, Cli};

#[tokio::main]
async fn main() -> Result<()> {
    run(Cli::parse()).await
}
