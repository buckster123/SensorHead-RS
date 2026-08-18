//! `/api/environment` body. ApexOS-RS `apex-sensor-bridge` reads
//! `temperature_c`, `humidity_pct`, `pressure_hpa`, `iaq`,
//! `co2_equivalent_ppm`, `breath_voc_ppm`, `iaq_accuracy`. If `error` is
//! true the bridge emits nothing.

use serde::{Deserialize, Serialize};

/// One `/api/environment` response. Success, raw-mode, and error shapes
/// share a single struct so a missing `iaq` stays `None` (charter D5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentBody {
    #[serde(default)]
    pub error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_c: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub humidity_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure_hpa: Option<f64>,

    /// Present only when BSEC produced a value. JSON `null` (raw mode) → `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iaq: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iaq_accuracy: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iaq_accuracy_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air_quality: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub air_quality_description: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub co2_equivalent_ppm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub breath_voc_ppm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bsec_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale: Option<bool>,

    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl EnvironmentBody {
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }

    /// Honest degrade payload when this process has no nose (no upstream, no
    /// native BME). Same shape the Python original uses for a missing sensor.
    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            error: true,
            sensor: Some("BME688".into()),
            status: Some("unavailable".into()),
            reason: Some(reason.into()),
            temperature_c: None,
            humidity_pct: None,
            pressure_hpa: None,
            iaq: None,
            iaq_accuracy: None,
            iaq_accuracy_label: None,
            air_quality: None,
            air_quality_description: None,
            co2_equivalent_ppm: None,
            breath_voc_ppm: None,
            bsec_version: None,
            stale: None,
            extra: serde_json::Map::new(),
        }
    }

    /// ApexOS-RS emits an AirQuality event only when `iaq` is a number and
    /// `error` is not set. Accuracy gating lives in the consumer.
    pub fn apexos_air_quality_possible(&self) -> bool {
        !self.error && self.iaq.is_some()
    }
}
