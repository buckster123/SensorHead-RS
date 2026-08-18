#!/usr/bin/env python3
"""Thin BSEC2 wall — system Python + operator egg, no venv.

Stdin/stdout is newline JSON (one request, one reply). Logs go to stderr.
This process is exclusive on the BME688: do not run it beside a dashboard
that also inits BSEC.

Commands: read | save_state | status | shutdown
Doctor:   python3 walls/bsec.py --doctor
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path

# stdout is the protocol. Nothing else goes there.
def emit(obj: dict) -> None:
    sys.stdout.write(json.dumps(obj, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def log(msg: str) -> None:
    sys.stderr.write(f"sensorhead-bsec: {msg}\n")
    sys.stderr.flush()


def parse_addr(raw: str) -> int:
    raw = raw.strip().lower()
    if raw.startswith("0x"):
        return int(raw, 16)
    return int(raw, 10)


def egg_from_env() -> str | None:
    raw = os.environ.get("SENSORHEAD_BME68X_EGG", "").strip()
    return raw or None


def install_egg(egg: str | None) -> None:
    if egg:
        sys.path.insert(0, egg)


IAQ_BANDS = [
    (0, 50, "excellent", "Clean air — fresh and pleasant"),
    (51, 100, "good", "Acceptable air quality"),
    (101, 150, "lightly_polluted", "Sensitive people may notice effects"),
    (151, 200, "moderately_polluted", "Increased discomfort likely"),
    (201, 250, "heavily_polluted", "Significant health effects possible"),
    (251, 350, "severely_polluted", "Health warnings — reduce exposure"),
    (351, 500, "extremely_polluted", "Emergency conditions"),
]

IAQ_ACCURACY_LABELS = [
    "stabilizing",
    "uncertain",
    "calibrating",
    "calibrated",
]

HINT = (
    "Place the pi3g bme68x egg on SENSORHEAD_BME68X_EGG and use "
    "/usr/bin/python3. Do not vendor the blob. Do not pip install a "
    "random wheel. See docs/bsec-sdk.md."
)


def iaq_band(iaq: float) -> tuple[str, str]:
    for lo, hi, label, desc in IAQ_BANDS:
        if lo <= iaq <= hi:
            return label, desc
    return "unknown", "Out of range"


def data_dir() -> Path:
    override = os.environ.get("SENSORHEAD_DATA_DIR", "").strip()
    if override:
        return Path(override)
    return Path.cwd() / "data"


def doctor() -> dict:
    egg = egg_from_env()
    install_egg(egg)
    report: dict = {
        "ok": False,
        "helper": "bsec",
        "python": sys.executable,
        "bme68x": False,
        "bsec_constants": False,
        "egg": egg,
        "reason": None,
        "hint": HINT,
    }
    try:
        import bme68x  # noqa: F401

        report["bme68x"] = True
    except Exception as e:
        report["reason"] = str(e)
        return report
    try:
        import bsecConstants  # noqa: F401

        report["bsec_constants"] = True
    except Exception as e:
        report["reason"] = str(e)
        return report
    report["ok"] = True
    report["reason"] = None
    return report


class BsecNose:
    """Long-lived BME688 + BSEC2 handle. Same state file as the original."""

    def __init__(self) -> None:
        self._sensor = None
        self._bsec_active = False
        self._available = False
        self._init_error: str | None = None
        self._last_bsec_data: dict | None = None
        self._last_state_save = 0.0
        self._last_save_error: str | None = None
        self._state_file = data_dir() / "bsec_state.json"
        self._addr = parse_addr(os.environ.get("SENSORHEAD_BME688_ADDR", "0x77"))
        self._save_interval = int(os.environ.get("SENSORHEAD_BSEC_SAVE_INTERVAL", "300"))

    def _init(self) -> None:
        if self._sensor is not None or self._init_error:
            return
        egg = egg_from_env()
        install_egg(egg)
        try:
            from bme68x import BME68X
            import bsecConstants as bsec
        except Exception as e:
            self._init_error = f"bme68x not importable: {e}"
            log(self._init_error)
            return
        try:
            self._sensor = BME68X(self._addr, 1)
            self._load_state()
            self._sensor.set_sample_rate(bsec.BSEC_SAMPLE_RATE_LP)
            self._bsec_active = True
            self._available = True
            log(
                f"BME688 BSEC {self._sensor.get_bsec_version()} "
                f"addr=0x{self._addr:02X} state={self._state_file}"
            )
        except Exception as e:
            self._sensor = None
            self._init_error = f"BSEC init failed: {e}"
            log(self._init_error)

    def _load_state(self) -> None:
        if not self._state_file.exists():
            log("No saved BSEC state — starting fresh calibration")
            return
        try:
            saved = json.loads(self._state_file.read_text())
            state_list = saved.get("state")
            if state_list and isinstance(state_list, list):
                self._sensor.set_bsec_state(state_list)
                log(
                    f"Restored BSEC state ({len(state_list)} bytes, "
                    f"accuracy was {saved.get('iaq_accuracy', '?')})"
                )
        except Exception as e:
            log(f"Failed to restore BSEC state: {e}")

    def save_state(self) -> dict:
        self._init()
        self._last_save_error = None
        if not self._bsec_active or self._sensor is None:
            reason = self._init_error or "BSEC not active"
            self._last_save_error = reason
            return {"status": "skipped", "reason": reason}
        try:
            state = self._sensor.get_bsec_state()
            if not state:
                self._last_save_error = "get_bsec_state returned empty"
                return {"status": "skipped", "reason": self._last_save_error}
            self._state_file.parent.mkdir(parents=True, exist_ok=True)
            payload = {
                "state": state,
                "saved_at": time.time(),
                "iaq_accuracy": (
                    self._last_bsec_data.get("iaq_accuracy", 0)
                    if self._last_bsec_data
                    else 0
                ),
                "bsec_version": self._sensor.get_bsec_version(),
            }
            tmp = self._state_file.with_suffix(".tmp")
            tmp.write_text(json.dumps(payload))
            tmp.rename(self._state_file)
            self._last_state_save = time.time()
            log(f"Saved BSEC state → {self._state_file}")
            return {"status": "saved", "file": str(self._state_file)}
        except Exception as e:
            self._last_save_error = str(e)
            log(f"Failed to save BSEC state: {e}")
            return {"status": "skipped", "reason": str(e)}

    def _maybe_save(self) -> None:
        if not self._bsec_active:
            return
        if time.time() - self._last_state_save >= self._save_interval:
            self.save_state()

    def status(self) -> dict:
        self._init()
        out = {
            "sensor": "BME688",
            "available": self._available,
            "bsec_active": self._bsec_active,
            "source": "helper",
            "state_file": str(self._state_file),
        }
        if self._init_error:
            out["error"] = True
            out["reason"] = self._init_error
        if self._available and self._sensor is not None and self._bsec_active:
            out["bsec_version"] = self._sensor.get_bsec_version()
            out["variant"] = self._sensor.get_variant()
            out["chip_id"] = f"0x{self._sensor.get_chip_id():02X}"
            out["state_saved"] = self._state_file.exists()
        return out

    def read(self) -> dict:
        self._init()
        if self._init_error:
            return {
                "error": True,
                "sensor": "BME688",
                "status": "unavailable",
                "reason": self._init_error,
                "source": "helper",
            }
        if not self._available or self._sensor is None:
            return {
                "error": True,
                "sensor": "BME688",
                "status": "unavailable",
                "reason": "Sensor not detected on I2C bus",
                "source": "helper",
            }

        data = None
        for _ in range(40):
            try:
                sample = self._sensor.get_bsec_data()
            except Exception as e:
                return {
                    "error": True,
                    "sensor": "BME688",
                    "status": "read_error",
                    "reason": str(e),
                    "source": "helper",
                }
            if sample:
                data = sample
                stale = False
                break
            time.sleep(0.1)
        else:
            if self._last_bsec_data is not None:
                data = self._last_bsec_data
                stale = True
            else:
                return {
                    "error": True,
                    "sensor": "BME688",
                    "status": "warming_up",
                    "reason": "BSEC2 still stabilizing — data not yet available",
                    "bsec_version": self._sensor.get_bsec_version(),
                    "source": "helper",
                }

        self._last_bsec_data = data
        iaq = data.get("iaq", 0)
        iaq_acc = data.get("iaq_accuracy", 0)
        quality, desc = iaq_band(iaq)
        self._maybe_save()
        return {
            "temperature_c": round(data.get("temperature", 0), 2),
            "humidity_pct": round(data.get("humidity", 0), 2),
            "pressure_hpa": round(data.get("raw_pressure", 0) / 100, 2),
            "iaq": round(iaq, 1),
            "iaq_accuracy": iaq_acc,
            "iaq_accuracy_label": IAQ_ACCURACY_LABELS[min(iaq_acc, 3)],
            "air_quality": quality,
            "air_quality_description": desc,
            "co2_equivalent_ppm": round(data.get("co2_equivalent", 0), 1),
            "co2_accuracy": data.get("co2_accuracy", 0),
            "breath_voc_ppm": round(data.get("breath_voc_equivalent", 0), 4),
            "breath_voc_accuracy": data.get("breath_voc_accuracy", 0),
            "gas_percentage": round(data.get("gas_percentage", 0), 2),
            "gas_percentage_accuracy": data.get("gas_percentage_accuracy", 0),
            "raw_temperature_c": round(data.get("raw_temperature", 0), 2),
            "raw_humidity_pct": round(data.get("raw_humidity", 0), 2),
            "raw_gas_resistance_ohm": round(data.get("raw_gas", 0), 1),
            "stabilization_status": data.get("stabilization_status", 0),
            "run_in_status": data.get("run_in_status", 0),
            "bsec_version": self._sensor.get_bsec_version(),
            "stale": stale,
            "timestamp": time.time(),
            "source": "helper",
        }


def serve() -> int:
    nose = BsecNose()
    for raw in sys.stdin:
        line = raw.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
        except json.JSONDecodeError as e:
            emit(
                {
                    "error": True,
                    "sensor": "BME688",
                    "status": "bad_request",
                    "reason": str(e),
                    "source": "helper",
                }
            )
            continue
        cmd = req.get("cmd")
        if cmd == "read":
            emit(nose.read())
        elif cmd == "save_state":
            emit(nose.save_state())
        elif cmd == "status":
            emit(nose.status())
        elif cmd == "shutdown":
            emit(nose.save_state())
            return 0
        else:
            emit(
                {
                    "error": True,
                    "sensor": "BME688",
                    "status": "bad_request",
                    "reason": f"unknown cmd {cmd!r}",
                    "source": "helper",
                }
            )
    nose.save_state()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="SensorHead-RS BSEC stdio wall")
    parser.add_argument(
        "--doctor",
        action="store_true",
        help="Check that bme68x imports; do not open I2C",
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
