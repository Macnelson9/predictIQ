use axum::{
    body::Body,
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

use crate::security::sanitize;

const DEFAULT_REQUEST_BODY_MAX_BYTES: usize = 1_048_576;
const REQUEST_BODY_MAX_BYTES_ENV: &str = "REQUEST_BODY_MAX_BYTES";

#[derive(Serialize)]
pub struct ValidationError {
    pub error: String,
    pub message: String,
}

impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

pub fn parse_request_body_max_bytes(raw: Option<&str>) -> usize {
    raw.and_then(|value| value.trim().parse::<usize>().ok())
        .filter(|bytes| *bytes > 0)
        .unwrap_or(DEFAULT_REQUEST_BODY_MAX_BYTES)
}

pub fn request_body_max_bytes_from_env() -> usize {
    parse_request_body_max_bytes(std::env::var(REQUEST_BODY_MAX_BYTES_ENV).ok().as_deref())
}

/// Request validation middleware — applied globally to all routes.
///
/// Rejects requests with `400 Bad Request` when:
/// - Query string or path contains SQL injection patterns (detected via [`sanitize::contains_sql_injection`])
/// - Query string exceeds 2 048 characters
/// - Path contains `..` (directory traversal) or `//` (double-slash)
///
/// Safe traffic passes through unmodified.
pub async fn request_validation_middleware(
    request: Request,
    next: Next,
) -> Result<Response, ValidationError> {
    // Extract and validate query parameters
    let uri = request.uri();
    let query = uri.query().unwrap_or("");

    // Check for SQL injection patterns in query
    if sanitize::contains_sql_injection(query) {
        return Err(ValidationError {
            error: "invalid_input".to_string(),
            message: "Invalid characters detected in request".to_string(),
        });
    }

    // Check for excessively long query strings
    if query.len() > 2048 {
        return Err(ValidationError {
            error: "invalid_input".to_string(),
            message: "Query string too long".to_string(),
        });
    }

    // Validate path parameters
    let path = uri.path();
    if sanitize::contains_sql_injection(path) {
        return Err(ValidationError {
            error: "invalid_input".to_string(),
            message: "Invalid characters detected in path".to_string(),
        });
    }

    // Check for path traversal attempts
    if path.contains("..") || path.contains("//") {
        return Err(ValidationError {
            error: "invalid_input".to_string(),
            message: "Invalid path format".to_string(),
        });
    }

    Ok(next.run(request).await)
}

/// Content-Type validation for POST/PUT requests
pub async fn content_type_validation_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let method = request.method();
    let headers = request.headers();

    // Only validate POST, PUT, PATCH requests
    if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        if let Some(content_type) = headers.get("content-type") {
            let ct = content_type.to_str().unwrap_or("");

            // Allow only JSON and form data
            if !ct.starts_with("application/json")
                && !ct.starts_with("application/x-www-form-urlencoded")
                && !ct.starts_with("multipart/form-data")
            {
                return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
            }
        } else {
            // Require Content-Type header for mutation requests
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    Ok(next.run(request).await)
}

/// Request size validation
pub async fn request_size_validation_middleware(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = request.headers();
    let max_bytes = request_body_max_bytes_from_env();

    // Check Content-Length header
    if let Some(content_length) = headers.get("content-length") {
        if let Ok(length_str) = content_length.to_str() {
            if let Ok(length) = length_str.parse::<usize>() {
                if length > max_bytes {
                    return Err(StatusCode::PAYLOAD_TOO_LARGE);
                }
            }
        }
    }

    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, middleware, routing::get, Router};
    use tower::ServiceExt;

    async fn validation_app() -> Router {
        Router::new()
            .route("/api/v1/items/:id", get(|| async { "ok" }))
            .layer(middleware::from_fn(request_validation_middleware))
    }

    // ── request_validation_middleware ─────────────────────────────────────

    #[tokio::test]
    async fn allows_clean_request() {
        let response = validation_app()
            .await
            .oneshot(
                Request::builder()
                    .uri("/api/v1/items/42?sort=asc")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn blocks_sql_injection_in_query() {
        let response = validation_app()
            .await
            .oneshot(
                Request::builder()
                    .uri("/api/v1/items/42?id=1%20OR%201%3D1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn blocks_sql_injection_in_path() {
        let response = validation_app()
            .await
            .oneshot(
                Request::builder()
                    .uri("/api/v1/items/1%20UNION%20SELECT%201")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn blocks_path_traversal() {
        let response = validation_app()
            .await
            .oneshot(
                Request::builder()
                    .uri("/api/v1/items/../secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn blocks_query_string_too_long() {
        let long_query = "a=".to_string() + &"x".repeat(2048);
        let uri = format!("/api/v1/items/1?{long_query}");
        let response = validation_app()
            .await
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // ── request_size_validation_middleware ────────────────────────────────

    #[tokio::test]
    async fn allows_request_within_size_limit() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(request_size_validation_middleware));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("content-length", "100")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn blocks_request_exceeding_size_limit() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(middleware::from_fn(request_size_validation_middleware));

        let over_limit = DEFAULT_REQUEST_BODY_MAX_BYTES + 1;
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/")
                    .header("content-length", over_limit.to_string())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
