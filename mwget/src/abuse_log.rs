use std::fmt;
use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use chrono::{Duration as CDuration, Utc};
use futures_util::{Stream, TryFutureExt, TryStreamExt};
use regex::{Regex, RegexBuilder};
use regex_syntax::ast::parse::Parser;
use regex_syntax::ast::{Ast, Span};
use serde::Deserialize;
use tokio::task::JoinHandle;
use tracing::warn;
use wiki::api::{AbuseLog, QueryResponse};
use wiki::req::abuse_log::{AbuseLogProp, ListAbuseLog};
use wiki::req::{Limit, QueryList};
use wiki::{Bot, BotPassword, Site, AccessExt};

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

#[derive(Default)]
pub enum CaseSensitivity {
    Insensitive,
    #[default]
    Sensitive,
}

pub struct Case<'a> {
    pub re: Regex,
    pub src: &'a str,
    pub span: Span,
    pub count: AtomicUsize,
}

impl<'a> Case<'a> {
    pub fn new(s: &'a str, span: Span, sensitivity: CaseSensitivity) -> Result<Self, regex::Error> {
        let start = span.start.offset;
        let end = span.end.offset;
        let src = &s[start..end];
        let mut builder = RegexBuilder::new(src);
        builder.case_insensitive(matches!(sensitivity, CaseSensitivity::Insensitive));
        let re = builder.build()?;
        Ok(Self {
            re,
            src,
            span,
            count: Default::default(),
        })
    }
}

pub fn search_within(
    bot: &Bot,
    filter: String,
    time: CDuration,
) -> impl Stream<Item = wiki::Result<MyResponse>> + Unpin + Send {
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
                end: Some((Utc::now() - CDuration::weeks(48)).into()),
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

pub async fn main() -> crate::Result<()> {
    let site = Site::enwiki();
    let (bot, runner) = site
        .login(
            BotPassword::new("ScannerBot@RustWiki", include_str!("../../veryverysecret")), // BotPassword::new("0xDeadbeef@Testing", include_str!("../verysecret")),
            Duration::from_secs(5),
        )
        .await
        .map_err(|(_, e)| e)?;
    tokio::spawn(runner.run());
    let s = include_str!("test.re");
    let mut parser = Parser::new();
    let ast = parser.parse(s)?;
    let cases: Vec<_> = if let Ast::Alternation(alt) = &ast {
        alt.asts
            .iter()
            .map(|ast| Case::new(s, *ast.span(), CaseSensitivity::Insensitive).unwrap())
            .collect()
    } else {
        panic!("no")
    };

    let cases = &*cases.leak();

    let (send, mut receive) = tokio::sync::mpsc::channel(10);

    let read = tokio::spawn(async move {
        let mut stream = search_within(&bot, "614".into(), CDuration::weeks(52));
        while let Some(res) = stream.try_next().await? {
            send.send(
                res.query
                    .abuse_log
                    .into_iter()
                    .map(|entry| (entry.details.added_lines.join("\n"), entry.id)),
            )
            .await?;
        }
        crate::Result::<_>::Ok(())
    });

    let write = tokio::spawn(async move {
        while let Some(log) = receive.recv().await {
            for (entry, id) in log {
                // let entry = ccnorm::ccnorm(&entry);
                let mut has_match = false;
                for case in cases {
                    if case.re.is_match(&entry) {
                        case.count.fetch_add(1, Ordering::Relaxed);
                        has_match = true;
                    }
                }
                if !has_match {
                    warn!("No regex matched {id}");
                }
            }
        }
    })
    .map_err(|e| e.into());

    tokio::try_join!(flatten(read), write)?;

    let mut cases = cases.to_vec();
    cases.sort_by_key(|case| case.count.load(Ordering::Relaxed));

    let mut file = File::create("result.txt")?;

    for case in cases {
        println!("{case:?}");
        writeln!(file, "{case:?}")?;
    }
    Ok(())
}

async fn flatten<T>(handle: JoinHandle<crate::Result<T>>) -> crate::Result<T> {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(err.into()),
    }
}

impl fmt::Debug for Case<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Case")
            .field("src", &self.src)
            .field("count", &self.count.load(Ordering::Relaxed))
            .finish()
    }
}

impl Clone for Case<'_> {
    fn clone(&self) -> Self {
        Self {
            re: self.re.clone(),
            src: self.src,
            span: self.span,
            count: AtomicUsize::new(self.count.load(Ordering::Relaxed)),
        }
    }
}
