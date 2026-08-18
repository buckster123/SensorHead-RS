# SensorHead-RS backlog — slice ledger

A row gets its ✅ when the slice is **merged, deployed, and verified live** — not when tests
pass (house doctrine #5). Notes carry the date and the evidence.

## v1 (native head, same wire)

- [x] **S0 — bootstrap**: launchpad stamp, `docs/design.md`, `docs/upstream.md`, workspace.
      `py-source/` cloned from `buckster123/SensorHead` (local, gitignored).
      Not committed / no remote yet — first commit is André's call.
- [ ] **S1 — fixtures + types**: parse `/api/environment` (BSEC + raw + warming_up) and
      `/api/thermal/data` from captured JSON. Unit tests, no hardware.
- [ ] **S2 — pure views**: IAQ bands, ironbow LUT, thermal min/max/mean, degrade mappers.
- [ ] **S3 — HTTP face**: axum daemon that can stand in for `:8080` for the two ApexOS-RS
      poll routes; BSEC/cameras may still be sidecars.
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
