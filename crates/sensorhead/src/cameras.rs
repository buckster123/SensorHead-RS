//! Picamera2 wall — source selection. The helper is `walls/cameras.py`.

/// Where `/api/capture/*`, `/api/detect`, `/api/classify`, `/api/pose`,
/// and `/api/models` get their bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraSource {
    /// Proxy the Python dashboard (default). Does not spawn a helper.
    Upstream,
    /// Spawn `walls/cameras.py` on system Python + apt picamera2.
    /// Exclusive on CSI — do not combine with a live Python camera owner.
    Helper,
}

impl CameraSource {
    pub fn parse(mode: &str) -> std::result::Result<Self, String> {
        match mode.trim().to_ascii_lowercase().as_str() {
            "" | "upstream" => Ok(Self::Upstream),
            "helper" => Ok(Self::Helper),
            other => Err(format!(
                "unknown SENSORHEAD_CAMERAS {other:?}; expected upstream or helper"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upstream => "upstream",
            Self::Helper => "helper",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_is_upstream() {
        assert_eq!(CameraSource::parse("").unwrap(), CameraSource::Upstream);
        assert_eq!(
            CameraSource::parse("upstream").unwrap(),
            CameraSource::Upstream
        );
    }

    #[test]
    fn parse_helper() {
        assert_eq!(CameraSource::parse("helper").unwrap(), CameraSource::Helper);
    }

    #[test]
    fn parse_rejects_typos_instead_of_silently_proxying() {
        let err = CameraSource::parse("help").unwrap_err();
        assert!(err.contains("help"), "{err}");
        assert!(err.contains("upstream or helper"), "{err}");
    }
}
