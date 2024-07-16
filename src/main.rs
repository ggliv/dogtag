mod scrape;
mod structs;

use config::{Config, Environment};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let _config = Config::builder()
        .add_source(Environment::with_prefix("DT"))
        .build()?;
    let html = std::fs::read_to_string("/home/ggliv/Downloads/csci_catalog.html")?;
    let doc = scraper::Html::parse_document(&html);
    println!(
        "{}",
        serde_json::to_string(&scrape::parse(doc, "202408".into()).await?)?
    );
    Ok(())
}
