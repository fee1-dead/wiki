use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;
use wiki::api::{QueryResponse, AbuseLog};

#[derive(Deserialize, Debug)]
pub struct AbuseLogEntry {
    pub id: u64,
}

pub type ListResponse = QueryResponse<AbuseLog<AbuseLogEntry>>;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// filter id
    filter_id: u64,
    
    /// Path to abuse filter file
    path: PathBuf,

    /// number of hours into the past to investigate. Accepts decimals, and defaults to one year.
    #[clap(long, default_value_t = 24.0 * 365.0)]
    hours: f64,
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseFloatError> {
    let hours: f64 = arg.parse()?;
    Ok(std::time::Duration::from_secs_f64(hours * 60.0 * 60.0))
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    let wiki = wiki::Site::enwiki();
    let q = wiki::req::Query {
        list: Some(
            QueryList::AbuseLog(ListAbuseLog {
                filter: Some(vec![filter]),
                start: None,
                logid: None,
                end: Some(time.into()),
                limit: Limit::Value(100),
                prop: AbuseLogProp::IDS | AbuseLogProp::DETAILS,
            })
            .into(),
        ),
        ..Default::default()
    };
    let stream = site.query_all(q)
        .try_filter_map(|x| Box::pin(async { Ok(Some(serde_json::from_value::<ListResponse>(x)?)) }));
    Ok(())
}
