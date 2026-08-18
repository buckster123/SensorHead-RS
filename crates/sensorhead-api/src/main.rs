//! sensorhead-api — drop-in HTTP face for the SensorHead :8080 contract.

use clap::Parser;
use sensorhead_api::{router, AppState};

#[derive(Parser, Debug)]
#[command(name = "sensorhead-api", about = "SensorHead-RS HTTP face")]
struct Args {
    /// Listen address. Default stays on loopback so a laptop run cannot
    /// steal apex1's live :8080 by accident.
    #[arg(long, env = "SENSORHEAD_BIND", default_value = "127.0.0.1:8080")]
    bind: String,

    /// Python (or other) SensorHead that still owns BSEC2 / Picamera2.
    #[arg(long, env = "SENSORHEAD_UPSTREAM")]
    upstream: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sensorhead_api=info".parse()?),
        )
        .init();

    let args = Args::parse();
    let state = AppState::new(args.upstream.clone())?;
    tracing::info!(bind = %args.bind, upstream = ?args.upstream, "sensorhead-api starting");
    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}
