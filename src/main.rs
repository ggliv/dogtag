mod scrape;
mod structs;

use config::{Config, Environment};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let config = Config::builder()
        .add_source(Environment::with_prefix("DT"))
        .set_default("bulletin_home_url", "https://bulletin.uga.edu/coursesHome")?
        .set_default(
            "course_details_url",
            "https://sis-ssb-prod.uga.edu/PROD/bwckctlg.p_disp_course_detail",
        )?
        .set_default(
            "course_search_url",
            "https://sis-ssb-prod.uga.edu/PROD/bwckgens.p_proc_term_date",
        )?
        .set_default(
            "course_sched_url",
            "https://sis-ssb-prod.uga.edu/PROD/bwckschd.p_get_crse_unsec",
        )?
        .set_default("per_min_ratelimit", 60)?
        .set_default("term", "202408")?
        .build()?;

    let previous_scrape = config.get_string("previous_scrape").ok().and_then(|v| {
        Some(
            serde_json::from_str::<std::collections::HashMap<String, structs::Subject>>(
                &std::fs::read_to_string(v).ok()?,
            )
            .ok()?,
        )
    });

    let res = scrape::go(config, previous_scrape).await?;

    println!("{}", serde_json::to_string(&res)?);

    Ok(())
}
