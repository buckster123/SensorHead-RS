# SensorHead-RS backlog — slice ledger

A row gets its ✅ when the slice is **merged, deployed, and verified live** — not when tests
pass (house doctrine #5). Notes carry the date and the evidence.

## v1 (native head, same wire)

- [x] **S0 — bootstrap**: launchpad stamp, `docs/design.md`, `docs/upstream.md`, workspace.
      `py-source/` cloned from `buckster123/SensorHead` (local, gitignored).
      Root commit `9f69e52` on `main` (2026-08-18). No GitHub remote yet.
- [ ] **S1 — fixtures + types**: parse `/api/environment` (BSEC + raw + warming_up) and
      `/api/thermal/data` from captured JSON. Unit tests, no hardware.
      *Evidence 2026-08-18 (laptop, not merged):* live apex1 capture in
      `crates/sensorhead/tests/fixtures/`; 7 fixture tests + 7 lib tests green.
- [ ] **S2 — pure views**: IAQ bands, ironbow LUT, thermal min/max/mean, degrade mappers.
      *Evidence 2026-08-18:* bands/accuracy/clamp/stats/LUT unit-tested; heatmap
      320×240 from a 32×24 frame.
- [ ] **S3 — HTTP face**: axum daemon that can stand in for `:8080` for the two ApexOS-RS
      poll routes; BSEC/cameras may still be sidecars.
      *Evidence 2026-08-18 (laptop → live apex1, not a cutover):*
      `sensorhead-api --bind 127.0.0.1:18080 --upstream http://192.168.0.158:8080`
      served `/api/environment` (BSEC 2.6.1.0 keys present), `/api/thermal/data`
      (768 floats), and a native-ironbow `/api/thermal/heatmap` JPEG.
      Python still owns apex1 `:8080`.
      `deploy/sensorhead-api.service` sits on loopback `:18080` so a Pi
      install cannot clobber the live dashboard.
      `/health` reports `version` + `git_sha`; `/api/status` is composed
      (Rust envelope, upstream nests, honest camera error).
- [ ] **S4 — field**: `SENSORHEAD_URL` on apex1 pointed at the Rust daemon; bridge still
      emits AirQuality + ThermalFrame; evidence recorded here.

## Post-v1 parking

- Picamera2 / libcamera sidecar shape (stdio vs localhost vs bindgen)
- Native MLX90640 I2C vs one sidecar owning the whole bus
- House MCP face (replace FastMCP)
- ApexAurum cloud bridge
- Pan-tilt / PCA9685
- Dashboard HTML rewrite
- eNose / parallel heater profiles
