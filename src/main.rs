use std::time::Instant;

use wiki::{BotPassword, Site};

#[tokio::main]
async fn main() -> wiki::Result<()> {
    main_().await
}

async fn main_() -> wiki::Result<()> {
    let site = Site::testwiki();
    let i = Instant::now();
    let bot = site
        .login(BotPassword::new(
            "0xDeadbeef@Testing",
            include_str!("../verysecret"),
        ))
        .await
        .unwrap();

    let page = bot.fetch("User talk:0xDeadbeef").await?;
    page.save(&format!("Hello, World!! {:?}", i.elapsed()), "botte")
        .await?;
    Ok(())
}
