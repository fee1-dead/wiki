use std::collections::HashMap;
use std::fs::File;

use chrono::{DateTime, Duration, Utc};
use color_eyre::eyre::ContextCompat;
use fancy_regex::{Regex, RegexBuilder};
use futures_util::{Stream, TryFutureExt, TryStreamExt};
use schemars::JsonSchema;
use serde::de::Error;
use serde::{Deserialize, Deserializer, Serialize};
use tokio::task::JoinHandle;
use tracing::info;
use wiki::api::{AbuseFilters, AbuseLog, Pattern, QueryResponse, RequestBuilderExt};
use wiki::builder::ClientBuilder;
use wiki::req::abuse_log::{AbuseFilterProp, AbuseLogProp, ListAbuseFilters, ListAbuseLog};
use wiki::req::{Action, Limit, Query, QueryList};
use wiki::Bot;

#[derive(Deserialize, Debug)]
pub struct AbuseLogEntry {
    pub id: u64,
    pub details: Details,
}

#[derive(Deserialize, Debug)]
pub struct AbuseLogTime {
    #[serde(with = "wiki::util::dt")]
    pub timestamp: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct Details {
    pub added_lines: Vec<String>,
}

pub type MyResponse = QueryResponse<AbuseLog<AbuseLogEntry>>;
pub type AbuseLogTimeResponse = QueryResponse<AbuseLog<AbuseLogTime>>;

/// an individual filter.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct Filter {
    /// id of the filter.
    pub id: u32,
    /// individual regex cases of the filter.
    pub cases: Vec<String>,
}
/// a bot run.
#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct Run {
    /// when the report was generated.
    #[serde(with = "wiki::util::dt")]
    #[schemars(with = "DateTime<Utc>")]
    pub date: DateTime<Utc>,
    /// overview of the filters analyzed in this run.
    pub filters: Vec<Filter>,
    /// the log entries that this run scanned.
    ///
    /// Most recent entries first.
    pub entries: Vec<LogEntry>,
}

/// a log entry hit.
#[derive(Deserialize, Serialize, Clone, Copy, Debug, JsonSchema)]
pub struct Match {
    /// a filter rule that this log entry triggered
    pub filter_index: usize,
    /// the specific case that matched this diff
    pub case_index: usize,
    pub is_ccnorm: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct JsonOutput {
    /// Report runs.
    pub runs: Vec<Run>,
}

#[derive(Deserialize, Serialize, Clone, Debug, JsonSchema)]
pub struct LogEntry {
    /// what was the id of this log entry?
    pub id: u64,
    /// what filters did this diff trigger?
    pub matches: Vec<Match>,
}

pub fn extract_cases(input: &str) -> Vec<&str> {
    let mut chars = input.chars().peekable();
    let mut lastpos = 0;
    let mut pos = 0;
    let mut depth = 0;
    let mut buffer = Vec::new();
    while let Some(c) = chars.next() {
        match c {
            // backslash, ignore what comes next.
            // Although escape could contain more than one characters, we don't care.
            '\\' => {
                pos += 1;
                chars.next().unwrap();
            }
            '(' => {
                depth += 1;
            }
            ')' => {
                depth -= 1;
            }
            '|' if depth == 0 => {
                buffer.push(&input[lastpos..pos]);
                lastpos = pos + 1;
            }
            _ => {}
        }
        pos += c.len_utf8();
    }
    buffer
}

pub fn search_back_to(
    bot: &Bot,
    filter: String,
    time: DateTime<Utc>,
) -> impl Stream<Item = wiki::Result<MyResponse>> + Unpin + Send {
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
    bot.query_all(q)
        .try_filter_map(|x| Box::pin(async { Ok(Some(serde_json::from_value::<MyResponse>(x)?)) }))
}

pub fn de_regex<'de, D: Deserializer<'de>>(x: D) -> Result<Regex, D::Error> {
    let s = String::deserialize(x)?;
    Regex::new(&s).map_err(|e| D::Error::custom(e))
}

#[derive(Clone, Debug, Deserialize)]
pub struct FilterDetails {
    pub id: u32,
    pub ccnorm: bool,
    pub case_insensitive: bool,
    /// regex that grabs the actual regex out of filter pattern
    #[serde(deserialize_with = "de_regex")]
    pub grab_pattern: Regex,
}

pub async fn get_time(bot: &Bot, id: u64) -> color_eyre::Result<DateTime<Utc>> {
    let q = wiki::req::Query {
        list: Some(
            QueryList::AbuseLog(ListAbuseLog {
                logid: Some(id),
                filter: None,
                start: None,
                end: None,
                limit: Limit::Value(1),
                prop: AbuseLogProp::TIMESTAMP,
            })
            .into(),
        ),
        ..Default::default()
    };

    let mut at: AbuseLogTimeResponse = bot.get(Action::Query(q)).send_parse().await?;
    Ok(at
        .query
        .abuse_log
        .pop()
        .context("could not find abuse log entry")?
        .timestamp)
}

pub struct ParsedFilters {
    filters: Vec<Filter>,
    cases_to_check: Vec<(Match, Regex)>,
}

async fn parse_filters(bot: &Bot, cfg: Vec<FilterDetails>) -> color_eyre::Result<ParsedFilters> {
    let mut filters = vec![];
    let mut cases_to_check = vec![];

    for (filter_index, filter) in cfg.into_iter().enumerate() {
        let bot = bot.clone();
        // first try to retrieve the source code of the regex
        let action = Action::Query(Query {
            list: Some(
                QueryList::AbuseFilters(ListAbuseFilters {
                    startid: Some(filter.id),
                    prop: AbuseFilterProp::PATTERN,
                    limit: Limit::Value(1),
                    ..Default::default()
                })
                .into(),
            ),
            ..Default::default()
        });

        let a: QueryResponse<AbuseFilters<Pattern>> = bot.get(action).send_parse().await?;
        let abuse_filter = a
            .query
            .abuse_filters
            .into_iter()
            .next()
            .ok_or_else(|| color_eyre::eyre::anyhow!("did not fetch filter"))?;
        info!("got filter raw: {}", abuse_filter.pattern);
        let matches = filter
            .grab_pattern
            .captures(&abuse_filter.pattern)?
            .expect("expected match");
        let regex = matches.get(1).unwrap().as_str();
        info!("got regex: {regex}");

        // now, we need to compile it
        let all_cases = extract_cases(regex);
        info!(?all_cases);
        let mut cases = vec![];

        for (case_index, case) in all_cases.iter().copied().enumerate() {
            let case = if filter.case_insensitive {
                format!("(?i:{case})")
            } else {
                case.to_owned()
            };
            cases_to_check.push((
                Match {
                    filter_index,
                    case_index,
                    is_ccnorm: filter.ccnorm,
                },
                // N.B: (?<!\\d|#)(?:69\\D*420|420\\D*69|(?:69\\D{0,50}){3,})(?!\\d)
                // has a LOT of back off. It exceeded the default limit of one million.
                RegexBuilder::new(&case)
                    .backtrack_limit(10_000_000)
                    .build()?,
            ));
            cases.push(case);
        }
        filters.push(Filter {
            id: filter.id,
            cases,
        });
    }

    Ok(ParsedFilters {
        filters,
        cases_to_check,
    })
}

pub async fn catch_up() -> color_eyre::Result<JsonOutput> {
    let bot = ClientBuilder::enwiki()
        .oauth(include_str!("../../bot_oauth.txt.secret"))
        .build()
        .await?;

    // update the schema
    let schema = schemars::schema_for!(JsonOutput);
    let schema = serde_json::to_string_pretty(&schema)?;

    bot.build_edit("User:DeadbeefBot/AbuseAnalyzer_Schema.json")
        .text(schema)
        .bot()
        .summary("updating schema")
        .send()
        .await?;

    // parse the config
    let config = bot
        .fetch_content("User:0xDeadbeef/AbuseAnalyzerConfig")
        .await?;
    let re = Regex::new("<syntaxhighlight lang=\"json\">((?s:.)*)</syntaxhighlight>")?;
    let config = re.captures(&config)?.unwrap().get(1).unwrap().as_str();
    let cfg: Vec<FilterDetails> = serde_json::from_str(config)?;
    info!("got config: {cfg:#?}");

    let ParsedFilters {
        filters,
        cases_to_check,
    } = parse_filters(&bot, cfg).await?;

    // read previous output
    let json: JsonOutput = serde_json::from_reader(File::open("result.json")?)?;
    let last_entry = json.runs.last().and_then(|run| run.entries.first());

    let time_to_start_from = if let Some(entry) = last_entry {
        get_time(&bot, entry.id).await?
    } else {
        Utc::now() - Duration::weeks(52)
    };

    let (send, mut receive) = tokio::sync::mpsc::channel(10);

    let new_filters = filters.clone();
    let read = tokio::spawn(async move {
        for filter in new_filters {
            let mut stream = search_back_to(&bot, filter.id.to_string(), time_to_start_from);
            while let Some(res) = stream.try_next().await? {
                send.send(
                    res.query
                        .abuse_log
                        .into_iter()
                        .map(|entry| (entry.details.added_lines.join("\n"), entry.id)),
                )
                .await?;
            }
        }

        color_eyre::Result::<_>::Ok(())
    });

    let (entry_sink, mut entry_out) = tokio::sync::mpsc::channel(10);

    let write = tokio::spawn(async move {
        let cases = cases_to_check;
        while let Some(log) = receive.recv().await {
            for (entry, id) in log {
                let ccnormed = crate::ccnorm::ccnorm(&entry);
                let mut matches = vec![];
                for (m, re) in cases.clone() {
                    let entry = if m.is_ccnorm { &ccnormed } else { &entry };
                    // info!("{}", re.as_str());
                    if re.is_match(entry).unwrap() {
                        matches.push(m)
                    }
                }
                entry_sink.send(LogEntry { id, matches }).await?;
            }
        }
        color_eyre::Result::<_>::Ok(())
    });

    let entry_out = tokio::spawn(async move {
        let mut v = vec![];
        while let Some(log) = entry_out.recv().await {
            v.push(log);
        }
        v
    })
    .map_err(|x| x.into());

    let (_, _, entries) = tokio::try_join!(flatten(read), flatten(write), entry_out)?;

    let run = Run {
        date: Utc::now(),
        filters,
        entries,
    };

    let file = File::create("result.json")?;

    let mut json = json;

    json.runs.push(run);

    serde_json::to_writer_pretty(file, &json)?;

    Ok(json)
}

pub struct CaseReport {
    pub regex: String,
    pub last_hit: u64,
    pub last_hit_date: String,
}

pub struct Analyzed {
    pub filters: HashMap<u32, HashMap<String, CaseReport>>,
}

pub async fn main() -> color_eyre::Result<()> {
    let json = catch_up().await?;

    Ok(())
}

async fn flatten<T>(handle: JoinHandle<color_eyre::Result<T>>) -> color_eyre::Result<T> {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(err.into()),
    }
}
