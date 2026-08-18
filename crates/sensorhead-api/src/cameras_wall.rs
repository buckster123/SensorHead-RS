//! Stdio client for `walls/cameras.py`.
//!
//! JPEG replies are `jpeg_b64` on a JSON line. This process never imports
//! Picamera2 and never `pip install`s it.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const STATUS_TIMEOUT: Duration = Duration::from_secs(8);
const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15);
const INFER_TIMEOUT: Duration = Duration::from_secs(25);

#[derive(Debug, Clone)]
pub struct CameraHelperConfig {
    pub python: PathBuf,
    pub script: PathBuf,
    pub model_dir: PathBuf,
}

impl Default for CameraHelperConfig {
    fn default() -> Self {
        Self {
            python: PathBuf::from("/usr/bin/python3"),
            script: PathBuf::from("walls/cameras.py"),
            model_dir: PathBuf::from("/usr/share/imx500-models"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CameraDoctorReport {
    pub ok: bool,
    pub python: String,
    pub script: String,
    pub python_ok: bool,
    pub script_ok: bool,
    pub picamera2: bool,
    pub libcamera: bool,
    pub reason: Option<String>,
    pub hint: String,
}

#[derive(Debug, Deserialize)]
struct HelperDoctor {
    ok: bool,
    #[serde(default)]
    picamera2: bool,
    #[serde(default)]
    libcamera: bool,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    hint: Option<String>,
}

pub struct CameraWall {
    cfg: CameraHelperConfig,
    live: Option<Live>,
}

struct Live {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl CameraWall {
    pub fn new(cfg: CameraHelperConfig) -> Self {
        Self { cfg, live: None }
    }

    pub async fn status(&mut self) -> Value {
        self.rpc(json!({"cmd": "status"}), STATUS_TIMEOUT)
            .await
            .unwrap_or_else(|reason| json!({"error": true, "reason": reason, "source": "helper"}))
    }

    pub async fn models(&mut self) -> Value {
        self.rpc(json!({"cmd": "models"}), STATUS_TIMEOUT)
            .await
            .unwrap_or_else(|reason| json!({"error": true, "reason": reason}))
    }

    pub async fn capture(&mut self, which: &str) -> Value {
        let cmd = match which {
            "night" | "noir" => "capture_night",
            _ => "capture_visual",
        };
        self.rpc(json!({"cmd": cmd}), CAPTURE_TIMEOUT)
            .await
            .unwrap_or_else(|reason| {
                json!({"error": true, "status": "unavailable", "reason": reason, "source": "helper"})
            })
    }

    pub async fn detect(&mut self, confidence: f64) -> Value {
        self.rpc(
            json!({"cmd": "detect", "confidence": confidence}),
            INFER_TIMEOUT,
        )
        .await
        .unwrap_or_else(|reason| json!({"detections": [], "error": reason, "source": "helper"}))
    }

    pub async fn classify(&mut self, top_k: u32) -> Value {
        self.rpc(json!({"cmd": "classify", "top_k": top_k}), INFER_TIMEOUT)
            .await
            .unwrap_or_else(
                |reason| json!({"predictions": [], "error": reason, "source": "helper"}),
            )
    }

    pub async fn pose(&mut self) -> Value {
        self.rpc(json!({"cmd": "pose"}), INFER_TIMEOUT)
            .await
            .unwrap_or_else(|reason| json!({"poses": [], "error": reason, "source": "helper"}))
    }

    async fn rpc(&mut self, req: Value, timeout: Duration) -> Result<Value, String> {
        self.ensure().await?;
        let live = self.live.as_mut().ok_or("cameras helper not started")?;
        let mut line = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        line.push('\n');
        live.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("cameras helper stdin: {e}"))?;
        live.stdin
            .flush()
            .await
            .map_err(|e| format!("cameras helper flush: {e}"))?;
        let mut buf = String::new();
        let read = live.stdout.read_line(&mut buf);
        match tokio::time::timeout(timeout, read).await {
            Ok(Ok(0)) => {
                self.live = None;
                Err("cameras helper closed stdout".into())
            }
            Ok(Ok(_)) => {
                serde_json::from_str(buf.trim()).map_err(|e| format!("cameras helper JSON: {e}"))
            }
            Ok(Err(e)) => {
                self.live = None;
                Err(format!("cameras helper stdout: {e}"))
            }
            Err(_) => {
                self.reap().await;
                Err("cameras helper timed out".into())
            }
        }
    }

    async fn ensure(&mut self) -> Result<(), String> {
        if let Some(live) = &mut self.live {
            match live.child.try_wait() {
                Ok(Some(status)) => {
                    self.live = None;
                    return Err(format!("cameras helper exited: {status}"));
                }
                Ok(None) => return Ok(()),
                Err(e) => {
                    self.live = None;
                    return Err(format!("cameras helper wait: {e}"));
                }
            }
        }
        self.spawn().await
    }

    async fn spawn(&mut self) -> Result<(), String> {
        if !self.cfg.script.is_file() {
            return Err(format!(
                "cameras helper script missing: {}",
                self.cfg.script.display()
            ));
        }
        let mut cmd = Command::new(&self.cfg.python);
        cmd.arg(&self.cfg.script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .env("SENSORHEAD_MODEL_DIR", &self.cfg.model_dir)
            .kill_on_drop(true);
        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "spawn {} {}: {e}",
                self.cfg.python.display(),
                self.cfg.script.display()
            )
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "cameras helper stdin not piped".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "cameras helper stdout not piped".to_string())?;
        self.live = Some(Live {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        });
        Ok(())
    }

    async fn reap(&mut self) {
        if let Some(mut live) = self.live.take() {
            let _ = live.child.kill().await;
        }
    }
}

impl Drop for CameraWall {
    fn drop(&mut self) {
        if let Some(live) = self.live.as_mut() {
            let _ = live.child.start_kill();
        }
    }
}

pub fn jpeg_from_helper(body: &Value) -> Result<Vec<u8>, String> {
    let b64 = body
        .get("jpeg_b64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "cameras helper JPEG missing jpeg_b64".to_string())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("cameras helper JPEG base64: {e}"))?;
    if bytes.len() < 2 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return Err("cameras helper returned non-JPEG bytes".into());
    }
    Ok(bytes)
}

pub async fn doctor_cameras(cfg: &CameraHelperConfig) -> CameraDoctorReport {
    const HINT: &str = "Install apt python3-picamera2 python3-libcamera. Do not pip install picamera2. See docs/picamera2.md.";
    let mut report = CameraDoctorReport {
        ok: false,
        python: cfg.python.display().to_string(),
        script: cfg.script.display().to_string(),
        python_ok: python_exists(&cfg.python),
        script_ok: cfg.script.is_file(),
        picamera2: false,
        libcamera: false,
        reason: None,
        hint: HINT.into(),
    };
    if !report.python_ok {
        report.reason = Some(format!("python not found: {}", cfg.python.display()));
        return report;
    }
    if !report.script_ok {
        report.reason = Some(format!("helper script missing: {}", cfg.script.display()));
        return report;
    }
    let mut cmd = Command::new(&cfg.python);
    cmd.arg(&cfg.script)
        .arg("--doctor")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = match tokio::time::timeout(Duration::from_secs(5), cmd.output()).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => {
            report.reason = Some(format!("doctor spawn: {e}"));
            return report;
        }
        Err(_) => {
            report.reason = Some("doctor timed out".into());
            return report;
        }
    };
    match serde_json::from_slice::<HelperDoctor>(&out.stdout) {
        Ok(h) => {
            report.ok = h.ok;
            report.picamera2 = h.picamera2;
            report.libcamera = h.libcamera;
            report.reason = h.reason;
            if let Some(hint) = h.hint {
                report.hint = hint;
            }
        }
        Err(e) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            report.reason = Some(format!("doctor JSON: {e}; stderr={stderr}"));
        }
    }
    report
}

fn python_exists(path: &Path) -> bool {
    path.is_file() || which_on_path(path)
}

fn which_on_path(path: &Path) -> bool {
    if path.components().count() != 1 {
        return false;
    }
    let name = path.as_os_str();
    let Some(dirs) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&dirs) {
        if dir.join(name).is_file() {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_helper() -> CameraHelperConfig {
        CameraHelperConfig {
            python: PathBuf::from("python3"),
            script: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../walls/cameras.py"),
            ..CameraHelperConfig::default()
        }
    }

    #[tokio::test]
    async fn doctor_without_picamera2_is_honest() {
        let report = doctor_cameras(&repo_helper()).await;
        assert!(report.script_ok, "{}", report.script);
        assert!(report.python_ok);
        assert!(!report.ok);
        assert!(!report.picamera2);
        let reason = report.reason.unwrap_or_default();
        assert!(
            reason.contains("picamera2") || reason.contains("No module"),
            "{reason}"
        );
    }

    #[tokio::test]
    async fn helper_capture_without_picamera2_does_not_invent_a_jpeg() {
        let mut wall = CameraWall::new(repo_helper());
        let body = wall.capture("visual").await;
        assert_eq!(body["error"], true);
        assert!(body.get("jpeg_b64").is_none());
        let reason = body["reason"].as_str().unwrap_or_default();
        assert!(
            reason.contains("picamera2") || reason.contains("not importable"),
            "{reason}"
        );
    }
}
