use chrono::{Duration, Utc};
use futures_util::{Stream, TryStreamExt};
use serde::Deserialize;
use wiki::api::{AbuseLog, QueryResponse};
use wiki::req::abuse_log::{AbuseLogProp, ListAbuseLog};
use wiki::req::{Limit, QueryList};
use wiki::Bot;

#[derive(Deserialize, Debug)]
pub struct AbuseLogEntry {
    pub id: u64,
    pub details: Details,
}

#[derive(Deserialize, Debug)]
pub struct Details {
    pub added_lines: Vec<String>,
}

pub type MyResponse = QueryResponse<AbuseLog<AbuseLogEntry>>;

pub fn search_within(
    bot: &Bot,
    filter: String,
    time: Duration,
) -> impl Stream<Item = wiki::Result<MyResponse>> + Unpin {
    let q = wiki::req::Query {
        list: Some(
            QueryList::AbuseLog(ListAbuseLog {
                filter: Some(vec![filter]),
                start: None,
                end: Some((Utc::now() - time).into()),
                limit: Limit::Value(100),
                prop: AbuseLogProp::IDS | AbuseLogProp::DETAILS,
            })
            .into(),
        ),
        ..Default::default()
    };
    bot.query_all(q)
        .try_filter_map(|x| Box::pin(async { Ok(Some(serde_json::from_value::<MyResponse>(x)?)) }))
}

pub async fn search(bot: &Bot, filter: String, re: regex::Regex) -> wiki::Result<()> {
    let q = wiki::req::Query {
        list: Some(
            QueryList::AbuseLog(ListAbuseLog {
                filter: Some(vec![filter]),
                start: None,
                end: Some((Utc::now() - Duration::weeks(48)).into()),
                limit: Limit::Value(100),
                prop: AbuseLogProp::IDS | AbuseLogProp::DETAILS,
            })
            .into(),
        ),
        ..Default::default()
    };

    bot.query_all(q)
        .try_for_each_concurrent(None, |x| async {
            let res: MyResponse = serde_json::from_value(x)?;
            for entry in res.query.abuse_log {
                if entry
                    .details
                    .added_lines
                    .into_iter()
                    .map(|mut x| {
                        x.make_ascii_lowercase();
                        x
                    })
                    .any(|line| re.is_match(&line))
                {
                    println!("found match: {}", entry.id);
                }
            }
            Ok(())
        })
        .await?;
    Ok(())
}
