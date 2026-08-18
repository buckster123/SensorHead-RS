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
      `/api/status` envelope `SensorHead-RS` + I2C 0x33/0x77 + honest
      `picamera2` camera error. `/api/environment` BSEC keys present
      (IAQ 500 / accuracy 0 — no `bsec_state.json`). `/api/thermal/data`
      768 floats. Native ironbow JPEG 200. Python still owns `:8080`.
- [ ] **S4 — field cutover**: Rust on `:8080`, `SENSORHEAD_URL` on
      `apex-sensor-bridge`, bridge emits AirQuality + ThermalFrame.

## Post-v1 parking

- Picamera2 / libcamera sidecar shape (stdio vs localhost vs bindgen)
- Native MLX90640 I2C vs one sidecar owning the whole bus
- House MCP face (replace FastMCP)
- ApexAurum cloud bridge
- Pan-tilt / PCA9685
- Dashboard HTML rewrite
- eNose / parallel heater profiles
