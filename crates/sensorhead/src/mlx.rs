//! Native MLX90640 I2C reader.
//!
//! This is not a vendor wall — the sensor is a documented I2C device at 0x33.
//! Default HTTP mode stays `upstream` so a sidecar with `PrivateDevices=true`
//! never opens the bus while Python still owns it.

use crate::thermal::{ThermalBody, THERMAL_PIXELS};

/// Where `/api/thermal/*` gets its 32×24 frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalSource {
    /// Proxy the Python dashboard (default). Does not open I2C.
    Upstream,
    /// Open `/dev/i2c-{bus}` ourselves. Exclusive — do not combine with a
    /// live Python thermal owner on the same bus.
    Native { bus: u8, addr: u8 },
}

impl ThermalSource {
    pub fn parse(mode: &str, bus: u8, addr: u8) -> std::result::Result<Self, String> {
        match mode.trim().to_ascii_lowercase().as_str() {
            "" | "upstream" => Ok(Self::Upstream),
            "native" => Ok(Self::Native { bus, addr }),
            other => Err(format!(
                "unknown SENSORHEAD_THERMAL {other:?}; expected upstream or native"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Upstream => "upstream",
            Self::Native { .. } => "native",
        }
    }
}

/// Lazy handle. Created on first native read so `AppState::new` never opens I2C.
#[derive(Default)]
pub struct NativeMlx {
    inner: Option<Inner>,
}

enum Inner {
    #[cfg(target_os = "linux")]
    Live(LinuxCam),
}

impl NativeMlx {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn read(&mut self, bus: u8, addr: u8) -> ThermalBody {
        match self.read_frame(bus, addr) {
            Ok((frame, ms)) => ThermalBody::from_celsius_frame(frame, ms)
                .unwrap_or_else(|e| ThermalBody::unavailable(e.to_string())),
            Err(reason) => ThermalBody::unavailable(reason),
        }
    }

    fn read_frame(&mut self, bus: u8, addr: u8) -> std::result::Result<(Vec<f32>, f32), String> {
        let t0 = std::time::Instant::now();
        let frame = self.fill(bus, addr)?;
        if frame.len() != THERMAL_PIXELS {
            return Err(format!(
                "native MLX90640 frame length {}, expected {THERMAL_PIXELS}",
                frame.len()
            ));
        }
        Ok((frame, t0.elapsed().as_secs_f32() * 1000.0))
    }

    fn fill(&mut self, bus: u8, addr: u8) -> std::result::Result<Vec<f32>, String> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (bus, addr);
            Err("native MLX90640 is Linux-only".into())
        }
        #[cfg(target_os = "linux")]
        {
            if self.inner.is_none() {
                self.inner = Some(Inner::Live(LinuxCam::open(bus, addr)?));
            }
            match self.inner.as_mut() {
                Some(Inner::Live(cam)) => cam.read_frame(),
                None => Err("native MLX90640 failed to open".into()),
            }
        }
    }
}

#[cfg(target_os = "linux")]
struct LinuxCam {
    camera: mlx9064x::Mlx90640Driver<linux_embedded_hal::I2cdev>,
    warmed: bool,
}

#[cfg(target_os = "linux")]
impl LinuxCam {
    fn open(bus: u8, addr: u8) -> std::result::Result<Self, String> {
        let path = format!("/dev/i2c-{bus}");
        let i2c =
            linux_embedded_hal::I2cdev::new(&path).map_err(|e| format!("open {path}: {e}"))?;
        let mut camera = mlx9064x::Mlx90640Driver::new(i2c, addr)
            .map_err(|e| format!("MLX90640@{addr:#04x} init: {e}"))?;
        if let Err(e) = camera.set_frame_rate(mlx9064x::FrameRate::Four) {
            tracing_warn_or_stderr(&format!(
                "MLX90640 set_frame_rate(4 Hz) failed ({e}); leaving camera default"
            ));
        }
        Ok(Self {
            camera,
            warmed: false,
        })
    }

    fn read_frame(&mut self) -> std::result::Result<Vec<f32>, String> {
        let mut buf = vec![0f32; THERMAL_PIXELS];
        if !self.warmed {
            // Same hygiene as the Python original: discard the first two full frames.
            for _ in 0..2 {
                self.fill_both_subpages(&mut buf)?;
            }
            self.warmed = true;
        }
        self.fill_both_subpages(&mut buf)?;
        Ok(buf)
    }

    fn fill_both_subpages(&mut self, dest: &mut [f32]) -> std::result::Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        let mut got = 0u8;
        while std::time::Instant::now() < deadline {
            match self.camera.generate_image_if_ready(dest) {
                Ok(true) => {
                    got += 1;
                    if got >= 2 {
                        return Ok(());
                    }
                }
                Ok(false) => std::thread::sleep(std::time::Duration::from_millis(40)),
                Err(e) => return Err(format!("MLX90640 read: {e}")),
            }
        }
        Err("MLX90640 timed out waiting for both subpages".into())
    }
}

#[cfg(target_os = "linux")]
fn tracing_warn_or_stderr(msg: &str) {
    // sensorhead is the core lib and does not depend on tracing. The API face logs.
    eprintln!("sensorhead: {msg}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_is_upstream() {
        assert_eq!(
            ThermalSource::parse("", 1, 0x33).unwrap(),
            ThermalSource::Upstream
        );
        assert_eq!(
            ThermalSource::parse("upstream", 1, 0x33).unwrap(),
            ThermalSource::Upstream
        );
    }

    #[test]
    fn parse_native_keeps_bus_and_addr() {
        assert_eq!(
            ThermalSource::parse("native", 1, 0x33).unwrap(),
            ThermalSource::Native { bus: 1, addr: 0x33 }
        );
    }

    #[test]
    fn parse_rejects_typos_instead_of_silently_proxying() {
        let err = ThermalSource::parse("nativ", 1, 0x33).unwrap_err();
        assert!(err.contains("nativ"), "{err}");
        assert!(err.contains("upstream or native"), "{err}");
    }
}
