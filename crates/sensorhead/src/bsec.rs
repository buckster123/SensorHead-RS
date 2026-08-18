//! BSEC2 wall — source selection. The helper itself is a system-Python
//! stdio process (`walls/bsec.py`); this crate only names the mode.

/// Where `/api/environment` gets its BME688 / IAQ body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IaqSource {
    /// Proxy the Python dashboard (default). Does not spawn a helper.
    Upstream,
    /// Spawn `walls/bsec.py` on system Python + an operator-supplied egg.
    /// Exclusive — do not combine with a live Python BSEC owner.
    Helper,
}

impl IaqSource {
    pub fn parse(mode: &str) -> std::result::Result<Self, String> {
        match mode.trim().to_ascii_lowercase().as_str() {
            "" | "upstream" => Ok(Self::Upstream),
            "helper" => Ok(Self::Helper),
            other => Err(format!(
                "unknown SENSORHEAD_IAQ {other:?}; expected upstream or helper"
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
        assert_eq!(IaqSource::parse("").unwrap(), IaqSource::Upstream);
        assert_eq!(IaqSource::parse("upstream").unwrap(), IaqSource::Upstream);
    }

    #[test]
    fn parse_helper() {
        assert_eq!(IaqSource::parse("helper").unwrap(), IaqSource::Helper);
    }

    #[test]
    fn parse_rejects_typos_instead_of_silently_proxying() {
        let err = IaqSource::parse("help").unwrap_err();
        assert!(err.contains("help"), "{err}");
        assert!(err.contains("upstream or helper"), "{err}");
    }
}
