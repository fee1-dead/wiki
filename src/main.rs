use wiki::{BotPassword, Site, req::{Main, TokenType}};

#[tokio::main]
async fn main() -> wiki::Result<()> {
    main_().await
}

async fn main_() -> wiki::Result<()> {
    let site = Site::testwiki();
    let bot = site
        .login(BotPassword::new(
            "0xDeadbeef@Testing",
            include_str!("../verysecret"),
        ))
        .await
        .unwrap();

    let page = bot.fetch("User talk:0xDeadbeef").await?;
    page.save("Hello, World!!", "botte").await?;
    Ok(())
}
