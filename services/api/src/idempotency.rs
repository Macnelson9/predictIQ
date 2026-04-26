use std::{sync::Arc, time::Duration};

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::AppState;

const IDEMPOTENCY_HEADER: &str = "Idempotency-Key";
const MAX_KEY_LEN: usize = 128;

#[derive(Serialize, Deserialize, Clone)]
struct CachedResponse {
    status: u16,
    body: Vec<u8>,
    content_type: Option<String>,
}

fn idempotency_cache_key(key: &str) -> String {
    format!("idempotency:v1:{key}")
}

/// Middleware that deduplicates POST requests using an `Idempotency-Key` header.
///
/// - If the header is absent the request passes through unchanged.
/// - If a cached response exists for the key it is returned immediately.
/// - Otherwise the request is executed, the response is cached, and returned.
pub async fn idempotency_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    let key = match req
        .headers()
        .get(IDEMPOTENCY_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.len() <= MAX_KEY_LEN)
    {
        Some(k) => k,
        None => return next.run(req).await,
    };

    let cache_key = idempotency_cache_key(&key);
    let ttl = Duration::from_secs(state.config.idempotency_window_secs);

    // Return cached response if present
    if let Ok(Some(cached)) = state.cache.get_json::<CachedResponse>(&cache_key).await {
        let status = StatusCode::from_u16(cached.status).unwrap_or(StatusCode::OK);
        let mut resp = Response::builder().status(status);
        if let Some(ct) = cached.content_type {
            resp = resp.header(axum::http::header::CONTENT_TYPE, ct);
        }
        resp = resp.header("Idempotency-Replayed", "true");
        return resp
            .body(Body::from(cached.body))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }

    // Execute the request
    let response = next.run(req).await;
    let (parts, body) = response.into_parts();

    let bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // Cache only successful responses (2xx)
    if parts.status.is_success() {
        let content_type = parts
            .headers
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let cached = CachedResponse {
            status: parts.status.as_u16(),
            body: bytes.to_vec(),
            content_type,
        };
        let _ = state.cache.set_json(&cache_key, &cached, ttl).await;
    }

    let mut resp = Response::from_parts(parts, Body::from(bytes));
    resp.headers_mut()
        .insert("Idempotency-Replayed", HeaderValue::from_static("false"));
    resp
}
