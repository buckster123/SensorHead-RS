//! SensorHead-RS core library.
//!
//! Types, parsers, and pure views over the `:8080` contract. The two vendor
//! walls (BSEC2, libcamera/Picamera2) are stdio helpers outside this crate.
//! MLX90640 I2C is native when `ThermalSource::Native` is selected.
//!
//! See `docs/design.md` for the contract and `docs/CHARTER.md` for the binding
//! decisions. The Python original is the read-only checkout at `py-source/`.

pub mod bsec;
pub mod environment;
pub mod error;
pub mod iaq;
pub mod ironbow;
pub mod mlx;
pub mod status;
pub mod thermal;

pub use bsec::IaqSource;
pub use environment::EnvironmentBody;
pub use error::{Error, Result};
pub use iaq::{iaq_accuracy_label, iaq_band};
pub use ironbow::{render_heatmap, RgbImage, HEATMAP_HEIGHT, HEATMAP_WIDTH};
pub use mlx::{NativeMlx, ThermalSource};
pub use status::{compose_status, UpstreamView};
pub use thermal::{
    clamp_celsius, frame_stats, ThermalBody, THERMAL_COLS, THERMAL_PIXELS, THERMAL_ROWS,
};
