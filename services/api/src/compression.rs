use axum::{
    body::Body,
    extract::Request,
    middleware::Next,
    response::Response,
};
use tower_http::compression::CompressionLayer;

pub fn compression_layer() -> CompressionLayer {
    CompressionLayer::new()
        .gzip(true)
        .br(true)
        .compress_when(|headers, body: &Body| {
            // Don't compress if already compressed
            if headers.contains_key("content-encoding") {
                return false;
            }

            // Get content length if available
            if let Some(content_length) = headers.get("content-length") {
                if let Ok(len_str) = content_length.to_str() {
                    if let Ok(len) = len_str.parse::<usize>() {
                        // Only compress responses larger than 1KB
                        return len > 1024;
                    }
                }
            }

            // Default to compressing if we can't determine size
            true
        })
}
