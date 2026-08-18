//! Fixture tests built from a live apex1 capture (2026-08-18) plus the
//! synthetic degrade shapes from the Python original.

use sensorhead::{
    frame_stats, iaq_accuracy_label, iaq_band, EnvironmentBody, ThermalBody, THERMAL_PIXELS,
};

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(path).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

#[test]
fn live_environment_is_bsec_and_apexos_can_emit_air_quality() {
    let env = EnvironmentBody::parse(&fixture("environment.json")).unwrap();
    assert!(!env.error);
    assert!(env.temperature_c.is_some());
    assert!(env.humidity_pct.is_some());
    assert!(env.pressure_hpa.is_some());
    assert!(env.iaq.is_some());
    assert_eq!(env.bsec_version.as_deref(), Some("2.6.1.0"));
    assert!(env.apexos_air_quality_possible());
    let iaq = env.iaq.unwrap();
    assert_eq!(iaq_band(iaq).quality, env.air_quality.as_deref().unwrap());
    assert_eq!(
        iaq_accuracy_label(env.iaq_accuracy.unwrap()),
        env.iaq_accuracy_label.as_deref().unwrap()
    );
}

#[test]
fn live_environment_keeps_apexos_keys_after_reserialize() {
    let env = EnvironmentBody::parse(&fixture("environment.json")).unwrap();
    let v = serde_json::to_value(&env).unwrap();
    for key in [
        "temperature_c",
        "humidity_pct",
        "pressure_hpa",
        "iaq",
        "co2_equivalent_ppm",
        "breath_voc_ppm",
        "iaq_accuracy",
    ] {
        assert!(v.get(key).is_some(), "missing {key}");
    }
}

#[test]
fn raw_mode_has_no_iaq_so_apexos_stays_quiet() {
    let env = EnvironmentBody::parse(&fixture("environment_raw.json")).unwrap();
    assert!(!env.error);
    assert!(env.iaq.is_none());
    assert_eq!(env.air_quality.as_deref(), Some("raw_mode"));
    assert!(!env.apexos_air_quality_possible());
}

#[test]
fn warming_up_is_an_error_not_a_zero_iaq() {
    let env = EnvironmentBody::parse(&fixture("environment_warming.json")).unwrap();
    assert!(env.error);
    assert_eq!(env.status.as_deref(), Some("warming_up"));
    assert!(env.iaq.is_none());
    assert!(!env.apexos_air_quality_possible());
}

#[test]
fn live_thermal_is_32x24_and_stats_recompute() {
    let body = ThermalBody::parse(&fixture("thermal_data.json")).unwrap();
    assert!(!body.error);
    let frame = body.frame.as_ref().unwrap();
    assert_eq!(frame.len(), THERMAL_PIXELS);
    assert_eq!(body.rows, Some(24));
    assert_eq!(body.cols, Some(32));
    let (lo, hi, avg) = frame_stats(frame).unwrap();
    assert_eq!(Some(lo), body.min_c);
    assert_eq!(Some(hi), body.max_c);
    assert_eq!(Some(avg), body.avg_c);
}

#[test]
fn thermal_unavailable_is_honest() {
    let body = ThermalBody::parse(&fixture("thermal_unavailable.json")).unwrap();
    assert!(body.error);
    assert_eq!(body.status.as_deref(), Some("unavailable"));
    assert!(body.frame.is_none());
}

#[test]
fn live_status_names_both_i2c_devices() {
    let v: serde_json::Value = serde_json::from_slice(&fixture("status.json")).unwrap();
    assert_eq!(v["server"], "SensorHead v0.4.0");
    assert_eq!(v["i2c_devices"]["0x33"], "MLX90640 (thermal)");
    assert_eq!(v["i2c_devices"]["0x77"], "BME688 (environment, alt)");
    assert_eq!(v["environment"]["bsec_version"], "2.6.1.0");
    assert_eq!(v["thermal"]["available"], true);
}
