//! `/api/thermal/data` body — 32×24 MLX90640 grid, row-major °C.

use serde::{Deserialize, Serialize};

pub const THERMAL_COLS: usize = 32;
pub const THERMAL_ROWS: usize = 24;
pub const THERMAL_PIXELS: usize = THERMAL_COLS * THERMAL_ROWS;
pub const THERMAL_MIN_C: f32 = -40.0;
pub const THERMAL_MAX_C: f32 = 300.0;

/// One `/api/thermal/data` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThermalBody {
    #[serde(default)]
    pub error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sensor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cols: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_c: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_c: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_c: Option<f32>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl ThermalBody {
    pub fn parse(bytes: &[u8]) -> crate::Result<Self> {
        let body: Self = serde_json::from_slice(bytes)?;
        if let Some(frame) = &body.frame {
            if frame.len() != THERMAL_PIXELS {
                return Err(crate::Error::ThermalFrameLen {
                    got: frame.len(),
                    expected: THERMAL_PIXELS,
                });
            }
        }
        Ok(body)
    }

    pub fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            error: true,
            sensor: Some("MLX90640".into()),
            status: Some("unavailable".into()),
            reason: Some(reason.into()),
            frame: None,
            rows: None,
            cols: None,
            min_c: None,
            max_c: None,
            avg_c: None,
            extra: serde_json::Map::new(),
        }
    }
}

/// Dead-pixel hygiene from the Python original. Not a fire suppressor —
/// ApexOS-RS persistence filter owns alerts.
pub fn clamp_celsius(t: f32) -> f32 {
    t.clamp(THERMAL_MIN_C, THERMAL_MAX_C)
}

pub fn round1(t: f32) -> f32 {
    (t * 10.0).round() / 10.0
}

/// `(min_c, max_c, avg_c)` after clamp, rounded to 0.1 °C like the original.
pub fn frame_stats(frame: &[f32]) -> Option<(f32, f32, f32)> {
    if frame.is_empty() {
        return None;
    }
    let mut lo = f32::MAX;
    let mut hi = f32::MIN;
    let mut sum = 0.0f32;
    for &t in frame {
        let t = clamp_celsius(t);
        lo = lo.min(t);
        hi = hi.max(t);
        sum += t;
    }
    Some((round1(lo), round1(hi), round1(sum / frame.len() as f32)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_is_hygiene_not_a_magnitude_guard() {
        assert_eq!(clamp_celsius(-50.0), -40.0);
        assert_eq!(clamp_celsius(80.0), 80.0);
        assert_eq!(clamp_celsius(1000.0), 300.0);
    }

    #[test]
    fn stats_round_like_python() {
        let frame = [20.04, 21.06, 22.0];
        let (lo, hi, avg) = frame_stats(&frame).unwrap();
        assert_eq!(lo, 20.0);
        assert_eq!(hi, 22.0);
        assert_eq!(avg, 21.0);
    }
}
