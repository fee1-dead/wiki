use std::time::Duration;

use wiki::{BotPassword, Site, jobs::{JobRunner, create_server}};

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
    let mut page = bot.fetch("User talk:0xDeadbeef".into()).await?;
    for i in 0..10 {
        page.content_mut().push_str("\nTestingtestingtesting");
        page.save(&format!("Testing API ({i})")).await?;
        eprintln!("Edited");
        page.refetch().await?;
    }

    Ok(())
}
