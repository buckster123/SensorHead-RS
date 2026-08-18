//! `/api/status` composition. Nested sensor objects stay the upstream's
//! (or an honest degrade). The envelope names this process.

use serde_json::{json, Value};

const NESTED: [&str; 5] = ["i2c_devices", "environment", "thermal", "cameras", "system"];

/// How we reached (or failed to reach) the Python dashboard.
#[derive(Debug, Clone, Copy)]
pub enum UpstreamView<'a> {
    /// No `SENSORHEAD_UPSTREAM` configured.
    Missing,
    /// Upstream was set but the fetch failed. `reason` is the real error.
    Failed(&'a str),
    /// Parsed JSON body from `/api/status`.
    Body(&'a Value),
}

/// Build the drop-in status object. Pure — timestamp and sha are inputs.
pub fn compose_status(
    now: f64,
    git_sha: &str,
    upstream_url: Option<&str>,
    view: UpstreamView<'_>,
) -> Value {
    let mut out = json!({
        "server": "SensorHead-RS",
        "frontend": "sensorhead-rs",
        "git_sha": git_sha,
        "timestamp": now,
    });
    if let Some(url) = upstream_url {
        out["upstream"] = json!(url);
    }

    match view {
        UpstreamView::Missing => {
            out["environment"] = unavailable("BME688", "no upstream configured");
            out["thermal"] = unavailable("MLX90640", "no upstream configured");
            out["cameras"] = json!({"error": "no upstream configured"});
        }
        UpstreamView::Failed(reason) => {
            out["environment"] = unavailable("BME688", reason);
            out["thermal"] = unavailable("MLX90640", reason);
            out["cameras"] = json!({"error": reason});
        }
        UpstreamView::Body(body) => {
            if let Some(server) = body.get("server") {
                out["upstream_server"] = server.clone();
            }
            if let Some(obj) = body.as_object() {
                for key in NESTED {
                    if let Some(v) = obj.get(key) {
                        out[key] = v.clone();
                    }
                }
            } else {
                out["upstream_error"] = json!("status body was not an object");
            }
        }
    }
    out
}

fn unavailable(sensor: &str, reason: &str) -> Value {
    json!({
        "error": true,
        "sensor": sensor,
        "status": "unavailable",
        "reason": reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_upstream_does_not_invent_iaq_or_cameras() {
        let v = compose_status(1.0, "deadbeef", None, UpstreamView::Missing);
        assert_eq!(v["server"], "SensorHead-RS");
        assert_eq!(v["frontend"], "sensorhead-rs");
        assert_eq!(v["git_sha"], "deadbeef");
        assert_eq!(v["environment"]["error"], true);
        assert!(v.get("iaq").is_none());
        assert_eq!(v["cameras"]["error"], "no upstream configured");
        assert!(v.get("upstream").is_none());
    }

    #[test]
    fn failed_upstream_carries_the_real_reason() {
        let v = compose_status(
            1.0,
            "deadbeef",
            Some("http://127.0.0.1:8080"),
            UpstreamView::Failed("connection refused"),
        );
        assert_eq!(v["upstream"], "http://127.0.0.1:8080");
        assert_eq!(v["thermal"]["reason"], "connection refused");
        assert_eq!(v["cameras"]["error"], "connection refused");
    }

    #[test]
    fn live_fixture_keeps_i2c_and_the_camera_error() {
        let path = format!("{}/tests/fixtures/status.json", env!("CARGO_MANIFEST_DIR"));
        let body: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let v = compose_status(
            9.0,
            "cafe",
            Some("http://192.168.0.158:8080"),
            UpstreamView::Body(&body),
        );
        assert_eq!(v["server"], "SensorHead-RS");
        assert_eq!(v["upstream_server"], "SensorHead v0.4.0");
        assert_eq!(v["i2c_devices"]["0x33"], "MLX90640 (thermal)");
        assert_eq!(v["i2c_devices"]["0x77"], "BME688 (environment, alt)");
        assert_eq!(v["environment"]["iaq"], 154.9);
        assert_eq!(v["thermal"]["available"], true);
        assert_eq!(v["cameras"]["error"], "No module named 'picamera2'");
        assert_eq!(v["system"]["mem_total_mb"], 8058);
    }
}
