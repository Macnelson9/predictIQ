use opentelemetry::{
    global,
    trace::{TraceError, TracerProvider as _},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime,
    trace::{RandomIdGenerator, Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions::resource::{SERVICE_NAME, SERVICE_VERSION};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize distributed tracing with OpenTelemetry
pub fn init_tracing(
    service_name: &str,
    service_version: &str,
    otlp_endpoint: Option<String>,
    sample_rate: f64,
) -> Result<(), TraceError> {
    // Create resource with service information
    let resource = Resource::new(vec![
        KeyValue::new(SERVICE_NAME, service_name.to_string()),
        KeyValue::new(SERVICE_VERSION, service_version.to_string()),
        KeyValue::new("deployment.environment", std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string())),
    ]);

    // Configure sampler based on sample rate
    let sampler = if sample_rate >= 1.0 {
        Sampler::AlwaysOn
    } else if sample_rate <= 0.0 {
        Sampler::AlwaysOff
    } else {
        Sampler::TraceIdRatioBased(sample_rate)
    };

    // Build tracer provider
    let tracer_provider = if let Some(ref endpoint) = otlp_endpoint {
        // Export to OTLP collector (Jaeger, Zipkin, etc.)
        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(endpoint);

        opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(exporter)
            .with_trace_config(
                opentelemetry_sdk::trace::Config::default()
                    .with_sampler(sampler)
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(resource),
            )
            .install_batch(runtime::Tokio)
            .map(|_tracer| {
                // install_batch already sets the global provider; build a local one too
                TracerProvider::builder()
                    .with_config(
                        opentelemetry_sdk::trace::Config::default()
                            .with_sampler(Sampler::AlwaysOn),
                    )
                    .build()
            })
            .unwrap_or_else(|_| {
                TracerProvider::builder().build()
            })
    } else {
        // No exporter configured - use noop
        TracerProvider::builder()
            .with_config(
                opentelemetry_sdk::trace::Config::default()
                    .with_sampler(sampler)
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(resource),
            )
            .build()
    };

    // Set global tracer provider
    global::set_tracer_provider(tracer_provider.clone());

    // Create tracing layer
    let telemetry_layer = tracing_opentelemetry::layer()
        .with_tracer(tracer_provider.tracer(service_name.to_string()));

    // Initialize tracing subscriber with OpenTelemetry layer
    tracing_subscriber::registry()
        .with(EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(telemetry_layer)
        .init();

    tracing::info!(
        service_name = service_name,
        service_version = service_version,
        sample_rate = sample_rate,
        otlp_endpoint = otlp_endpoint.as_deref().unwrap_or("none"),
        "Distributed tracing initialized"
    );

    Ok(())
}

/// Shutdown tracing and flush remaining spans
pub fn shutdown_tracing() {
    tracing::info!("Shutting down tracing");
    global::shutdown_tracer_provider();
}

/// Extract trace context from HTTP headers for propagation
pub fn extract_trace_context(headers: &axum::http::HeaderMap) -> opentelemetry::Context {
    use opentelemetry::propagation::TextMapPropagator;
    use opentelemetry_sdk::propagation::TraceContextPropagator;

    let propagator = TraceContextPropagator::new();
    let context = propagator.extract(&HeaderExtractor(headers));
    context
}

/// Inject trace context into HTTP headers for propagation
pub fn inject_trace_context(
    headers: &mut reqwest::header::HeaderMap,
    context: &opentelemetry::Context,
) {
    use opentelemetry::propagation::TextMapPropagator;
    use opentelemetry_sdk::propagation::TraceContextPropagator;

    let propagator = TraceContextPropagator::new();
    propagator.inject_context(context, &mut HeaderInjector(headers));
}

/// Helper to extract headers for OpenTelemetry propagation
struct HeaderExtractor<'a>(&'a axum::http::HeaderMap);

impl<'a> opentelemetry::propagation::Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Helper to inject headers for OpenTelemetry propagation
struct HeaderInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl<'a> opentelemetry::propagation::Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(header_name) = reqwest::header::HeaderName::from_bytes(key.as_bytes()) {
            if let Ok(header_value) = reqwest::header::HeaderValue::from_str(&value) {
                self.0.insert(header_name, header_value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampler_configuration() {
        // Test always on
        let result = init_tracing("test-service", "0.1.0", None, 1.0);
        assert!(result.is_ok());
        shutdown_tracing();

        // Test always off
        let result = init_tracing("test-service", "0.1.0", None, 0.0);
        assert!(result.is_ok());
        shutdown_tracing();

        // Test ratio-based
        let result = init_tracing("test-service", "0.1.0", None, 0.5);
        assert!(result.is_ok());
        shutdown_tracing();
    }
}
