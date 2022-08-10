use std::time::Duration;

use futures_util::{StreamExt, TryStreamExt};
use wiki::generators::rcpatrol::RecentChangesPatroller;
use wiki::req::rc::{RcProp, RcType};
use wiki::{BotPassword, Site};

#[tokio::main]
async fn main() -> wiki::Result<()> {
    test_streams().await
}

async fn test_streams() -> wiki::Result<()> {
    let stream = wiki::events::ReqwestSseStream::revision_scores().await?;
    let events = stream.take(10).try_collect::<Vec<_>>().await?;
    dbg!(events);
    Ok(())
}

/* 
async fn main_() -> wiki::Result<()> {
    let site = Site::enwiki();
    let bot = site
        .login(
            BotPassword::new("ScannerBot@RustWiki", include_str!("../veryverysecret")), // BotPassword::new("0xDeadbeef@Testing", include_str!("../verysecret")),
            Duration::from_secs(5),
        )
        .await
        .map_err(|(_, e)| e)?;
    let rcp = RecentChangesPatroller::new(
        bot,
        Duration::from_secs(2),
        RcProp::ORES_SCORES | RcProp::TAGS | RcProp::TITLE | RcProp::TIMESTAMP,
        RcType::EDIT,
    );
    tokio::spawn(async move {
        rcp.try_for_each_concurrent(None, |x| async move {
            println!("{:?}", x.oresscores);
            Ok(())
        })
        .await
    });
    tokio::time::sleep(Duration::from_secs(100)).await;
    Ok(())
}*/
