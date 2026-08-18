//! Bosch IAQ bands and accuracy labels — copied from
//! `py-source/sensor_head/hardware/environment.py`. A rename here is a
//! contract break (charter D4 / D5).

/// Bosch IAQ quality band as serialized in `/api/environment`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IaqBand {
    pub quality: &'static str,
    pub description: &'static str,
}

const BANDS: [(f64, f64, IaqBand); 7] = [
    (
        0.0,
        50.0,
        IaqBand {
            quality: "excellent",
            description: "Clean air — fresh and pleasant",
        },
    ),
    (
        51.0,
        100.0,
        IaqBand {
            quality: "good",
            description: "Acceptable air quality",
        },
    ),
    (
        101.0,
        150.0,
        IaqBand {
            quality: "lightly_polluted",
            description: "Sensitive people may notice effects",
        },
    ),
    (
        151.0,
        200.0,
        IaqBand {
            quality: "moderately_polluted",
            description: "Increased discomfort likely",
        },
    ),
    (
        201.0,
        250.0,
        IaqBand {
            quality: "heavily_polluted",
            description: "Significant health effects possible",
        },
    ),
    (
        251.0,
        350.0,
        IaqBand {
            quality: "severely_polluted",
            description: "Health warnings — reduce exposure",
        },
    ),
    (
        351.0,
        500.0,
        IaqBand {
            quality: "extremely_polluted",
            description: "Emergency conditions",
        },
    ),
];

const UNKNOWN: IaqBand = IaqBand {
    quality: "unknown",
    description: "Out of range",
};

/// Classify an IAQ value into a Bosch quality band.
pub fn iaq_band(iaq: f64) -> IaqBand {
    for (lo, hi, band) in BANDS {
        if iaq >= lo && iaq <= hi {
            return band;
        }
    }
    UNKNOWN
}

/// Labels for `iaq_accuracy` 0–3. Out-of-range indexes clamp to 3, matching
/// the Python `IAQ_ACCURACY_LABELS[min(iaq_acc, 3)]`.
pub fn iaq_accuracy_label(accuracy: u8) -> &'static str {
    match accuracy.min(3) {
        0 => "stabilizing",
        1 => "uncertain",
        2 => "calibrating",
        _ => "calibrated",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_match_python_table() {
        assert_eq!(iaq_band(0.0).quality, "excellent");
        assert_eq!(iaq_band(50.0).quality, "excellent");
        assert_eq!(iaq_band(51.0).quality, "good");
        assert_eq!(iaq_band(154.7).quality, "moderately_polluted");
        assert_eq!(iaq_band(500.0).quality, "extremely_polluted");
        assert_eq!(iaq_band(-1.0).quality, "unknown");
        assert_eq!(iaq_band(501.0).quality, "unknown");
    }

    #[test]
    fn accuracy_labels_clamp() {
        assert_eq!(iaq_accuracy_label(0), "stabilizing");
        assert_eq!(iaq_accuracy_label(3), "calibrated");
        assert_eq!(iaq_accuracy_label(9), "calibrated");
    }
}
