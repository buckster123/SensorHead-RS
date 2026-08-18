# Picamera2 wall — apt stack, never pip

libcamera / Picamera2 (and IMX500 on-chip AI) is the second named FFI wall.
Charter **D9**: `walls/cameras.py` on `/usr/bin/python3`. No venv.

## What you need

On a Raspberry Pi with CSI cameras:

```sh
sudo apt install python3-picamera2 python3-libcamera
# IMX500 on-chip models:
sudo apt install imx500-firmware imx500-models
```

**Do not `pip install picamera2`.** That wheel cannot talk to the Pi
libcamera stack.

## Doctor (does not open CSI)

```sh
sensorhead-api --doctor
# or:
python3 walls/cameras.py --doctor
```

`cameras.ok: true` means `import picamera2` and `import libcamera` worked.
Exit 1 from `--doctor` only when **both** BSEC and cameras imports failed.

## Helper (exclusive)

`SENSORHEAD_CAMERAS=helper` spawns `walls/cameras.py`. Do not run it while
`sensorhead-dashboard` still opens Picamera2. JPEG replies are `jpeg_b64`
on a JSON line; the HTTP face decodes them to `image/jpeg`. IMX500 still
and inference stay exclusive — visual capture returns **409**
`{ai_active: true}` if a detect/classify/pose is in flight.

Default `SENSORHEAD_CAMERAS` stays `upstream`.
