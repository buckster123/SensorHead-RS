//! sensorhead-api — drop-in HTTP face for the SensorHead :8080 contract.

use std::path::PathBuf;

use clap::Parser;
use sensorhead_api::{
    doctor, doctor_cameras, router, AppState, BsecHelperConfig, CameraHelperConfig,
};

#[derive(Parser, Debug)]
#[command(name = "sensorhead-api", about = "SensorHead-RS HTTP face")]
struct Args {
    /// Listen address. Default stays on loopback so a laptop run cannot
    /// steal apex1's live :8080 by accident.
    #[arg(long, env = "SENSORHEAD_BIND", default_value = "127.0.0.1:8080")]
    bind: String,

    /// Python (or other) SensorHead that still owns default BSEC / cameras.
    #[arg(long, env = "SENSORHEAD_UPSTREAM")]
    upstream: Option<String>,

    /// `upstream` (default) proxies Python. `native` opens the MLX90640
    /// on I2C — exclusive; do not combine with a live Python thermal owner.
    #[arg(long, env = "SENSORHEAD_THERMAL", default_value = "upstream")]
    thermal: String,

    /// `upstream` (default) proxies Python `/api/environment`. `helper`
    /// spawns `walls/bsec.py` — exclusive on the BME688.
    #[arg(long, env = "SENSORHEAD_IAQ", default_value = "upstream")]
    iaq: String,

    /// `upstream` (default) proxies Python camera routes. `helper`
    /// spawns `walls/cameras.py` — exclusive on CSI.
    #[arg(long, env = "SENSORHEAD_CAMERAS", default_value = "upstream")]
    cameras: String,

    /// I2C bus number for `SENSORHEAD_THERMAL=native` (`/dev/i2c-N`).
    #[arg(long, env = "SENSORHEAD_I2C_BUS", default_value_t = 1)]
    i2c_bus: u8,

    /// MLX90640 7-bit address (default 0x33).
    #[arg(long, env = "SENSORHEAD_MLX_ADDR", default_value_t = 0x33)]
    mlx_addr: u8,

    /// BME688 7-bit address for `SENSORHEAD_IAQ=helper` (default 0x77).
    #[arg(long, env = "SENSORHEAD_BME688_ADDR", default_value_t = 0x77)]
    bme688_addr: u8,

    /// System Python for the helpers. Never a venv interpreter.
    #[arg(long, env = "SENSORHEAD_PYTHON", default_value = "/usr/bin/python3")]
    python: PathBuf,

    /// Path to `walls/bsec.py`.
    #[arg(long, env = "SENSORHEAD_BSEC_HELPER", default_value = "walls/bsec.py")]
    bsec_helper: PathBuf,

    /// Path to `walls/cameras.py`.
    #[arg(
        long,
        env = "SENSORHEAD_CAMERAS_HELPER",
        default_value = "walls/cameras.py"
    )]
    cameras_helper: PathBuf,

    /// IMX500 on-chip model directory.
    #[arg(
        long,
        env = "SENSORHEAD_MODEL_DIR",
        default_value = "/usr/share/imx500-models"
    )]
    model_dir: PathBuf,

    /// Operator-supplied pi3g `bme68x` egg. Not in git.
    #[arg(long, env = "SENSORHEAD_BME68X_EGG")]
    bme68x_egg: Option<PathBuf>,

    /// BSEC state directory (`bsec_state.json`).
    #[arg(long, env = "SENSORHEAD_DATA_DIR", default_value = "data")]
    data_dir: PathBuf,

    /// Check BSEC + Picamera2 helpers (imports only). Does not open I2C or CSI.
    #[arg(long)]
    doctor: bool,
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
    let iaq = sensorhead::IaqSource::parse(&args.iaq).map_err(|e| anyhow::anyhow!(e))?;
    let cameras = sensorhead::CameraSource::parse(&args.cameras).map_err(|e| anyhow::anyhow!(e))?;
    let bsec = BsecHelperConfig {
        python: args.python.clone(),
        script: args.bsec_helper,
        egg: args.bme68x_egg,
        data_dir: args.data_dir,
        addr: args.bme688_addr,
    };
    let cam = CameraHelperConfig {
        python: args.python,
        script: args.cameras_helper,
        model_dir: args.model_dir,
    };
    if args.doctor {
        let bsec_report = doctor(&bsec).await;
        let cam_report = doctor_cameras(&cam).await;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "bsec": bsec_report,
                "cameras": cam_report,
            }))?
        );
        if bsec_report.ok || cam_report.ok {
            return Ok(());
        }
        std::process::exit(1);
    }
    let state = AppState::with_all(args.upstream.clone(), thermal, iaq, bsec, cameras, cam)?;
    tracing::info!(
        bind = %args.bind,
        upstream = ?args.upstream,
        thermal = thermal.as_str(),
        iaq = iaq.as_str(),
        cameras = cameras.as_str(),
        "sensorhead-api starting"
    );
    let listener = tokio::net::TcpListener::bind(&args.bind).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}
