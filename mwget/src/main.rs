use tracing::metadata::LevelFilter;
use tracing::Dispatch;
use tracing_subscriber::{EnvFilter, Layer};

mod abuse_log;
pub mod ccnorm;
pub mod equivset;
// mod spitimeline;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let sub = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .finish();
    let layer =
        tracing_timing::Builder::default().layer(|| tracing_timing::Histogram::new(3).unwrap());
    let layered = layer.with_subscriber(sub);
    let dispatch = Dispatch::new(layered);
    tracing::dispatcher::set_global_default(dispatch.clone())
        .expect("setting tracing default failed");

    // spitimeline::main().await?;
    // spitimeline::sort()?;
    abuse_log::main().await?;
    Ok(())
}
