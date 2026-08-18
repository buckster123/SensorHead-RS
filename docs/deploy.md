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
```

## Cutover (S4 — not this slice)

Move Python to a side port, point `SENSORHEAD_BIND` at `0.0.0.0:8080`,
set `SENSORHEAD_URL=http://127.0.0.1:8080` on `apex-sensor-bridge`,
enable the bridge. Do that as its own slice, with evidence on apex1.
