use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use futures::FutureExt;
use sigfinn::{ExitStatus, LifecycleManager, Shutdown};
use tokio::net::TcpListener;
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

    lifecycle_manager.spawn("TCP server", |signal| async {
        tracing::info!("TCP server is working");
        match start_server(signal).await {
            Ok(_) => ExitStatus::Success,
            Err(error) => ExitStatus::Failure(error),
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

async fn start_server(signal: Shutdown) -> Result<(), std::io::Error> {
    tokio::pin!(signal);

    let listen_address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 3000));

    tracing::info!("Listen on {listen_address}");

    let server = TcpListener::bind(listen_address).await?;

    loop {
        let socket = tokio::select! {
            s = server.accept().fuse() => s,
            _ = signal.as_mut() => break,
        };

        match socket {
            Ok((stream, socket_addr)) => {
                tracing::info!("Accepted {socket_addr}");
                drop(stream);
            }
            Err(e) => tracing::error!("{e}"),
        }
    }

    Ok(())
}
