use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::Utc;
use clap::Parser;
use futures_util::TryStreamExt;
use serde::Deserialize;
use tracing_subscriber::EnvFilter;
use wiki::api::{AbuseFilterCheckMatchResponse, AbuseLog, QueryResponse, RequestBuilderExt};
use wiki::builder::ClientBuilder;
use wiki::req::abuse_filter::{CheckMatch, CheckMatchTest};
use wiki::req::abuse_log::{AbuseLogProp, ListAbuseLog};
use wiki::req::{Action, Limit, QueryList};

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

fn from_hours_f64(x: f64) -> chrono::Duration {
    chrono::Duration::seconds((x * 60.0 * 60.0).round() as i64)
}

async fn main_inner() -> color_eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
    let Args {
        path,
        filter_id,
        hours,
    } = Args::parse();
    let site = ClientBuilder::enwiki()
        .oauth(include_str!("../../account_oauth.txt.secret"))
        .user_agent("DeadbeefBot")
        .build()
        .await?;
    let filter_text = fs::read_to_string(path)?;
    let duration = from_hours_f64(hours);
    let q = wiki::req::Query {
        list: Some(
            QueryList::AbuseLog(ListAbuseLog {
                filter: Some(vec![filter_id.to_string()]),
                start: None,
                logid: None,
                end: Some((Utc::now() - duration).into()),
                limit: Limit::Value(100),
                prop: AbuseLogProp::IDS,
            })
            .into(),
        ),
        ..Default::default()
    };
    let stream = site.query_all(q).try_filter_map(|x| {
        Box::pin(async { Ok(Some(serde_json::from_value::<ListResponse>(x)?)) })
    });

    let total = AtomicUsize::new(0);
    let matched = AtomicUsize::new(0);

    stream
        .try_for_each(|x| async {
            for entry in x.query.abuse_log {
                let id = entry.id;
                let action = Action::AbuseFilterCheckMatch(CheckMatch {
                    filter: filter_text.clone(),
                    test: CheckMatchTest::LogId(id),
                });
                total.fetch_add(1, Ordering::Relaxed);
                let x: AbuseFilterCheckMatchResponse = site.get(action).send_parse().await?;
                if x.inner.result {
                    matched.fetch_add(1, Ordering::Relaxed);
                }
            }
            Ok(())
        })
        .await?;

    let total = total.load(Ordering::Relaxed);
    let matched = matched.load(Ordering::Relaxed);

    println!(
        "Over past {:?}, filter {filter_id} has matched {total} edits in total, with \
        {matched} edits matching the filter supplied. ({}%)",
        duration.to_std()?,
        matched as f64 / total as f64 * 100.0,
    );

    Ok(())
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    main_inner().await
}
