# SensorHead-RS — charter

> **The decisions log below is BINDING.** Amend it with a dated entry; never silently.
> Where this document and the code disagree, one of them is a bug — say which.
> Where a later doc and D1–Dn disagree, **D1–Dn win**.

## What this is

The SensorHead option for an ApexOS Pi 5: a small daemon that reads the attached sense
array (air, thermal, cameras) and serves the same HTTP — and later MCP — surface the
Python original does, so an agent can see the room it lives in.

## What it is not

- Not an ApexOS-RS crate. ApexOS-RS **polls** this service; it does not own the I2C
  devices or the camera pipeline (`PrivateDevices=true` on `apex-sensor-bridge` is load-bearing).
- Not a laptop sensor array (that is Argus).
- Not a cloud product. The ApexAurum bridge in the Python original is out of v1 here.
- Not a reimplementation of Bosch BSEC or Sony's IMX500 firmware. We call those, we
  do not clone their signal processing.
- Not a 3D "face", pan-tilt rig, or eNose classifier in v1.

## Decisions

Numbered, binding, dated. One decision per entry, with the reason — a decision whose
rationale is lost gets re-litigated within a month.

- **D1 — Standalone sibling.** This is its own repo and workspace. ApexOS-RS is the
  first consumer, never the owner. Assimilation, if it ever happens, is decided in
  *that* repo's thread. Rules out merging this tree into `ApexOS-RS/tools`.
- **D2 — Rust as far as possible; FFI only at named walls.** The walls, named from the
  original's own docs (`py-source/README.md`, `environment.py`, `cameras.py`):
  **(1) Bosch BSEC2** — closed-source C, shipped today as the pi3g `bme68x` Python
  extension (`.egg` on aarch64). Required for IAQ / CO₂eq / breath-VOC / compensated
  T+RH. **(2) libcamera + Picamera2** — C++ camera stack; IMX500 on-chip inference
  is specifically `picamera2.devices.imx500`. Everything else (HTTP, MCP, MLX90640
  I2C, raw BME688, ironbow, status, sentinel policy) is a native-Rust candidate.
  Rules out "keep the whole dashboard in Python because one library is C".
- **D3 — `py-source/` is a read-only checkout.** `git clone` of
  `https://github.com/buckster123/SensorHead` into `py-source/`. Gitignored from
  *this* repo. Refresh with `git -C py-source pull`. **Do not edit files in
  `py-source/`.** Fixes go upstream in that repo, or are reimplemented here in Rust.
- **D4 — Keep the Python `:8080` JSON contract.** `apex-sensor-bridge` already parses
  `/api/environment` (`temperature_c`, `humidity_pct`, `pressure_hpa`, `iaq`,
  `co2_equivalent_ppm`, `breath_voc_ppm`, `iaq_accuracy`) and `/api/thermal/data`
  (`min_c`, `max_c`, `avg_c`, plus the 768-float `frame` the gateway proxies). A
  field rename here is a coordinated ApexOS-RS change — don't.
- **D5 — Honest degrade, never a fake nose.** Missing I2C, BSEC init failure, camera
  busy (409), or a warming-up BSEC (`error: true`, `status: warming_up`) is returned
  as that fact. Do not invent an IAQ number from raw gas resistance. ApexOS-RS
  already treats absent `iaq` as "no AirQuality event".
- **D6 — Do not redistribute BSEC2.** Bosch's library has its own licence. We never
  vendor the `.so` / `.egg` / headers into this git tree. The sidecar obtains them
  the same way the Pi already does.
- **D7 — Field truth is apex1.** The SensorHead option is verified live there
  (BME688@0x77, MLX90640@0x33, I2C needs `i2c-dev` on Pi 5). Laptop work is types,
  parsers, and fixtures. A slice that talks to hardware is done only on the node.
- **D8 — FORGE may merge on this repo.** André loosened the house "do not merge"
  rule here on 2026-08-18: one branch, one slice, then commit → push → merge →
  next, as long as `cargo test --workspace` and `clippy -- -D warnings` are
  green. Still no force-push. Still no commit directly to `main`.
- **D9 — FFI walls are in-repo stdio helpers on system Python.** No venv.
  `walls/bsec.py` and `walls/cameras.py` speak newline JSON on stdio.
  The operator supplies the BSEC egg at `SENSORHEAD_BME68X_EGG`; this tree
  never vendors it. `sensorhead-api --doctor` checks the import and does
  **not** fetch Bosch's SDK. `bindgen` to BSEC C is a later exclusive step
  for IAQ, not a second architecture. Default `SENSORHEAD_IAQ` stays
  `upstream` until an exclusive cutover. Rules out recreating a "small venv"
  for hygiene, and rules out an installer that downloads the blob.

## Phases

Each with a "done when" gate that is checkable, not aspirational.

| Phase | Scope | Done when |
|-------|-------|-----------|
| P0 | Bootstrap + source ref + portability map | `py-source/` present, launchpad placeholders gone, walls named in this charter |
| P1 | Pin the HTTP contract with fixtures | `docs/design.md` has captured JSON from the original (and, when reachable, apex1); rust types parse both BSEC and raw-mode bodies |
| P2 | Native pieces that need no vendor lib | ironbow, IAQ bands, thermal stats, degrade mappers — unit-tested, no I²C |
| P3 | Daemon face that can stand in for `:8080` | `GET /api/environment` and `GET /api/thermal/data` answer the ApexOS-RS parsers; BSEC/cameras may still be sidecars |
| P4 | Field cutover on apex1 | `SENSORHEAD_URL` pointed at the Rust daemon; bridge still emits AirQuality + ThermalFrame; evidence in `BACKLOG.md` |

## Deliberately out of v1

Each with the reason, so a future reader knows it was a decision and not an oversight.

**Permanently out**

- Reimplementing Bosch BSEC signal processing — proprietary, 48-hour calibration
  state, and we have no right to clone it
- Owning `/dev/i2c-*` from inside ApexOS-RS — the sandbox split stays

**Out of v1, honestly deferred**

- ApexAurum cloud bridge — the Python original has it; this repo is the local head first
- Pan-tilt / PCA9685 — hardware isn't a working rig (servo kit buckles under the stack)
- Custom IMX500 models and eNose / parallel heater profiles
- Rewriting the cyberpunk dashboard HTML — serve the existing static tree or a
  minimal status page until the API is native
- Hand-rolled MCP face — Python FastMCP stays until the HTTP daemon is the live one

## Open questions

- FFI shape for the two walls: long-lived Python sidecar, `bindgen` to BSEC C, or
  subprocess to a thin `bme68x` helper? Same question for Picamera2.
  **Answered 2026-08-18 (D9):** in-repo stdio helpers on `/usr/bin/python3`.
  BSEC helper is `walls/bsec.py` behind `SENSORHEAD_IAQ=helper`. Picamera2
  helper is `walls/cameras.py` behind `SENSORHEAD_CAMERAS=helper`.
  `bindgen` later for IAQ only.
- Does MLX90640 go native Rust in P3 (`embedded-hal` / `i2cdev`) or stay behind the
  same sidecar as BSEC for one I2C owner? **Answered 2026-08-18:** native exists,
  gated (`SENSORHEAD_THERMAL=native`). Default stays `upstream` while Python
  owns the bus. The public unit keeps `PrivateDevices=true`.
- House MCP (newline JSON-RPC, no SDK) vs keeping FastMCP until a consumer other
  than ApexOS-RS needs stdio.

---

## Amendments

Dated entries. A decision changes here first, then in the code.

- **2026-08-18** — charter adopted. Source: `py-source/` at `buckster123/SensorHead`
  (clone tip `6da6541` on this laptop) plus ApexOS-RS `docs/gotchas.md` (sensor-head
  = external service).
- **2026-08-18** — D8 adopted. André: cook with commit-push-merge while tests
  stay green.
- **2026-08-18** — MLX90640 is native-capable behind `SENSORHEAD_THERMAL`.
  Default remains the Python sidecar so there is one I2C owner. S4 thin
  (same day) put Rust on public `:8080` with `thermal=upstream`; native
  I2C and `DeviceAllow` stay a later exclusive step.
- **2026-08-18** — D9 adopted. André: thin stdio helpers in this repo, system
  Python, no venv, doctor-not-fetch, bindgen later for IAQ.
- **2026-08-18** — Picamera2 wall is `walls/cameras.py` (`SENSORHEAD_CAMERAS=helper`).
  Apt stack only; doctor does not open CSI. Default stays `upstream`.
