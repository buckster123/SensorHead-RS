# SensorHead-RS — the contract

> **Contract first** (house doctrine #1). This document is pinned **before** the code it
> describes. Code follows this doc; a PR that changes behaviour updates this doc in the same
> commit. When the two disagree, that is a bug in one of them — find out which, don't guess.

The shapes below are extracted from `py-source/` (`sensor_head/dashboard.py`,
`hardware/environment.py`, `hardware/thermal.py`, `server.py`) and from the keys
`ApexOS-RS/tools/crates/apex-sensor-bridge` already parses. Live captured JSON from
apex1 (2026-08-18) lives in `crates/sensorhead/tests/fixtures/` — BSEC body,
32×24 thermal frame, and `/api/status`. Synthetic raw-mode / warming_up /
thermal-unavailable fixtures cover the degrade paths that were not on the wire
that afternoon.

## Scope

Covers the HTTP surface on port **8080** that ApexOS-RS and the MCP script consume,
plus the degrade rules. Does **not** cover the ApexAurum cloud bridge, sentinel
push policy, or dashboard HTML.

## The wire / API surface

Default bind: `http://127.0.0.1:8080` (loopback). ApexOS-RS sets `SENSORHEAD_URL`.

| Endpoint | Purpose | Shape |
|---|---|---|
| `GET /api/status` | Health + which senses answered | JSON object; `environment` / `thermal` / `cameras` may each be a reading, `{status: not_detected}`, or `{error: …}` |
| `GET /api/environment` | BME688 (+ BSEC2 when active) | JSON — see Types. ApexOS-RS reads `temperature_c`, `humidity_pct`, `pressure_hpa`, `iaq`, `co2_equivalent_ppm`, `breath_voc_ppm`, `iaq_accuracy`. If `error` is true, the bridge emits nothing |
| `GET /api/environment/save-state` | Persist BSEC calibration blob | `{status: saved\|skipped, …}` |
| `GET /api/thermal/data` | MLX90640 32×24 grid | JSON — `frame` (768 °C floats, row-major), `rows` 24, `cols` 32, `min_c`, `max_c`, `avg_c`. Gateway proxies the frame; the bridge forwards only the three scalars (`avg_c` → `mean_c` on the WS side) |
| `GET /api/thermal/heatmap` | Ironbow JPEG | `image/jpeg` |
| `GET /api/capture/visual` | IMX500 still | `image/jpeg`, or **409** `{error, ai_active: true}` when inference holds the camera |
| `GET /api/capture/night` | IMX708 NoIR still | `image/jpeg` |
| `GET /api/detect` | IMX500 EfficientDet | JSON detections; `confidence` query, default `0.3` |
| `GET /api/classify` | IMX500 MobileNetV2 | JSON classes |
| `GET /api/pose` | IMX500 PoseNet | JSON poses |
| `GET /api/models` | Models on the IMX500 | JSON list |

MCP tools in the Python original (stdio FastMCP) — names to preserve when the
house MCP face lands:

| Tool | Purpose |
|---|---|
| `sense_environment` | BME688 JSON text |
| `sense_thermal` | Ironbow image |
| `read_thermal` | Thermal JSON text (no full frame in the short form) |
| `capture_visual` / `capture_night` | JPEG images |
| `detect_objects` / `classify_scene` / `estimate_poses` | IMX500 on-chip |
| `list_ai_models` | Model inventory |
| `get_head_status` | Same facts as `/api/status` |

## Types

Load-bearing serialized names — a rename is an ApexOS-RS break (charter D4).

**Environment (BSEC active).** Compensated T/RH live in `temperature_c` /
`humidity_pct` (raw copies are `raw_*`). `pressure_hpa` is `raw_pressure / 100`.
`iaq` is 0–500. `iaq_accuracy` is 0–3 (`stabilizing` / `uncertain` / `calibrating`
/ `calibrated`). `co2_equivalent_ppm` is VOC-correlated, not a CO₂ sensor.
`stale: true` means last-known BSEC sample.

**Environment (raw fallback).** Same T/RH/P keys, `iaq: null`,
`air_quality: "raw_mode"`, `bsec_version: "N/A (raw mode)"`. No AirQuality event
on the ApexOS-RS side.

**Environment (not ready).** `{error: true, sensor: "BME688", status: "warming_up"|"read_error"|…, reason: "<real reason>"}`.

**Thermal.** `frame` length 768, row-major, °C. Python clamps `<-40` and `>300`
before stats — that clamp is a display/dead-pixel convenience, **not** a fire
suppressor (ApexOS-RS persistence filter owns alerts). `avg_c` is the mean.

**IAQ bands** (Bosch, copied from `environment.py` — keep identical):

| Range | `air_quality` |
|-------|----------------|
| 0–50 | `excellent` |
| 51–100 | `good` |
| 101–150 | `lightly_polluted` |
| 151–200 | `moderately_polluted` |
| 201–250 | `heavily_polluted` |
| 251–350 | `severely_polluted` |
| 351–500 | `extremely_polluted` |

## Lifecycle / state machine

**Dashboard / daemon.** Start → probe I2C (BME688@0x77, MLX90640@0x33) → lazy-init
each sense on first read → serve. BSEC LP sample is ~3 s; a cold read may loop
briefly then return `warming_up` or a stale last sample. On shutdown, save BSEC
state to `bsec_state.json` when BSEC is active.

**BSEC accuracy.** 0 → 1 → 2 → **3**. Full calibration is ~48 h of continuous
power. Restored state skips the worst of that. Losing the state file is not a
crash — it is a fresh calibration, stated as accuracy 0.

**IMX500.** Still capture and on-chip inference are mutually exclusive. Inference
holds the camera → visual capture returns 409, never a black JPEG.

**A job must never sit in `pending` forever** — these are request/response
senses, not a job queue. A blocked I2C read fails with `read_error` + the OS
reason.

## Environment

| Var | Default | Purpose |
|-----|---------|---------|
| `SENSORHEAD_BIND` | `127.0.0.1:8080` | HTTP listen (proposed; Python today is `--port`) |
| `SENSORHEAD_DATA_DIR` | platform data dir | BSEC state + logs. ApexOS already uses this name for the Python unit |
| `SENSORHEAD_URL` | unset | **Consumer** knob on ApexOS-RS, not this process |

Seed-only until a config file earns its keep. BSEC state on disk **is**
persisted and wins across restarts (the calibration, not the listen address).

## Invariants

- This process is the I2C / camera owner. ApexOS-RS never opens those nodes.
- `iaq` is omitted or JSON-null unless BSEC produced it. No homemade IAQ.
- Thermal `frame` is 32×24 row-major. Ironbow is a view; the floats are truth.
- MCP (when present) logs on stderr only.
- BSEC blobs stay out of git (charter D6).

## Honest degrades

| Condition | Response |
|-----------|----------|
| No BME688 | `/api/environment` error object with `status` + `reason`; bridge stays quiet |
| BSEC missing / init failed | Raw T/RH/P/gas, `iaq` null, `air_quality: raw_mode` |
| BSEC warming | `error: true`, `status: warming_up` — not a zeroed IAQ |
| No MLX90640 | thermal error object, `status: unavailable` |
| IMX500 held by AI | 409 + `ai_active: true` |
| No cameras | capture 500 with the libcamera/Picamera2 error string |
| Not a sensor node | daemon may still start; every sense reports unavailable. Never crash-loop |

## Open questions

- Exact listen/env names once the Rust binary exists (`--port` vs `SENSORHEAD_BIND`).
- Whether `/api/thermal/data` keeps serving the full 768-float frame from Rust
  (gateway depends on it) while the bridge keeps ignoring it — yes, unless ApexOS-RS
  changes; assume yes.
- Whether `/api/thermal/heatmap` stays a native Rust render (current
  `sensorhead-api` path) or must byte-match the Python JPEG. Consumers today
  want a JPEG, not a specific encoding.
