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
    let handle = lifecycle_manager.handle();
    let _handle = lifecycle_manager
        .spawn("future 1", |_signal| async move {
            tracing::info!("future 1 is working");
            tokio::time::sleep(Duration::from_millis(500)).await;

            let _ = handle.spawn("future 3", |_signal| async move {
                tracing::info!("future 3 is working");
                tokio::time::sleep(Duration::from_millis(100)).await;

                tracing::info!("future 3 is completed");
                ExitStatus::Success
            });

            tracing::info!("future 1 is completed");
            ExitStatus::Success
        })
        .spawn("future 2", |signal| async {
            tracing::info!("future 2 is working");
            let sleep = tokio::time::sleep(Duration::from_secs(5));
            tokio::pin!(sleep);

            match future::select(signal, sleep).await {
                Either::Left(_) => tracing::info!("future 2 got shutdown signal"),
                Either::Right(_) => tracing::info!("future 2 is completed"),
            };

            ExitStatus::Success
        });

    tracing::info!("Press `Ctrl+C` to stop");
    lifecycle_manager.serve().await?.ok();

    tracing::info!("Completed");

    Ok(())
}
