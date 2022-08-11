use futures_util::{StreamExt, TryStreamExt};

#[tokio::main]
async fn main() -> wiki::Result<()> {
    let stream = wiki::events::ReqwestSseStream::revision_scores().await?;
    let events = stream.take(10).try_collect::<Vec<_>>().await?;
    dbg!(events);
    Ok(())
}
