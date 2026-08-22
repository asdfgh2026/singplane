#![cfg_attr(windows, windows_subsystem = "windows")]

//! HTTP shell around [`singpanel_core`]. GPUI talks JSON over loopback.
//! Official sing-box stays an external process. Elevated TUN still goes through
//! the existing helper service.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use singpanel_core::assemble::{AssembleOptions, ContentKind, PatchOptions};
use singpanel_core::convert::ConvertSidecar;
use singpanel_core::engine::{Engine, StartSpec};

struct AppState {
    token: String,
    engine: Mutex<Engine>,
    convert: Mutex<ConvertSidecar>,
}

#[derive(Serialize)]
struct ApiError {
    ok: bool,
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

#[derive(Serialize)]
struct PingBody {
    ok: bool,
    service: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartBody {
    core_path: String,
    config_path: String,
    #[serde(default)]
    require_helper: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckBody {
    core_path: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FetchBody {
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConvertBody {
    subscription_body: String,
    #[serde(default)]
    include: String,
    #[serde(default)]
    exclude: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssembleBody {
    source_body: String,
    template_content: String,
    #[serde(default)]
    options: AssembleOptions,
    #[serde(default)]
    patch: PatchOptions,
    #[serde(default)]
    content_kind: Option<ContentKind>,
    #[serde(default)]
    convert_if_needed: bool,
}

#[tokio::main]
async fn main() {
    let token = random_token();
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("listen 127.0.0.1");
    let port = listener.local_addr().expect("local addr").port();
    println!("READY port={port} token={token}");
    let _ = std::io::Write::flush(&mut std::io::stdout());

    let state = Arc::new(AppState {
        token,
        engine: Mutex::new(Engine::new()),
        convert: Mutex::new(ConvertSidecar::new()),
    });

    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/ping", get(ping))
        .route("/v1/status", get(status))
        .route("/v1/start", post(start))
        .route("/v1/stop", post(stop))
        .route("/v1/check", post(check_cfg))
        .route("/v1/fetch", post(fetch_sub))
        .route("/v1/convert", post(convert_sub))
        .route("/v1/assemble", post(assemble_cfg))
        .route("/v1/clash", post(clash_proxy))
        .with_state(state.clone());

    let shutdown_state = state;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            wait_shutdown_signal().await;
            shutdown_state.engine.lock().await.stop();
            shutdown_state.convert.lock().await.stop();
        })
        .await
        .expect("serve");
}

async fn wait_shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("listen SIGTERM");
        tokio::select! {
            _ = ctrl_c => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = ctrl_c.await;
    }
}

fn random_token() -> String {
    let mut b = [0u8; 24];
    if getrandom::getrandom(&mut b).is_err() {
        // Fallback to high-entropy source if getrandom unexpectedly fails
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        return format!("{:x}{:08x}{:08x}", nanos, std::process::id(), rand_seed());
    }
    b.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn rand_seed() -> u64 {
    // Fallback entropy: PID + timestamp (only reached when getrandom fails)
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (std::process::id() as u64)
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == format!("Bearer {token}"))
}

async fn healthz() -> &'static str {
    "ok"
}

async fn ping(State(st): State<Arc<AppState>>, headers: HeaderMap) -> Result<Json<PingBody>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    Ok(Json(PingBody {
        ok: true,
        service: "singpanel-host",
    }))
}

async fn status(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    let snap = st.engine.lock().await.snapshot();
    Ok(Json(serde_json::to_value(snap).unwrap_or_else(|_| {
        serde_json::json!({"ok": false, "error": "encode"})
    })))
}

async fn start(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<StartBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    let spec = StartSpec {
        core_path: body.core_path,
        config_path: body.config_path,
        require_helper: body.require_helper,
    };
    match st.engine.lock().await.start(spec) {
        Ok(snap) => Ok(Json(serde_json::to_value(snap).unwrap_or_else(|_| {
            serde_json::json!({"ok": true, "running": true})
        }))),
        Err(err) => Ok(Json(serde_json::json!({
            "ok": false,
            "running": false,
            "error": err.message,
            "code": err.code,
        }))),
    }
}

async fn stop(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    let snap = st.engine.lock().await.stop();
    Ok(Json(serde_json::to_value(snap).unwrap_or_else(|_| {
        serde_json::json!({"ok": true, "running": false})
    })))
}

async fn check_cfg(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<CheckBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    match singpanel_core::check::check_content(&body.core_path, &body.content) {
        Ok(()) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(error) => Ok(Json(serde_json::json!({"ok": false, "error": error}))),
    }
}

async fn fetch_sub(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<FetchBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    let out = singpanel_core::fetch::fetch_url(&body.url).await;
    Ok(Json(serde_json::to_value(out).unwrap_or_else(|_| {
        serde_json::json!({"ok": false, "error": "encode"})
    })))
}

async fn convert_sub(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ConvertBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    let mut conv = st.convert.lock().await;
    let out = singpanel_core::convert::convert_body(
        &mut conv,
        body.subscription_body,
        body.include,
        body.exclude,
    )
    .await;
    Ok(Json(out))
}

async fn assemble_cfg(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<AssembleBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    let kind = body.content_kind.unwrap_or(ContentKind::Auto);
    let detected = if kind == ContentKind::Auto {
        singpanel_core::assemble::detect(&body.source_body)
    } else {
        kind
    };
    let mut source = body.source_body.clone();
    let mut extra_warnings = Vec::new();
    if body.convert_if_needed
        && (detected == ContentKind::Clash || detected == ContentKind::UriList)
    {
        let mut conv = st.convert.lock().await;
        let converted = singpanel_core::convert::convert_body(
            &mut conv,
            source.clone(),
            body.options.include.clone(),
            body.options.exclude.clone(),
        )
        .await;
        if converted.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let err = converted
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Clash/URI 转换失败");
            return Ok(Json(serde_json::json!({
                "ok": false,
                "detectedKind": detected,
                "error": err,
            })));
        }
        if let Some(arr) = converted.get("warnings").and_then(|v| v.as_array()) {
            extra_warnings = arr.clone();
        }
        source = serde_json::json!({
            "outbounds": converted.get("outbounds").cloned().unwrap_or(serde_json::json!([])),
            "endpoints": converted.get("endpoints").cloned().unwrap_or(serde_json::json!([])),
        })
        .to_string();
    }
    let mut out = singpanel_core::assemble::run(
        &source,
        &body.template_content,
        &body.options,
        &body.patch,
        if body.convert_if_needed {
            ContentKind::Auto
        } else {
            kind
        },
    );
    if !extra_warnings.is_empty() {
        // prepend convert warnings as raw objects via re-serialize
        if let Ok(mut v) = serde_json::to_value(&out) {
            if let Some(arr) = v.get_mut("warnings").and_then(|w| w.as_array_mut()) {
                let mut merged = extra_warnings;
                merged.extend(arr.iter().cloned());
                *arr = merged;
            }
            return Ok(Json(v));
        }
    }
    let _ = &mut out;
    Ok(Json(serde_json::to_value(out).unwrap_or_else(|_| {
        serde_json::json!({"ok": false, "error": "encode"})
    })))
}

async fn clash_proxy(
    State(st): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<singpanel_core::clash::ClashReq>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ApiError>)> {
    deny(&st, &headers)?;
    Ok(Json(singpanel_core::clash::proxy(body).await))
}

fn deny(st: &AppState, headers: &HeaderMap) -> Result<(), (StatusCode, Json<ApiError>)> {
    if authorized(headers, &st.token) {
        return Ok(());
    }
    Err((
        StatusCode::UNAUTHORIZED,
        Json(ApiError {
            ok: false,
            error: "unauthorized".into(),
            code: Some("unauthorized".into()),
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_random_token_entropy_and_uniqueness() {
        let mut tokens = HashSet::new();
        for _ in 0..200 {
            let tok = random_token();
            assert_eq!(tok.len(), 48, "Token must be 48 hex characters (24 bytes)");
            assert!(
                tok.chars().all(|c| c.is_ascii_hexdigit()),
                "Token must be valid hex: {tok}"
            );
            assert!(
                tokens.insert(tok.clone()),
                "Token collision detected: {tok}"
            );
        }
    }

    #[test]
    fn test_authorized_bearer() {
        let mut headers = HeaderMap::new();
        let tok = "abcdef123456";
        assert!(!authorized(&headers, tok));

        headers.insert(
            axum::http::header::AUTHORIZATION,
            format!("Bearer {tok}").parse().unwrap(),
        );
        assert!(authorized(&headers, tok));
        assert!(!authorized(&headers, "wrong_token"));
    }
}
