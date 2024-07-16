mod scrape;
mod structs;

use config::{Config, Environment};
use std::error::Error;
use std::fs;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    println!("{}", module_path!());
    env_logger::init();

    let config = Config::builder()
        .add_source(Environment::with_prefix("DT"))
        .set_default("bulletin_home_url", "https://bulletin.uga.edu/coursesHome")?
        .set_default(
            "course_details_url",
            "https://sis-ssb-prod.uga.edu/PROD/bwckctlg.p_disp_course_detail",
        )?
        .set_default("per_min_ratelimit", 60)?
        .set_default("term", "202408")?
        .build()?;
    let ctx = scrape::Context::new(config)?;
    let html = fs::read_to_string("/home/ggliv/Downloads/classes.html")?;
    let doc = scraper::Html::parse_document(&html);

    println!(
        "{}",
        serde_json::to_string(&scrape::scrape_doc(&ctx, doc).await?)?
    );

    Ok(())
}
