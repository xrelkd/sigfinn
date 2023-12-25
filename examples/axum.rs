use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use axum::{routing::get, Router};
use sigfinn::{ExitStatus, LifecycleManager};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    {
        // filter
        let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug"));

        // format
        let fmt_layer = tracing_subscriber::fmt::layer();

        // subscriber
        tracing_subscriber::registry().with(filter_layer).with(fmt_layer).init();
    }

    let lifecycle_manager = LifecycleManager::new();

    lifecycle_manager.spawn("Axum server", |signal| async {
        tracing::info!("Axum server is working");

        let listen_address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3200));

        match axum::Server::try_bind(&listen_address) {
            Ok(server) => {
                let app = Router::new().route("/", get(|| async { axum::Json("Hello, World!") }));
                tracing::info!("Host is available on http://{listen_address}/");

                match server.serve(app.into_make_service()).with_graceful_shutdown(signal).await {
                    Ok(()) => ExitStatus::Success,
                    Err(err) => ExitStatus::FatalError(err),
                }
            }
            Err(err) => ExitStatus::FatalError(err),
        }
    });

    tracing::info!("Press `Ctrl+C` to stop");
    tracing::info!("Use `$ kill -s TERM {}` to stop", std::process::id());

    if let Err(err) = lifecycle_manager.serve().await? {
        tracing::error!("{err}");
    }

    tracing::info!("Completed");

    Ok(())
}
