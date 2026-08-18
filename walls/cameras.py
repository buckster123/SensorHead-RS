#!/usr/bin/env python3
"""Thin Picamera2 wall — system Python + apt stack, no venv.

Stdin/stdout is newline JSON (one request, one reply). Logs go to stderr.
JPEGs travel as jpeg_b64. This process is exclusive on CSI: do not run
it beside a dashboard that also opens Picamera2.

Commands: status | capture_visual | capture_night | models
          detect | classify | pose | shutdown
Doctor:   python3 walls/cameras.py --doctor   (does not open CSI)
"""

from __future__ import annotations

import argparse
import base64
import io
import json
import os
import sys
import time

HINT = (
    "Install apt python3-picamera2 python3-libcamera (and imx500-firmware "
    "/ imx500-models for on-chip AI). Do not pip install picamera2. "
    "Use /usr/bin/python3. See docs/picamera2.md."
)


def emit(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def log(msg: str) -> None:
    sys.stderr.write(f"sensorhead-cameras: {msg}\n")
    sys.stderr.flush()


def parse_size(raw: str, default: tuple[int, int]) -> tuple[int, int]:
    try:
        w, h = raw.lower().split("x", 1)
        return int(w), int(h)
    except Exception:
        return default


IMX500_MODEL = "imx500"
IMX708_MODEL = "imx708_wide_noir"

COCO_LABELS = [
    "person", "bicycle", "car", "motorcycle", "airplane", "bus", "train",
    "truck", "boat", "traffic light", "fire hydrant", "stop sign",
    "parking meter", "bench", "bird", "cat", "dog", "horse", "sheep",
    "cow", "elephant", "bear", "zebra", "giraffe", "backpack", "umbrella",
    "handbag", "tie", "suitcase", "frisbee", "skis", "snowboard",
    "sports ball", "kite", "baseball bat", "baseball glove", "skateboard",
    "surfboard", "tennis racket", "bottle", "wine glass", "cup", "fork",
    "knife", "spoon", "bowl", "banana", "apple", "sandwich", "orange",
    "broccoli", "carrot", "hot dog", "pizza", "donut", "cake", "chair",
    "couch", "potted plant", "bed", "dining table", "toilet", "tv",
    "laptop", "mouse", "remote", "keyboard", "cell phone", "microwave",
    "oven", "toaster", "sink", "refrigerator", "book", "clock", "vase",
    "scissors", "teddy bear", "hair drier", "toothbrush",
]

POSE_KEYPOINTS = [
    "nose", "left_eye", "right_eye", "left_ear", "right_ear",
    "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
    "left_wrist", "right_wrist", "left_hip", "right_hip",
    "left_knee", "right_knee", "left_ankle", "right_ankle",
]

MODELS = {
    "efficientdet": ("EfficientDet Lite0", "imx500_network_efficientdet_lite0_pp.rpk", "detection", "320x320"),
    "ssd_mobilenet": ("SSD MobileNetV2 FPN Lite", "imx500_network_ssd_mobilenetv2_fpnlite_320x320_pp.rpk", "detection", "320x320"),
    "nanodet": ("NanoDet Plus", "imx500_network_nanodet_plus_416x416_pp.rpk", "detection", "416x416"),
    "mobilenet_v2": ("MobileNetV2", "imx500_network_mobilenet_v2.rpk", "classification", "224x224"),
    "efficientnet_b0": ("EfficientNetV2-B0", "imx500_network_efficientnetv2_b0.rpk", "classification", "224x224"),
    "posenet": ("PoseNet", "imx500_network_posenet.rpk", "pose", "481x353"),
    "higherhrnet": ("HigherHRNet", "imx500_network_higherhrnet_coco.rpk", "pose", "variable"),
}


def doctor() -> dict:
    report = {
        "ok": False,
        "helper": "cameras",
        "python": sys.executable,
        "picamera2": False,
        "libcamera": False,
        "imx500": False,
        "reason": None,
        "hint": HINT,
    }
    try:
        import picamera2  # noqa: F401

        report["picamera2"] = True
    except Exception as e:
        report["reason"] = str(e)
        return report
    try:
        import libcamera  # noqa: F401

        report["libcamera"] = True
    except Exception as e:
        report["reason"] = str(e)
        return report
    try:
        from picamera2.devices.imx500 import IMX500  # noqa: F401

        report["imx500"] = True
    except Exception as e:
        report["reason"] = f"picamera2 ok; IMX500 helper missing: {e}"
        report["ok"] = True
        return report
    report["ok"] = True
    return report


def _jpeg_payload(jpeg: bytes, camera: str) -> dict:
    return {
        "ok": True,
        "kind": "jpeg",
        "content_type": "image/jpeg",
        "camera": camera,
        "bytes": len(jpeg),
        "jpeg_b64": base64.b64encode(jpeg).decode("ascii"),
        "source": "helper",
    }


class CameraWall:
    def __init__(self) -> None:
        self._map: dict[str, int] | None = None
        self._import_error: str | None = None
        self._ai_active = False
        self._imx500 = None
        self._picam2 = None
        self._active_model: str | None = None
        self._rotation = int(os.environ.get("SENSORHEAD_CAMERA_ROTATION", "180"))
        self._ae = float(os.environ.get("SENSORHEAD_AE_SETTLE", "1.5"))
        self._imx500_size = parse_size(
            os.environ.get("SENSORHEAD_IMX500_SIZE", "2028x1520"), (2028, 1520)
        )
        self._noir_size = parse_size(
            os.environ.get("SENSORHEAD_NOIR_SIZE", "2304x1296"), (2304, 1296)
        )
        self._model_dir = os.environ.get("SENSORHEAD_MODEL_DIR", "/usr/share/imx500-models")

    def _imports(self) -> tuple[object, object] | None:
        if self._import_error:
            return None
        try:
            from picamera2 import Picamera2
            from libcamera import Transform

            return Picamera2, Transform
        except Exception as e:
            self._import_error = f"picamera2 not importable: {e}"
            log(self._import_error)
            return None

    def _discover(self) -> dict[str, int]:
        if self._map is not None:
            return self._map
        mods = self._imports()
        if mods is None:
            return {}
        Picamera2, _ = mods
        mapping: dict[str, int] = {}
        for cam in Picamera2.global_camera_info():
            num = cam.get("Num")
            model = cam.get("Model", "")
            if IMX500_MODEL in model:
                mapping["imx500"] = num
                log(f"IMX500 at camera {num}")
            elif IMX708_MODEL in model or "imx708" in model:
                mapping["noir"] = num
                log(f"IMX708 at camera {num}")
            else:
                log(f"unknown camera {model!r} at {num}")
        self._map = mapping
        return mapping

    def _encode_jpeg(self, picam2) -> bytes:
        buf = io.BytesIO()
        try:
            picam2.capture_file(buf, format="jpeg")
            data = buf.getvalue()
            if data[:2] == b"\xff\xd8":
                return data
        except Exception as e:
            log(f"capture_file jpeg failed ({e}); trying PIL")
        array = picam2.capture_array("main")
        from PIL import Image

        out = io.BytesIO()
        Image.fromarray(array).save(out, format="JPEG", quality=85)
        return out.getvalue()

    def _capture(self, camera_num: int, resolution: tuple[int, int]) -> bytes:
        mods = self._imports()
        if mods is None:
            raise RuntimeError(self._import_error)
        Picamera2, Transform = mods
        rot = self._rotation
        hflip = rot in (180, 270)
        vflip = rot in (180, 90)
        last_error = None
        for attempt in range(2):
            if attempt:
                time.sleep(1.5)
            picam2 = Picamera2(camera_num)
            try:
                cfg = picam2.create_still_configuration(
                    main={"size": resolution, "format": "RGB888"},
                    transform=Transform(hflip=hflip, vflip=vflip),
                )
                picam2.configure(cfg)
                picam2.start()
                time.sleep(self._ae)
                return self._encode_jpeg(picam2)
            except Exception as e:
                last_error = e
                log(f"camera {camera_num} capture failed: {e}")
            finally:
                try:
                    picam2.stop()
                except Exception:
                    pass
                try:
                    picam2.close()
                except Exception:
                    pass
        raise RuntimeError(f"camera {camera_num} failed: {last_error}")

    def status(self) -> dict:
        mods = self._imports()
        if mods is None:
            return {
                "error": True,
                "reason": self._import_error,
                "source": "helper",
            }
        Picamera2, _ = mods
        try:
            info = Picamera2.global_camera_info()
            mapping = self._discover()
        except Exception as e:
            return {"error": True, "reason": str(e), "source": "helper"}
        return {
            "cameras_detected": len(info),
            "camera_info": [
                {"num": c.get("Num"), "model": c.get("Model"), "location": c.get("Location")}
                for c in info
            ],
            "imx500_available": "imx500" in mapping,
            "imx500_camera_num": mapping.get("imx500"),
            "noir_available": "noir" in mapping,
            "noir_camera_num": mapping.get("noir"),
            "rotation_deg": self._rotation,
            "source": "helper",
        }

    def capture(self, key: str) -> dict:
        if key == "imx500" and self._ai_active:
            return {
                "error": True,
                "ai_active": True,
                "status": "busy",
                "reason": "IMX500 busy — AI inference active",
                "source": "helper",
            }
        mapping = self._discover()
        if self._import_error:
            return {
                "error": True,
                "status": "unavailable",
                "reason": self._import_error,
                "source": "helper",
            }
        if key not in mapping:
            name = "IMX500 AI Camera" if key == "imx500" else "IMX708 Wide NoIR"
            return {
                "error": True,
                "status": "unavailable",
                "reason": f"{name} not detected",
                "source": "helper",
            }
        size = self._imx500_size if key == "imx500" else self._noir_size
        try:
            jpeg = self._capture(mapping[key], size)
            return _jpeg_payload(jpeg, key)
        except Exception as e:
            return {
                "error": True,
                "status": "unavailable",
                "reason": str(e),
                "source": "helper",
            }

    def models(self) -> dict:
        available = {}
        for key, (name, filename, typ, size) in MODELS.items():
            path = os.path.join(self._model_dir, filename)
            available[key] = {
                "name": name,
                "type": typ,
                "input_size": size,
                "installed": os.path.exists(path),
                "active": key == self._active_model,
            }
        return available

    def _teardown_ai(self) -> None:
        if self._picam2 is not None:
            try:
                self._picam2.stop()
            except Exception:
                pass
            try:
                self._picam2.close()
            except Exception:
                pass
        self._picam2 = None
        self._imx500 = None
        self._active_model = None
        self._ai_active = False
        time.sleep(0.5)

    def _load_model(self, model_key: str) -> None:
        if self._active_model == model_key and self._picam2 is not None:
            return
        self._teardown_ai()
        if model_key not in MODELS:
            raise RuntimeError(f"unknown model {model_key!r}")
        name, filename, _, _ = MODELS[model_key]
        path = os.path.join(self._model_dir, filename)
        if not os.path.exists(path):
            raise RuntimeError(f"model not installed: {path}")
        from picamera2 import Picamera2
        from picamera2.devices.imx500 import IMX500

        log(f"loading {name}")
        self._imx500 = IMX500(path)
        self._picam2 = Picamera2(self._imx500.camera_num)
        cfg = self._picam2.create_preview_configuration(
            controls={"FrameRate": 30},
            buffer_count=12,
        )
        self._picam2.start(cfg)
        self._imx500.set_auto_aspect_ratio()
        time.sleep(5)
        self._active_model = model_key
        self._ai_active = True

    def _outputs(self, attempts: int = 20):
        for _ in range(attempts):
            metadata = self._picam2.capture_metadata()
            outputs = self._imx500.get_outputs(metadata)
            if outputs is not None:
                return outputs, self._imx500.get_kpi_info(metadata)
            time.sleep(0.2)
        return None, None

    def detect(self, confidence: float, model: str) -> dict:
        try:
            import numpy as np  # noqa: F401
        except Exception as e:
            return {"detections": [], "error": f"numpy not importable: {e}", "source": "helper"}
        try:
            self._load_model(model)
            outputs, kpi = self._outputs()
        except Exception as e:
            self._teardown_ai()
            return {"detections": [], "error": str(e), "source": "helper"}
        if outputs is None:
            self._teardown_ai()
            return {"detections": [], "error": "No inference output", "source": "helper"}
        boxes, scores, classes = outputs[0], outputs[1], outputs[2]
        mask = scores > confidence
        detections = []
        if mask.any():
            for box, score, cls in zip(boxes[mask], scores[mask], classes[mask].astype(int)):
                label = COCO_LABELS[cls] if cls < len(COCO_LABELS) else f"class_{cls}"
                detections.append({
                    "label": label,
                    "class_id": int(cls),
                    "confidence": round(float(score), 3),
                    "bbox": {
                        "y_min": round(float(box[0]), 4),
                        "x_min": round(float(box[1]), 4),
                        "y_max": round(float(box[2]), 4),
                        "x_max": round(float(box[3]), 4),
                    },
                })
        result = {
            "model": MODELS[model][0],
            "detections": detections,
            "count": len(detections),
            "source": "helper",
        }
        if kpi:
            result["performance"] = {
                "dnn_ms": round(kpi[0], 1),
                "dsp_ms": round(kpi[1], 1),
                "total_ms": round(kpi[0] + kpi[1], 1),
            }
        self._teardown_ai()
        return result

    def classify(self, top_k: int, model: str) -> dict:
        try:
            import numpy as np
        except Exception as e:
            return {"predictions": [], "error": f"numpy not importable: {e}", "source": "helper"}
        try:
            self._load_model(model)
            outputs, kpi = self._outputs()
        except Exception as e:
            self._teardown_ai()
            return {"predictions": [], "error": str(e), "source": "helper"}
        if outputs is None:
            self._teardown_ai()
            return {"predictions": [], "error": "No inference output", "source": "helper"}
        probs = outputs[0]
        if getattr(probs, "ndim", 1) > 1:
            probs = probs.flatten()
        top = np.argsort(probs)[-top_k:][::-1]
        predictions = []
        for idx in top:
            predictions.append({
                "label": f"class_{int(idx)}",
                "class_id": int(idx),
                "confidence": round(float(probs[idx]), 4),
            })
        result = {"model": MODELS[model][0], "predictions": predictions, "source": "helper"}
        if kpi:
            result["performance"] = {
                "dnn_ms": round(kpi[0], 1),
                "dsp_ms": round(kpi[1], 1),
                "total_ms": round(kpi[0] + kpi[1], 1),
            }
        self._teardown_ai()
        return result

    def pose(self, model: str) -> dict:
        try:
            import numpy as np
        except Exception as e:
            return {"poses": [], "error": f"numpy not importable: {e}", "source": "helper"}
        try:
            self._load_model(model)
            outputs, kpi = self._outputs()
        except Exception as e:
            self._teardown_ai()
            return {"poses": [], "error": str(e), "source": "helper"}
        if outputs is None:
            self._teardown_ai()
            return {"poses": [], "error": "No inference output", "source": "helper"}
        heatmaps = outputs[0]
        poses = []
        if getattr(heatmaps, "ndim", 0) == 3:
            h, w, num_kpts = heatmaps.shape
            keypoints = []
            for kpt_idx in range(min(num_kpts, 17)):
                heatmap = heatmaps[:, :, kpt_idx]
                max_val = float(heatmap.max())
                if max_val > -5.0:
                    peak = np.unravel_index(heatmap.argmax(), heatmap.shape)
                    name = POSE_KEYPOINTS[kpt_idx] if kpt_idx < len(POSE_KEYPOINTS) else f"kpt_{kpt_idx}"
                    keypoints.append({
                        "name": name,
                        "y": round(float(peak[0]) / h, 4),
                        "x": round(float(peak[1]) / w, 4),
                        "score": round(max_val, 3),
                    })
            if keypoints:
                good = [k for k in keypoints if k["score"] > -2.0]
                poses.append({
                    "keypoints": keypoints,
                    "keypoints_detected": len(good),
                    "total_keypoints": len(keypoints),
                })
        result = {
            "model": MODELS[model][0],
            "poses": poses,
            "people_detected": len(poses),
            "source": "helper",
        }
        if kpi:
            result["performance"] = {
                "dnn_ms": round(kpi[0], 1),
                "dsp_ms": round(kpi[1], 1),
                "total_ms": round(kpi[0] + kpi[1], 1),
            }
        self._teardown_ai()
        return result


def serve() -> int:
    wall = CameraWall()
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            emit({"error": True, "status": "bad_request", "reason": str(e), "source": "helper"})
            continue
        cmd = req.get("cmd")
        if cmd == "status":
            emit(wall.status())
        elif cmd == "capture_visual":
            emit(wall.capture("imx500"))
        elif cmd == "capture_night":
            emit(wall.capture("noir"))
        elif cmd == "models":
            emit(wall.models())
        elif cmd == "detect":
            emit(wall.detect(float(req.get("confidence", 0.3)), req.get("model", "efficientdet")))
        elif cmd == "classify":
            emit(wall.classify(int(req.get("top_k", 5)), req.get("model", "mobilenet_v2")))
        elif cmd == "pose":
            emit(wall.pose(req.get("model", "posenet")))
        elif cmd == "shutdown":
            wall._teardown_ai()
            emit({"ok": True, "status": "shutdown"})
            return 0
        else:
            emit({
                "error": True,
                "status": "bad_request",
                "reason": f"unknown cmd {cmd!r}",
                "source": "helper",
            })
    wall._teardown_ai()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="SensorHead-RS Picamera2 stdio wall")
    parser.add_argument(
        "--doctor",
        action="store_true",
        help="Check picamera2/libcamera imports; do not open CSI",
    )
    args = parser.parse_args()
    if args.doctor:
        report = doctor()
        emit(report)
        return 0 if report["ok"] else 1
    return serve()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except BrokenPipeError:
        raise SystemExit(0)
