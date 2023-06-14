use std::time::Duration;

use futures::{future, future::Either};
use sigfinn::{ExitStatus, LifecycleManager};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    {
        // filter
        let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));

        // format
        let fmt_layer = tracing_subscriber::fmt::layer();

        // subscriber
        tracing_subscriber::registry().with(filter_layer).with(fmt_layer).init();
    }

    let lifecycle_manager = LifecycleManager::<()>::new();

    lifecycle_manager.spawn("future 1", |signal| async {
        tracing::info!("future 1 is working");

        let sleep = tokio::time::sleep(Duration::from_secs(15));
        tokio::pin!(sleep);

        match future::select(signal, sleep).await {
            Either::Left(_) => tracing::info!("future 1 got shutdown signal"),
            Either::Right(_) => tracing::info!("future 1 is completed"),
        };

        ExitStatus::Success
    });

    tracing::info!("Press `Ctrl+C` to stop");
    tracing::info!("Use `$ kill -s TERM {}` to stop", std::process::id());

    lifecycle_manager.serve().await?.ok();

    Ok(())
}
