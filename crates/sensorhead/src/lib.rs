//! SensorHead-RS core library.
//!
//! Types, parsers, and pure views over the `:8080` contract. Hardware I/O and
//! the two vendor walls (BSEC2, libcamera/Picamera2) stay out of this crate.
//!
//! See `docs/design.md` for the contract and `docs/CHARTER.md` for the binding
//! decisions. The Python original is the read-only checkout at `py-source/`.

pub mod environment;
pub mod error;
pub mod iaq;
pub mod ironbow;
pub mod thermal;

pub use environment::EnvironmentBody;
pub use error::{Error, Result};
pub use iaq::{iaq_accuracy_label, iaq_band};
pub use ironbow::{render_heatmap, RgbImage, HEATMAP_HEIGHT, HEATMAP_WIDTH};
pub use thermal::{
    clamp_celsius, frame_stats, ThermalBody, THERMAL_COLS, THERMAL_PIXELS, THERMAL_ROWS,
};
