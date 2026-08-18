# Gotchas — the invariant ledger

> **RULE: before modifying ANY subsystem, grep this file for it and read the matching
> entries.** These are load-bearing invariants — most were written after something broke
> on a live node, and many end with an explicit "don't do X" that a future change would
> otherwise walk straight into.
>
> **A newly discovered gotcha goes HERE**, not in CLAUDE.md. Docs travel with code —
> update this file in the same PR as the change that discovered or altered an invariant.
>
> Format: one bullet, **bold lead naming the invariant**, then the story, ending with the
> explicit don't. Cross-project version drift lives in
> `~/Projects/Launchpad-RS/docs/sharp-edges.md` instead.

- **BSEC2 is the IAQ wall, not "the BME688 is Python".** Raw T/RH/P/gas work without
  Bosch via `adafruit_bme680` or native I2C. IAQ / CO₂eq / breath-VOC / compensated T+RH
  come from the closed-source BSEC C library (pi3g `bme68x` 2.6.1 on the live Pi).
  ApexOS-RS already drops the AirQuality event when `iaq` is absent. **Don't invent an
  IAQ from raw gas ohms, and don't vendor the BSEC `.egg` / `.so` into this git tree.**

- **libcamera / Picamera2 is the camera wall, especially IMX500.** Still capture and
  on-chip detect/classify/pose go through `picamera2` (`devices.imx500.IMX500`).
  **Don't promise a pure-Rust IMX500 path without a proven libcamera client.**

- **This service owns the devices; ApexOS-RS only HTTP-polls.**
  `apex-sensor-bridge` is `PrivateDevices=true` on purpose. **Don't open `/dev/i2c-*`
  or CSI from an ApexOS-RS crate "to simplify".**

- **Pi 5 I2C needs `i2c-dev`, not just `dtparam=i2c_arm=on`.** Without the module there
  are no `/dev/i2c-*` nodes. ApexOS-RS `install.sh` provisions this when the sensor
  option is selected; it needs a reboot. **Don't debug "no BME688" as a Rust bug until
  `i2cdetect -y 1` shows 0x77 / 0x33.**

- **BME688@0x77, MLX90640@0x33, shared bus, different rails.** Thermal VIN is Pi 3.3 V,
  not the BME688 breakout's 5 V VDD. **Don't power the MLX from the BME VDD pin.**

- **Discard the first two MLX90640 frames.** Warm-up garbage. Default 100 kHz I2C is
  ~1.4 s/frame; 400 kHz via dtparam is ~0.4 s. **Don't treat frame 0 as a real scene.**

- **BSEC full calibration is ~48 hours; save `bsec_state.json`.** Accuracy 0→3.
  Compensated temperature runs ~3–5 °C below raw (heater). CO₂eq is VOC-derived, not a
  NDIR CO₂ reading. **Don't compare raw BME temp to a room thermometer and call the
  sensor wrong; don't advertise CO₂eq as real CO₂.**

- **IMX500 still capture and inference are exclusive.** Dashboard returns 409
  `{ai_active: true}` when the AI engine holds CAM0. **Don't return a black JPEG.**

- **Thermal 300 °C spikes were a lighter, not a stuck pixel** (apex1, ApexOS-RS
  gotchas). Alerts belong to the consumer's persistence filter, not a magnitude
  clamp in this repo. The Python clamp of `<-40` / `>300` is dead-pixel hygiene for
  the grid. **Don't silence a sustained hotspot here to "fix" transients.**

- **`py-source/` is read-only and gitignored.** **Don't edit the checkout; don't
  `git add` it.**

- **apex1 2026-08-18 field read.** `sensorhead-dashboard.service` is the live
  Python unit (`/home/apex1/SensorHead/venv`, binds `0.0.0.0:8080`). I2C shows
  BME688@0x77 + MLX90640@0x33. BSEC 2.6.1.0 **is** in that venv
  (`bme68x-2.6.1-py3.13-linux-aarch64.egg`) and `/api/environment` returns `iaq`
  — but `data/` has no `bsec_state.json`, so accuracy stays 0 (stabilizing) and
  IAQ can peg at 500. `picamera2` is **not** installed; capture/detect return
  `{"error":"No module named 'picamera2'"}`. `apex-sensor-bridge` is inactive
  and has no `SENSORHEAD_URL`. **Don't assume the ApexOS-RS "raw-mode, no BSEC"
  note is still the live truth — probe `/api/environment`. Don't treat accuracy-0
  IAQ as a calibrated nose.**

- **`/api/environment/save-state` can lie.** Same afternoon it returned
  `{status: skipped, reason: BSEC not active}` while `/api/environment` was
  returning a full BSEC body. Lazy-init / instance-state bug in the Python
  original. **Don't use save-state as the BSEC-alive probe; read the environment
  body.**
