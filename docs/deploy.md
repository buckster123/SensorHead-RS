# Deploy

House Pi/systemd pattern: `~/Projects/Launchpad-RS/docs/deploy.md`.
**Build on the target. Never cross-compile.**

## Sit beside the live Python dashboard (safe / pre-cutover)

Use this on a node where Python still owns `:8080`. The unit listens on
loopback `:18080` and cannot steal the public port by accident. apex1
has moved on — see thin S4 below.

```sh
# on the Pi
git clone https://github.com/buckster123/SensorHead-RS
cd SensorHead-RS
cargo build --release -p sensorhead-api
sudo cp target/release/sensorhead-api /usr/local/bin/sensorhead-api
sudo cp deploy/sensorhead-api.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable --now sensorhead-api
curl -s http://127.0.0.1:18080/health
curl -s http://127.0.0.1:18080/api/environment
```

Optional knobs in `/etc/sensorhead/env` (`0600`, root-owned) — on a
pre-cutover sidecar:

```
SENSORHEAD_BIND=127.0.0.1:18080
SENSORHEAD_UPSTREAM=http://127.0.0.1:8080
# SENSORHEAD_THERMAL=upstream
```

Exclusive native probe (stops the Python owner — brief):

```sh
sudo systemctl stop sensorhead-dashboard
SENSORHEAD_BIND=127.0.0.1:18081 SENSORHEAD_THERMAL=native \
  ./target/release/sensorhead-api
# elsewhere:
curl -sS http://127.0.0.1:18081/api/thermal/data
sudo systemctl start sensorhead-dashboard
```

**Live on apex1 (2026-08-18):** native frame 24.1–32.6 °C (mean 26.7),
JPEG 320×240. Python back on `:8080` afterwards.

Exclusive BSEC helper probe (stops the Python owner — brief). Reuse the
live state file. See `docs/bsec-sdk.md`.

```sh
sudo systemctl stop sensorhead-dashboard
EGG=$(find /home/apex1/SensorHead/venv -name 'bme68x-*.egg' | head -1)
SENSORHEAD_BIND=127.0.0.1:18081 \
SENSORHEAD_IAQ=helper \
SENSORHEAD_BSEC_HELPER=/home/apex1/SensorHead-RS/walls/bsec.py \
SENSORHEAD_PYTHON=/usr/bin/python3 \
SENSORHEAD_BME68X_EGG="$EGG" \
SENSORHEAD_DATA_DIR=/home/apex1/SensorHead/data \
  /usr/local/bin/sensorhead-api
# elsewhere:
curl -sS http://127.0.0.1:18081/api/environment
sudo systemctl start sensorhead-dashboard
```

**Live on apex1 (2026-08-18):** doctor `ok` on `/usr/bin/python3` + the
leftover venv egg (no venv interpreter). Exclusive `:18081` helper
restored the 238-byte state, returned `source=helper` BSEC 2.6.1.0
(iaq 0.0, accuracy 1), and saved the live state file. Public unit
stayed `SENSORHEAD_IAQ=upstream`.

## Cameras (Python wall on the Pi)

Still capture and IMX500 inference stay in the Python dashboard. On
Raspberry Pi OS / Debian trixie with the Pi repo:

```sh
sudo apt install -y python3-picamera2 python3-libcamera \
  imx500-firmware imx500-models rpicam-apps-imx500-postprocess
# existing venv — do not recreate (BSEC egg lives there)
sed -i 's/^include-system-site-packages = false$/include-system-site-packages = true/' \
  /home/apex1/SensorHead/venv/pyvenv.cfg
sudo systemctl restart sensorhead-dashboard
```

`rpicam-hello` / `rpicam-still` proving the CSI bus is not enough — the
dashboard venv must import `picamera2` from `/usr/lib/python3/dist-packages`.

**Live on apex1 (2026-08-18, later the same day as the sidecar):** both
CSI cameras answer. Visual 2028×1520 and NoIR 2304×1296 JPEGs via
`:8080` and `:18080`. Detect reaches the chip; first outputs were empty
(see `docs/gotchas.md`).

## BSEC state (Python wall, writable path)

The dashboard must be able to write `bsec_state.json`. Upstream SensorHead
#2 defaults `data_dir` to `<repo>/data` and honors `SENSORHEAD_DATA_DIR`.
On apex1 that is `/home/apex1/SensorHead/data/bsec_state.json` (238-byte
real blob, 2026-08-18). Accuracy 0 after restore is still stabilizing,
not a broken save.

```sh
curl -sS http://127.0.0.1:8080/api/environment/save-state
# {status: saved, file: /home/apex1/SensorHead/data/bsec_state.json}
```

## Thin cutover (S4 — live on apex1 2026-08-18)

Python is the BSEC / Picamera2 wall on loopback `:8081`. Rust owns
public `:8080` and proxies that wall. `SENSORHEAD_THERMAL` stays
`upstream`. The sidecar port `:18080` is gone.

```sh
# files in deploy/cutover/
sudo mkdir -p /etc/sensorhead \
  /etc/systemd/system/sensorhead-dashboard.service.d \
  /etc/systemd/system/apex-sensor-bridge.service.d
sudo install -m 0600 deploy/cutover/sensorhead.env /etc/sensorhead/env
sudo cp deploy/cutover/sensorhead-dashboard.conf \
  /etc/systemd/system/sensorhead-dashboard.service.d/port.conf
sudo cp deploy/cutover/apex-sensor-bridge-sensorhead.conf \
  /etc/systemd/system/apex-sensor-bridge.service.d/sensorhead.conf
sudo systemctl daemon-reload
sudo systemctl restart sensorhead-dashboard sensorhead-api
sudo systemctl restart apex-sensor-bridge
curl -sS http://127.0.0.1:8080/health
# service=sensorhead-rs thermal=upstream upstream=http://127.0.0.1:8081
```

**Live evidence (apex1, same afternoon):** `/health` `git_sha=fd4bb1c`.
Environment BSEC body on `:8080`. Thermal 768 floats. Visual 2028×1520
and NoIR 2304×1296 JPEGs. `apex-sensor-bridge` connected;
`agentd` logged `AirQuality` + `ThermalFrame` after the port swap
(accuracy 0 still stabilizing). Rollback: remove the two drop-ins and
`/etc/sensorhead/env`, daemon-reload, restart the three units.
