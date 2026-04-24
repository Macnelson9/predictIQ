use tower_http::compression::CompressionLayer;

pub fn compression_layer() -> CompressionLayer {
    CompressionLayer::new().gzip(true).br(true)
}
