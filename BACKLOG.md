# SensorHead-RS backlog — slice ledger

A row gets its ✅ when the slice is **merged, deployed, and verified live** — not when tests
pass (house doctrine #5). Notes carry the date and the evidence.

## v1 (native head, same wire)

- [x] **S0 — bootstrap**: launchpad stamp, `docs/design.md`, `docs/upstream.md`, workspace.
      Public at https://github.com/buckster123/SensorHead-RS (2026-08-18).
- [x] **S1 — fixtures + types**: parse `/api/environment` (BSEC + raw + warming_up) and
      `/api/thermal/data` from captured JSON. Merged in #1. Live apex1 fixtures in
      `crates/sensorhead/tests/fixtures/`.
- [x] **S2 — pure views**: IAQ bands, ironbow LUT, thermal min/max/mean, degrade mappers.
      Merged in #1; native heatmap JPEG served from the apex1 sidecar (below).
- [x] **S3 — HTTP face**: `sensorhead-api` drop-in. Merged #1 + #2.
      **Live on apex1 2026-08-18:** `sensorhead-api.service` active,
      `127.0.0.1:18080` → Python `:8080`. `/health` `git_sha=b2efe29`.
      `/api/status` envelope `SensorHead-RS` + I2C 0x33/0x77. Same-day
      follow-up: apt `python3-picamera2` + venv system-site-packages;
      both CSI cameras live; visual/night JPEGs on `:8080` and `:18080`.
      `/api/environment` BSEC keys present. Same-day
      `buckster123/SensorHead` #2: `bsec_state.json` now persists
      (238-byte blob, restores across restart); accuracy still 0 until
      the 48 h calibration. `/api/thermal/data` 768 floats. Native ironbow
      JPEG 200. Python still owns `:8080`.
- [x] **S4 — field cutover** (thin, 2026-08-18): Rust owns `0.0.0.0:8080`
      (`fd4bb1c`, `thermal=upstream` → Python `127.0.0.1:8081`).
      `apex-sensor-bridge` drop-in `SENSORHEAD_URL=http://127.0.0.1:8080`,
      unit active. `agentd` journal: `AirQuality { iaq: 50.0, accuracy: 0 }`
      + `ThermalFrame` after the swap. Visual/night JPEGs still proxy.
      Native I2C not flipped.

## Post-v1 parking

- Picamera2 / libcamera sidecar shape (stdio vs localhost vs bindgen)
- Native MLX90640 I2C (gated; default stays `upstream` until Python
      stops opening 0x33 and the unit gets `DeviceAllow=/dev/i2c-1`).
      **apex1 2026-08-18 exclusive probe:** dashboard stopped, `SENSORHEAD_THERMAL=native`
      on `:18081`, 768-float frame min 24.1 / max 32.6 / avg 26.7 °C, ironbow
      JPEG 320×240. Python dashboard restored; its next frame 24.5 / 32.8 / 26.8.
      Live sidecar stays `upstream` + `PrivateDevices=true`.
- House MCP face (replace FastMCP)
- ApexAurum cloud bridge
- Pan-tilt / PCA9685
- Dashboard HTML rewrite
- eNose / parallel heater profiles
