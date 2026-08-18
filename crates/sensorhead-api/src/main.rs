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

    /// `upstream` (default) proxies Python. `native` opens the MLX90640
    /// on I2C — exclusive; do not combine with a live Python thermal owner.
    #[arg(long, env = "SENSORHEAD_THERMAL", default_value = "upstream")]
    thermal: String,

    /// I2C bus number for `SENSORHEAD_THERMAL=native` (`/dev/i2c-N`).
    #[arg(long, env = "SENSORHEAD_I2C_BUS", default_value_t = 1)]
    i2c_bus: u8,

    /// MLX90640 7-bit address (default 0x33).
    #[arg(long, env = "SENSORHEAD_MLX_ADDR", default_value_t = 0x33)]
    mlx_addr: u8,
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
    let thermal = sensorhead::ThermalSource::parse(&args.thermal, args.i2c_bus, args.mlx_addr)
        .map_err(|e| anyhow::anyhow!(e))?;
    let state = AppState::with_thermal(args.upstream.clone(), thermal)?;
    tracing::info!(
        bind = %args.bind,
        upstream = ?args.upstream,
        thermal = thermal.as_str(),
        "sensorhead-api starting"
    );
    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}
