use tracing::Dispatch;
use tracing_subscriber::{EnvFilter, Layer};

mod abuse_log;
mod ccnorm;
pub mod equivset;
mod spitimeline;

type Error = Box<dyn std::error::Error + Send + Sync>;
type Result<T, E = Error> = std::result::Result<T, E>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let sub = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .finish();
    let layer =
        tracing_timing::Builder::default().layer(|| tracing_timing::Histogram::new(3).unwrap());
    // let downcaster = layer.downcaster();
    let layered = layer.with_subscriber(sub);
    let dispatch = Dispatch::new(layered);
    tracing::dispatcher::set_global_default(dispatch.clone())
        .expect("setting tracing default failed");

    // spitimeline::main().await?;
    // spitimeline::sort()?;
    abuse_log::main().await?;

    // abuse_log_grep::search(&bot, "614".into(), Regex::new(r"epst(?:ei|ie)n\W+did\s*n.?t\s+kill").unwrap()).await?;
    Ok(())
}
