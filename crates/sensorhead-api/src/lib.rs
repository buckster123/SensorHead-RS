//! HTTP face over the SensorHead contract.
//!
//! Hardware routes proxy to the live Python dashboard when
//! `SENSORHEAD_UPSTREAM` is set (Picamera2 and, by default, BSEC stay there).
//! `SENSORHEAD_IAQ=helper` reads BSEC via `walls/bsec.py` on system Python.
//! `/api/thermal/heatmap` is rendered in Rust from `/api/thermal/data`.
//! Thermal frames come from upstream unless `SENSORHEAD_THERMAL=native`.

mod bsec_wall;

use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use sensorhead::{
    compose_status, render_heatmap, EnvironmentBody, IaqSource, NativeMlx, ThermalBody,
    ThermalSource, UpstreamView, HEATMAP_HEIGHT, HEATMAP_WIDTH,
};
use serde::Deserialize;

pub use bsec_wall::{doctor, BsecHelperConfig, DoctorReport};

pub struct AppState {
    pub upstream: Option<String>,
    pub thermal: ThermalSource,
    pub iaq: IaqSource,
    pub mlx: Mutex<NativeMlx>,
    pub bsec: tokio::sync::Mutex<bsec_wall::BsecWall>,
    pub client: reqwest::Client,
}

impl AppState {
    pub fn new(upstream: Option<String>) -> anyhow::Result<Self> {
        Self::with_thermal(upstream, ThermalSource::Upstream)
    }

    pub fn with_thermal(upstream: Option<String>, thermal: ThermalSource) -> anyhow::Result<Self> {
        Self::with_sources(
            upstream,
            thermal,
            IaqSource::Upstream,
            BsecHelperConfig::default(),
        )
    }

    pub fn with_sources(
        upstream: Option<String>,
        thermal: ThermalSource,
        iaq: IaqSource,
        bsec: BsecHelperConfig,
    ) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()?;
        Ok(Self {
            upstream: upstream.filter(|s| !s.is_empty()),
            thermal,
            iaq,
            mlx: Mutex::new(NativeMlx::new()),
            bsec: tokio::sync::Mutex::new(bsec_wall::BsecWall::new(bsec)),
            client,
        })
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/environment", get(environment))
        .route("/api/environment/save-state", get(save_state))
        .route("/api/thermal/data", get(thermal_data))
        .route("/api/thermal/heatmap", get(thermal_heatmap))
        .route("/api/status", get(status))
        .route("/api/models", get(proxy_named))
        .route("/api/capture/visual", get(proxy_named))
        .route("/api/capture/night", get(proxy_named))
        .route("/api/detect", get(proxy_named))
        .route("/api/classify", get(proxy_named))
        .route("/api/pose", get(proxy_named))
        .fallback(proxy_fallback)
        .with_state(Arc::new(state))
}

pub fn git_sha() -> &'static str {
    option_env!("SENSORHEAD_GIT_SHA").unwrap_or("unknown")
}

fn now_secs() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

async fn health(State(st): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "ok": true,
        "service": "sensorhead-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "git_sha": git_sha(),
        "upstream": st.upstream,
        "thermal": st.thermal.as_str(),
        "iaq": st.iaq.as_str(),
    }))
}

enum StatusLoad {
    Missing,
    Failed(String),
    Body(serde_json::Value),
}

async fn load_upstream_status(st: &AppState) -> StatusLoad {
    if st.upstream.is_none() {
        return StatusLoad::Missing;
    }
    match fetch_upstream(st, "/api/status").await {
        Ok(resp) => match resp.bytes().await {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(body) => StatusLoad::Body(body),
                Err(e) => StatusLoad::Failed(e.to_string()),
            },
            Err(e) => StatusLoad::Failed(e.to_string()),
        },
        Err(reason) => StatusLoad::Failed(reason),
    }
}

async fn status(State(st): State<Arc<AppState>>) -> impl IntoResponse {
    let url = st.upstream.as_deref();
    let load = load_upstream_status(&st).await;
    let view = match &load {
        StatusLoad::Missing => UpstreamView::Missing,
        StatusLoad::Failed(reason) => UpstreamView::Failed(reason),
        StatusLoad::Body(body) => UpstreamView::Body(body),
    };
    let mut body = compose_status(now_secs(), git_sha(), url, view);
    if st.iaq == IaqSource::Helper {
        let env = {
            let mut wall = st.bsec.lock().await;
            wall.read().await
        };
        if let Ok(v) = serde_json::to_value(env) {
            body["environment"] = v;
        }
    }
    Json(body)
}

async fn environment(State(st): State<Arc<AppState>>) -> Response {
    match st.iaq {
        IaqSource::Helper => {
            let body = {
                let mut wall = st.bsec.lock().await;
                wall.read().await
            };
            Json(body).into_response()
        }
        IaqSource::Upstream => match fetch_upstream(&st, "/api/environment").await {
            Ok(resp) => forward(resp).await,
            Err(reason) => Json(EnvironmentBody::unavailable(reason)).into_response(),
        },
    }
}

async fn save_state(State(st): State<Arc<AppState>>, uri: Uri) -> Response {
    match st.iaq {
        IaqSource::Helper => {
            let body = {
                let mut wall = st.bsec.lock().await;
                wall.save_state().await
            };
            Json(body).into_response()
        }
        IaqSource::Upstream => proxy_path(&st, uri.path(), uri.query()).await,
    }
}

async fn thermal_data(State(st): State<Arc<AppState>>) -> Response {
    match st.thermal {
        ThermalSource::Native { .. } => Json(native_thermal(&st).await).into_response(),
        ThermalSource::Upstream => match fetch_upstream(&st, "/api/thermal/data").await {
            Ok(resp) => forward(resp).await,
            Err(reason) => Json(ThermalBody::unavailable(reason)).into_response(),
        },
    }
}

#[derive(Deserialize)]
struct HeatmapQuery {
    width: Option<u32>,
    height: Option<u32>,
}

async fn thermal_heatmap(
    State(st): State<Arc<AppState>>,
    Query(q): Query<HeatmapQuery>,
) -> Response {
    let width = q.width.unwrap_or(HEATMAP_WIDTH);
    let height = q.height.unwrap_or(HEATMAP_HEIGHT);
    if matches!(st.thermal, ThermalSource::Native { .. }) {
        return heatmap_from_body(native_thermal(&st).await, width, height);
    }
    let resp = match fetch_upstream(&st, "/api/thermal/data").await {
        Ok(r) => r,
        Err(reason) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": reason})),
            )
                .into_response();
        }
    };
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };
    let body = match ThermalBody::parse(&bytes) {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };
    heatmap_from_body(body, width, height)
}

async fn native_thermal(st: &Arc<AppState>) -> ThermalBody {
    let ThermalSource::Native { bus, addr } = st.thermal else {
        return ThermalBody::unavailable("thermal source is not native");
    };
    let st = Arc::clone(st);
    match tokio::task::spawn_blocking(move || {
        let mut guard = st.mlx.lock().unwrap_or_else(|e| e.into_inner());
        guard.read(bus, addr)
    })
    .await
    {
        Ok(body) => body,
        Err(e) => ThermalBody::unavailable(format!("native thermal task: {e}")),
    }
}

fn heatmap_from_body(body: ThermalBody, width: u32, height: u32) -> Response {
    if body.error {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": body.reason.unwrap_or_else(|| "thermal unavailable".into())
            })),
        )
            .into_response();
    }
    let Some(frame) = body.frame else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "thermal frame missing"})),
        )
            .into_response();
    };
    let Some(img) = render_heatmap(&frame, width, height) else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "thermal frame too short"})),
        )
            .into_response();
    };
    match encode_jpeg(&img) {
        Ok(jpeg) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"))],
            jpeg,
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn proxy_named(State(st): State<Arc<AppState>>, uri: Uri) -> Response {
    proxy_path(&st, uri.path(), uri.query()).await
}

async fn proxy_fallback(State(st): State<Arc<AppState>>, uri: Uri) -> Response {
    proxy_path(&st, uri.path(), uri.query()).await
}

async fn proxy_path(st: &AppState, path: &str, query: Option<&str>) -> Response {
    let suffix = match query {
        Some(q) => format!("{path}?{q}"),
        None => path.to_string(),
    };
    match fetch_upstream(st, &suffix).await {
        Ok(resp) => forward(resp).await,
        Err(reason) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": true,
                "status": "no_upstream",
                "reason": reason,
            })),
        )
            .into_response(),
    }
}

async fn fetch_upstream(st: &AppState, path_and_query: &str) -> Result<reqwest::Response, String> {
    let base = st
        .upstream
        .as_ref()
        .ok_or_else(|| "no upstream configured".to_string())?;
    let url = format!("{}{path_and_query}", base.trim_end_matches('/'));
    st.client.get(url).send().await.map_err(|e| e.to_string())
}

async fn forward(resp: reqwest::Response) -> Response {
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    match resp.bytes().await {
        Ok(bytes) => {
            let mut out = Response::new(Body::from(bytes));
            *out.status_mut() = status;
            if let Ok(v) = HeaderValue::from_str(&content_type) {
                out.headers_mut().insert(header::CONTENT_TYPE, v);
            }
            out
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

fn encode_jpeg(img: &sensorhead::RgbImage) -> anyhow::Result<Vec<u8>> {
    use image::{codecs::jpeg::JpegEncoder, ImageEncoder, Rgb};
    let buffer = image::ImageBuffer::<Rgb<u8>, _>::from_raw(img.width, img.height, img.rgb.clone())
        .ok_or_else(|| anyhow::anyhow!("ironbow buffer shape rejected by image crate"))?;
    let mut out = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut out, 90);
    encoder.write_image(
        buffer.as_raw(),
        img.width,
        img.height,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_names_this_process() {
        let app = router(AppState::new(None).unwrap());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["service"], "sensorhead-rs");
        assert_eq!(v["version"], "0.1.0");
        assert!(v["git_sha"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(v["upstream"].is_null());
        assert_eq!(v["thermal"], "upstream");
        assert_eq!(v["iaq"], "upstream");
    }

    fn repo_bsec_cfg() -> BsecHelperConfig {
        BsecHelperConfig {
            python: std::path::PathBuf::from("python3"),
            script: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../walls/bsec.py"),
            ..BsecHelperConfig::default()
        }
    }

    #[tokio::test]
    async fn helper_iaq_without_egg_is_honest() {
        let app = router(
            AppState::with_sources(
                None,
                ThermalSource::Upstream,
                IaqSource::Helper,
                repo_bsec_cfg(),
            )
            .unwrap(),
        );
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/environment")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let env = EnvironmentBody::parse(&bytes).unwrap();
        assert!(env.error);
        assert!(env.iaq.is_none());
        assert!(!env.apexos_air_quality_possible());
        let reason = env.reason.unwrap_or_default();
        assert!(
            reason.contains("bme68x") || reason.contains("not importable"),
            "{reason}"
        );
    }

    #[tokio::test]
    async fn native_thermal_without_a_device_is_honest() {
        let app = router(
            AppState::with_thermal(None, ThermalSource::Native { bus: 1, addr: 0x33 }).unwrap(),
        );
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/thermal/data")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let body = ThermalBody::parse(&bytes).unwrap();
        assert!(body.error);
        assert!(body.frame.is_none());
        let reason = body.reason.unwrap_or_default();
        assert!(
            reason.contains("i2c") || reason.contains("MLX") || reason.contains("Linux"),
            "{reason}"
        );
    }

    #[tokio::test]
    async fn status_without_upstream_is_composed_and_honest() {
        let app = router(AppState::new(None).unwrap());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["server"], "SensorHead-RS");
        assert_eq!(v["frontend"], "sensorhead-rs");
        assert_eq!(v["environment"]["error"], true);
        assert!(v.get("iaq").is_none());
        assert_eq!(v["cameras"]["error"], "no upstream configured");
    }

    #[tokio::test]
    async fn environment_without_upstream_is_unavailable_not_fake() {
        let app = router(AppState::new(None).unwrap());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/environment")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let env = EnvironmentBody::parse(&bytes).unwrap();
        assert!(env.error);
        assert_eq!(env.status.as_deref(), Some("unavailable"));
        assert!(!env.apexos_air_quality_possible());
    }

    #[tokio::test]
    async fn thermal_without_upstream_is_unavailable() {
        let app = router(AppState::new(None).unwrap());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/thermal/data")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        let body = ThermalBody::parse(&bytes).unwrap();
        assert!(body.error);
        assert!(body.frame.is_none());
    }
}
