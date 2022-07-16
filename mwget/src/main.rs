use std::fs::File;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::fmt;
use std::io::Write;

use chrono::Duration as CDuration;
use futures_util::{TryStreamExt, TryFutureExt};
use regex::{Regex, RegexBuilder};
use regex_syntax::ast::parse::Parser;
use regex_syntax::ast::{Ast, Span};
use tokio::task::JoinHandle;
use tracing::{Dispatch, warn};
use tracing_subscriber::{Layer, EnvFilter};
use wiki::{BotPassword, Site};

mod abuse_log;
mod ccnorm;
pub mod equivset;

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
        Ok(Self { re, src, span, count: Default::default() })
    }
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T, E = Error> = std::result::Result<T, E>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let sub = tracing_subscriber::fmt().with_env_filter(EnvFilter::from_default_env()).finish();
    let layer = tracing_timing::Builder::default().layer(|| tracing_timing::Histogram::new(3).unwrap());
    // let downcaster = layer.downcaster();
    let layered = layer.with_subscriber(sub);
    let dispatch = Dispatch::new(layered);
    tracing::dispatcher::set_global_default(dispatch.clone())
        .expect("setting tracing default failed");
    

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
        let mut stream = abuse_log::search_within(&bot, "614".into(), CDuration::weeks(52));
        while let Some(res) = stream.try_next().await? {
            send.send(res.query.abuse_log.into_iter().map(|entry| (entry.details.added_lines.join("\n"), entry.id))).await?;
        }
        Result::<_>::Ok(())
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
    }).map_err(|e| e.into());

    tokio::try_join!(flatten(read), write)?;

    let mut cases = cases.to_vec();
    cases.sort_by_key(|case| case.count.load(Ordering::Relaxed));

    let mut file = File::create("result.txt")?;

    for case in cases {
        println!("{case:?}");
        writeln!(file, "{case:?}")?;
    }

    // abuse_log_grep::search(&bot, "614".into(), Regex::new(r"epst(?:ei|ie)n\W+did\s*n.?t\s+kill").unwrap()).await?;
    Ok(())
}

async fn flatten<T>(handle: JoinHandle<Result<T>>) -> Result<T> {
    match handle.await {
        Ok(Ok(result)) => Ok(result),
        Ok(Err(err)) => Err(err),
        Err(err) => Err(err.into()),
    }
}

impl fmt::Debug for Case<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Case").field("src", &self.src).field("count", &self.count.load(Ordering::Relaxed)).finish()
    }
}

impl Clone for Case<'_> {
    fn clone(&self) -> Self {
        Self { re: self.re.clone(), src: self.src, span: self.span, count: AtomicUsize::new(self.count.load(Ordering::Relaxed)) }
    }
}