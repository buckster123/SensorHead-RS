//! Stdio client for `walls/bsec.py`.
//!
//! One request line, one reply line. The helper owns BSEC; this process
//! never imports Python and never vendors the egg.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use sensorhead::EnvironmentBody;
use serde::Deserialize;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const RPC_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone)]
pub struct BsecHelperConfig {
    pub python: PathBuf,
    pub script: PathBuf,
    pub egg: Option<PathBuf>,
    pub data_dir: PathBuf,
    pub addr: u8,
}

impl Default for BsecHelperConfig {
    fn default() -> Self {
        Self {
            python: PathBuf::from("/usr/bin/python3"),
            script: PathBuf::from("walls/bsec.py"),
            egg: None,
            data_dir: PathBuf::from("data"),
            addr: 0x77,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub python: String,
    pub script: String,
    pub python_ok: bool,
    pub script_ok: bool,
    pub bme68x: bool,
    pub egg: Option<String>,
    pub reason: Option<String>,
    pub hint: String,
}

#[derive(Debug, Deserialize)]
struct HelperDoctor {
    ok: bool,
    #[serde(default)]
    bme68x: bool,
    #[serde(default)]
    egg: Option<String>,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    hint: Option<String>,
}

pub struct BsecWall {
    cfg: BsecHelperConfig,
    live: Option<Live>,
}

struct Live {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl BsecWall {
    pub fn new(cfg: BsecHelperConfig) -> Self {
        Self { cfg, live: None }
    }

    pub async fn read(&mut self) -> EnvironmentBody {
        match self.rpc("read").await {
            Ok(v) => EnvironmentBody::parse(v.to_string().as_bytes())
                .unwrap_or_else(|e| EnvironmentBody::unavailable(e.to_string())),
            Err(reason) => EnvironmentBody::unavailable(reason),
        }
    }

    pub async fn save_state(&mut self) -> Value {
        match self.rpc("save_state").await {
            Ok(v) => v,
            Err(reason) => serde_json::json!({"status": "skipped", "reason": reason}),
        }
    }

    pub async fn status(&mut self) -> Value {
        match self.rpc("status").await {
            Ok(v) => v,
            Err(reason) => serde_json::json!({
                "error": true,
                "sensor": "BME688",
                "status": "unavailable",
                "reason": reason,
                "source": "helper",
            }),
        }
    }

    async fn rpc(&mut self, cmd: &str) -> Result<Value, String> {
        self.ensure().await?;
        let live = self.live.as_mut().ok_or("bsec helper not started")?;
        let line = format!("{{\"cmd\":\"{cmd}\"}}\n");
        live.stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|e| format!("bsec helper stdin: {e}"))?;
        live.stdin
            .flush()
            .await
            .map_err(|e| format!("bsec helper flush: {e}"))?;
        let mut buf = String::new();
        let read = live.stdout.read_line(&mut buf);
        match tokio::time::timeout(RPC_TIMEOUT, read).await {
            Ok(Ok(0)) => {
                self.live = None;
                Err("bsec helper closed stdout".into())
            }
            Ok(Ok(_)) => serde_json::from_str(buf.trim())
                .map_err(|e| format!("bsec helper JSON: {e}: {buf}")),
            Ok(Err(e)) => {
                self.live = None;
                Err(format!("bsec helper stdout: {e}"))
            }
            Err(_) => {
                self.reap().await;
                Err("bsec helper timed out".into())
            }
        }
    }

    async fn ensure(&mut self) -> Result<(), String> {
        if let Some(live) = &mut self.live {
            match live.child.try_wait() {
                Ok(Some(status)) => {
                    self.live = None;
                    return Err(format!("bsec helper exited: {status}"));
                }
                Ok(None) => return Ok(()),
                Err(e) => {
                    self.live = None;
                    return Err(format!("bsec helper wait: {e}"));
                }
            }
        }
        self.spawn().await
    }

    async fn spawn(&mut self) -> Result<(), String> {
        if !self.cfg.script.is_file() {
            return Err(format!(
                "bsec helper script missing: {}",
                self.cfg.script.display()
            ));
        }
        let mut cmd = Command::new(&self.cfg.python);
        cmd.arg(&self.cfg.script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .env("SENSORHEAD_DATA_DIR", &self.cfg.data_dir)
            .env("SENSORHEAD_BME688_ADDR", format!("0x{:02x}", self.cfg.addr))
            .kill_on_drop(true);
        if let Some(egg) = &self.cfg.egg {
            cmd.env("SENSORHEAD_BME68X_EGG", egg);
        }
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
            .ok_or_else(|| "bsec helper stdin not piped".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "bsec helper stdout not piped".to_string())?;
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

impl Drop for BsecWall {
    fn drop(&mut self) {
        if let Some(live) = self.live.as_mut() {
            let _ = live.child.start_kill();
        }
    }
}

pub async fn doctor(cfg: &BsecHelperConfig) -> DoctorReport {
    const HINT: &str = "Place the pi3g bme68x egg on SENSORHEAD_BME68X_EGG and use /usr/bin/python3. Do not vendor the blob. See docs/bsec-sdk.md.";
    let mut report = DoctorReport {
        ok: false,
        python: cfg.python.display().to_string(),
        script: cfg.script.display().to_string(),
        python_ok: python_exists(&cfg.python),
        script_ok: cfg.script.is_file(),
        bme68x: false,
        egg: cfg.egg.as_ref().map(|p| p.display().to_string()),
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
    if let Some(egg) = &cfg.egg {
        cmd.env("SENSORHEAD_BME68X_EGG", egg);
    }
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
    let parsed: Result<HelperDoctor, _> = serde_json::from_slice(&out.stdout);
    match parsed {
        Ok(h) => {
            report.ok = h.ok;
            report.bme68x = h.bme68x;
            if h.egg.is_some() {
                report.egg = h.egg;
            }
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

    fn repo_helper() -> BsecHelperConfig {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../walls/bsec.py");
        BsecHelperConfig {
            python: PathBuf::from("python3"),
            script,
            ..BsecHelperConfig::default()
        }
    }

    #[tokio::test]
    async fn doctor_without_egg_is_honest() {
        let report = doctor(&repo_helper()).await;
        assert!(report.script_ok, "{}", report.script);
        assert!(
            report.python_ok,
            "CI/laptop should have python3: {}",
            report.reason.as_deref().unwrap_or("?")
        );
        assert!(!report.ok);
        assert!(!report.bme68x);
        let reason = report.reason.unwrap_or_default();
        assert!(
            reason.contains("bme68x") || reason.contains("No module"),
            "{reason}"
        );
    }

    #[tokio::test]
    async fn helper_read_without_egg_does_not_invent_iaq() {
        let mut wall = BsecWall::new(repo_helper());
        let body = wall.read().await;
        assert!(body.error);
        assert_eq!(body.status.as_deref(), Some("unavailable"));
        assert!(body.iaq.is_none());
        assert!(!body.apexos_air_quality_possible());
        let reason = body.reason.unwrap_or_default();
        assert!(
            reason.contains("bme68x") || reason.contains("not importable"),
            "{reason}"
        );
    }
}
