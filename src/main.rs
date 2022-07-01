use std::time::{Duration, Instant};

use futures_util::StreamExt;
use wiki::gen::SearchGenerator;
use wiki::jobs::{create_server, JobRunner};
use wiki::{BotPassword, Site};

#[tokio::main]
async fn main() -> wiki::Result<()> {
    main_().await
}

async fn main_() -> wiki::Result<()> {
    let site = Site::testwiki();
    let (bot, runner) = site
        .login(
            BotPassword::new("0xDeadbeef@Testing", include_str!("../verysecret")),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    tokio::spawn(runner.run());
    let i = Instant::now();
    let gen = SearchGenerator::new(bot, r"test".into());
    let s = gen.into_stream();
    let c = s.count().await;
    dbg!(c);
    dbg!(i.elapsed());
    Ok(())
}
