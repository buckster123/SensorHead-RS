# BSEC2 SDK — operator path (never fetched)

Bosch BSEC2 is closed-source C. Charter **D6 / D9**: this repo never vendors
the `.egg` / `.so` / headers, and `sensorhead-api --doctor` never downloads
them. IAQ is absent until the operator places the pi3g `bme68x` egg on disk.

## What you need

1. Bosch Sensortec BSEC2 **v2.6.1.0** (accept Bosch's licence on their site).
2. The pi3g Python wrapper built for your Pi's Python (`bme68x-2.6.1-py3.13-linux-aarch64.egg` on apex1).
3. System Python — `/usr/bin/python3`. **Not a venv.**

The live apex1 leftover lives in the old SensorHead venv. That path is a
*source to copy from*, not a runtime we keep:

```
/home/apex1/SensorHead/venv/lib/python3.13/site-packages/bme68x-2.6.1-py3.13-linux-aarch64.egg
```

Preferred home:

```
sudo mkdir -p /opt/sensorhead
sudo cp /path/to/bme68x-*.egg /opt/sensorhead/
# /etc/sensorhead/env (0600):
# SENSORHEAD_BME68X_EGG=/opt/sensorhead/bme68x-2.6.1-py3.13-linux-aarch64.egg
```

## Doctor (does not open I2C)

```sh
# from the SensorHead-RS checkout
sensorhead-api --doctor
# or:
python3 walls/bsec.py --doctor
```

`ok: true` means `import bme68x` worked. Exit 1 means the egg is missing —
that is an honest degrade, not a crash. `/api/environment` then returns
`error: true` with the import reason and **no** `iaq`.

## Helper (exclusive)

`SENSORHEAD_IAQ=helper` spawns `walls/bsec.py` on `SENSORHEAD_PYTHON`.
Do not run it while `sensorhead-dashboard` still inits BSEC on 0x77.
Do not point `SENSORHEAD_PYTHON` at a venv. Bindgen to BSEC C is later.

State file is `$SENSORHEAD_DATA_DIR/bsec_state.json` — same shape as the
Python original. On a brief exclusive probe, reuse the live file so
calibration is not reset.
