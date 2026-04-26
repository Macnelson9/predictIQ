use axum::{
    extract::Request,
    http::{header, HeaderValue},
    middleware::Next,
    response::Response,
};

pub const CURRENT_VERSION: &str = "v1";
pub const SUPPORTED_VERSIONS: &[&str] = &["v1"];

/// Injects the resolved API version into request extensions.
/// Reads `API-Version` header; defaults to current version.
#[derive(Clone, Debug)]
pub struct ApiVersion(pub String);

pub async fn versioning_middleware(mut req: Request, next: Next) -> Response {
    let version = req
        .headers()
        .get("API-Version")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_lowercase())
        .filter(|v| SUPPORTED_VERSIONS.contains(&v.as_str()))
        .unwrap_or_else(|| CURRENT_VERSION.to_string());

    req.extensions_mut().insert(ApiVersion(version));
    next.run(req).await
}

/// Adds `Deprecation` and `Sunset` headers to responses for v1 routes.
/// Communicates the deprecation policy to clients.
pub async fn v1_deprecation_middleware(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    // RFC 8594 Deprecation header — "true" signals the resource is deprecated
    headers.insert(
        "Deprecation",
        HeaderValue::from_static("false"),
    );
    // Sunset date: communicate when v1 will be removed (1 year from now, update as needed)
    headers.insert(
        "Sunset",
        HeaderValue::from_static("Sat, 25 Apr 2026 00:00:00 GMT"),
    );
    headers.insert(
        header::LINK,
        HeaderValue::from_static(
            "</api/v1>; rel=\"deprecation\"; type=\"text/html\"",
        ),
    );
    response
}
