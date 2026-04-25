use axum::{
    extract::Request,
    http::HeaderValue,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Middleware that attaches a correlation ID to every request.
///
/// - Reads `X-Request-ID` from the incoming request if present; otherwise
///   generates a new UUID v4.
/// - Records the ID as a `request_id` field on the current tracing span so
///   every log line emitted within the request carries it automatically.
/// - Echoes the ID back in the `X-Request-ID` response header.
pub async fn correlation_id_middleware(mut req: Request, next: Next) -> Response {
    let id = req
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_owned())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Normalise: ensure the header is present on the request for downstream handlers.
    if let Ok(val) = HeaderValue::from_str(&id) {
        req.headers_mut().insert(REQUEST_ID_HEADER, val);
    }

    let span = tracing::Span::current();
    span.record("request_id", &id.as_str());

    let mut response = next.run(req).await;

    if let Ok(val) = HeaderValue::from_str(&id) {
        response.headers_mut().insert(REQUEST_ID_HEADER, val);
    }

    response
}
