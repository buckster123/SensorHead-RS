# Upstream source and the FFI walls

## `py-source/` — read-only checkout

This laptop had never cloned the original. The launchpad stamp created
`~/Projects/SensorHead-RS/`; the Python tree lives next to the Rust workspace as a
nested git checkout:

```sh
git clone https://github.com/buckster123/SensorHead.git py-source
# later:
git -C py-source pull --ff-only
git -C py-source log -1 --oneline
```

`py-source/` is in `.gitignore`. It is a **source reference**, not a vendored crate
and not a submodule (we can promote it to a submodule once this repo has a remote).

**Do not modify `py-source/`.** The original repo is ground truth for behaviour we
have not re-specified. Fixes that belong in Python go to `buckster123/SensorHead`.
Reimplementation happens in this workspace.

First-look map:

| Path | What it is |
|------|------------|
| `py-source/README.md` | Hardware table, `:8080` routes, BSEC / camera notes |
| `py-source/SESSION_KNOWLEDGE.md` | Wiring, I2C piggyback, BSEC calibration, startup |
| `py-source/sensor_head/dashboard.py` | FastAPI routes |
| `py-source/sensor_head/server.py` | FastMCP tools |
| `py-source/sensor_head/hardware/environment.py` | BME688 + BSEC2 + raw fallback |
| `py-source/sensor_head/hardware/thermal.py` | MLX90640 + ironbow |
| `py-source/sensor_head/hardware/cameras.py` | Picamera2 / libcamera |
| `py-source/sensor_head/hardware/inference.py` | IMX500 on-chip models |

## Why some of this stays C / Python

The original's own requirements list is the story. Two stacks, not "Python because
we started there":

### 1. Bosch BSEC2 — the nose

`BME688 + BSEC2` in the README: official Bosch Sensortec Environmental Cluster
**v2.6.1.0**, consumed today via the pi3g `bme68x` Python package
(`from bme68x import BME68X`, `bsecConstants`). On the live Pi the import path
includes a prebuilt

`bme68x-2.6.1-py3.13-linux-aarch64.egg`.

BSEC is closed-source C. It is what turns raw MOX gas resistance into IAQ,
CO₂-equivalent, breath-VOC, and heater-compensated T/RH. ApexOS-RS already
documents that **BSEC is optional**: `adafruit_bme680` raw mode still yields
T/RH/P/gas; it just will not emit an `AirQuality` event.

What this means for Rust:

- Native I2C to the BME688 can own **raw** channels.
- **IAQ intelligence** means linking Bosch's C (bindgen / cc) *or* talking to a
  tiny Python/`bme68x` sidecar that already has the licensed blob.
- We do not vendor that blob (charter D6).
- We do not invent a substitute IAQ (charter D5).

### 2. libcamera + Picamera2 — the eyes

`cameras.py` and `inference.py` import `picamera2` and `libcamera`. The IMX500
path is `picamera2.devices.imx500.IMX500` — on-chip detection / classify / pose
**before the frame leaves the sensor**. That API is the Raspberry Pi / Sony
Python stack over a C++ libcamera pipeline. There is no honest pure-Rust
replacement for IMX500 model load + metadata today.

What this means for Rust:

- Still capture and inference stay behind a Picamera2 sidecar (or a future
  libcamera C++ shim) until someone proves a Rust libcamera client for IMX500.
- The HTTP/MCP faces can still be Rust: they already treat the cameras as
  "give me a JPEG / JSON" over localhost.

### What is *not* a wall

| Piece | Why it can be Rust |
|-------|--------------------|
| MLX90640 | Plain I2C @ 0x33. Adafruit Blinka is convenience. Discard first 2 frames; 400 kHz I2C. |
| Ironbow | Pure LUT — already reimplemented in ApexOS-RS `ui-slint` for the 32×24 grid |
| IAQ bands / accuracy labels | Pure tables in `environment.py` |
| FastAPI dashboard, MCP, bridge, sentinel policy | Application code |
| I2C scan / status | `i2cdev` / sysfs |

Blinka/`lgpio` show up in ApexOS-RS install notes because the **Python** thermal
and raw-BME path needs them on Pi 5. A native Rust I2C reader does not.

## Consumer already in the garden

`ApexOS-RS` `apex-sensor-bridge` HTTP-polls this service. It never opens
`/dev/i2c-*`. Installer provision of `dtparam=i2c_arm=on` + `i2c-dev` is an
ApexOS-RS concern; this repo assumes the bus exists when hardware is attached.

Argus is the laptop array, not a substitute for this head.
