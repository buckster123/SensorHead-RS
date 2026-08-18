# Deploy

House Pi/systemd pattern: `~/Projects/Launchpad-RS/docs/deploy.md`.
**Build on the target. Never cross-compile.**

## Sit beside the live Python dashboard (safe)

apex1 already owns `:8080` (`sensorhead-dashboard.service`). This unit
listens on loopback `:18080` and proxies that dashboard. It cannot steal
the public port by accident.

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

**Live on apex1 (2026-08-18):** unit `sensorhead-api.service` is enabled,
binary at `/usr/local/bin/sensorhead-api`, checkout `~/SensorHead-RS`
(`b2efe29`). Python still owns `:8080`.

Optional knobs in `/etc/sensorhead/env` (`0600`, root-owned):

```
SENSORHEAD_BIND=127.0.0.1:18080
SENSORHEAD_UPSTREAM=http://127.0.0.1:8080
# SENSORHEAD_THERMAL=upstream   # default — do not set native on this unit
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

## Cutover (S4 — not this slice)

Move Python to a side port, point `SENSORHEAD_BIND` at `0.0.0.0:8080`,
set `SENSORHEAD_URL=http://127.0.0.1:8080` on `apex-sensor-bridge`,
enable the bridge. Do that as its own slice, with evidence on apex1.
