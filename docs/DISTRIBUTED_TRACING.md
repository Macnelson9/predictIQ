# Distributed Tracing Configuration

This document describes the distributed tracing implementation across PredictIQ services.

## Overview

Distributed tracing is implemented using OpenTelemetry with support for Jaeger and Zipkin exporters. Trace context is propagated across service boundaries to enable end-to-end request tracking.

## Architecture

- **API Service (Rust)**: Uses `tracing-opentelemetry` with OTLP exporter
- **TTS Service (TypeScript)**: Uses `@opentelemetry/sdk-node` with OTLP exporter
- **Trace Propagation**: W3C Trace Context standard via HTTP headers

## Configuration

### Environment Variables

All services support the following environment variables:

- `OTEL_EXPORTER_OTLP_ENDPOINT`: OTLP collector endpoint (default: `http://localhost:4317`)
- `OTEL_SERVICE_NAME`: Service name for traces (default: service-specific)
- `OTEL_TRACE_SAMPLING_RATIO`: Sampling rate 0.0-1.0 (default: `1.0`)
- `RUST_LOG`: Log level for Rust services (default: `info`)

### API Service Configuration

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4317
export OTEL_SERVICE_NAME=predictiq-api
export OTEL_TRACE_SAMPLING_RATIO=0.1
export RUST_LOG=info
```

### TTS Service Configuration

```bash
export OTEL_EXPORTER_OTLP_ENDPOINT=http://jaeger:4317
export OTEL_SERVICE_NAME=predictiq-tts
export OTEL_TRACE_SAMPLING_RATIO=0.1
```

## Deployment

### Using Jaeger (Recommended)

```yaml
# docker-compose.yml
services:
  jaeger:
    image: jaegertracing/all-in-one:latest
    ports:
      - "16686:16686"  # Jaeger UI
      - "4317:4317"    # OTLP gRPC receiver
      - "4318:4318"    # OTLP HTTP receiver
    environment:
      - COLLECTOR_OTLP_ENABLED=true
```

Access Jaeger UI at: http://localhost:16686

### Using Zipkin

```yaml
# docker-compose.yml
services:
  zipkin:
    image: openzipkin/zipkin:latest
    ports:
      - "9411:9411"
  
  otel-collector:
    image: otel/opentelemetry-collector:latest
    command: ["--config=/etc/otel-collector-config.yaml"]
    volumes:
      - ./otel-collector-config.yaml:/etc/otel-collector-config.yaml
    ports:
      - "4317:4317"
```

Access Zipkin UI at: http://localhost:9411

## Trace Context Propagation

Trace context is automatically propagated via HTTP headers using W3C Trace Context:

- `traceparent`: Contains trace ID, span ID, and sampling decision
- `tracestate`: Vendor-specific trace information

### Example: Frontend to API

```typescript
// Frontend makes request with trace context
const response = await fetch('/api/markets/featured', {
  headers: {
    'traceparent': '00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01'
  }
});
```

### Example: API to Blockchain

Trace context is automatically injected into outgoing HTTP requests via the OpenTelemetry instrumentation.

## Sampling Strategies

### Production (Low Volume)
```bash
OTEL_TRACE_SAMPLING_RATIO=0.1  # Sample 10% of traces
```

### Staging/Testing
```bash
OTEL_TRACE_SAMPLING_RATIO=1.0  # Sample 100% of traces
```

### High-Traffic Production
```bash
OTEL_TRACE_SAMPLING_RATIO=0.01  # Sample 1% of traces
```

## Custom Spans

### Rust (API Service)

```rust
use tracing::{info_span, instrument};

#[instrument(skip(state))]
async fn my_handler(state: State<AppState>) -> Result<Json<Response>> {
    let span = info_span!("database_query");
    let _guard = span.enter();
    
    // Your code here
    
    Ok(Json(response))
}
```

### TypeScript (TTS Service)

```typescript
import { trace } from "@opentelemetry/api";

const tracer = trace.getTracer("tts-service");

async function processJob(job: TTSJob) {
  return tracer.startActiveSpan("process_job", async (span) => {
    try {
      span.setAttribute("job.id", job.id);
      span.setAttribute("job.provider", job.provider);
      
      // Your code here
      
      span.setStatus({ code: SpanStatusCode.OK });
    } catch (error) {
      span.setStatus({ code: SpanStatusCode.ERROR, message: String(error) });
      throw error;
    } finally {
      span.end();
    }
  });
}
```

## Troubleshooting

### No traces appearing in Jaeger

1. Check OTLP endpoint is reachable:
   ```bash
   curl http://localhost:4317
   ```

2. Verify environment variables are set correctly

3. Check service logs for OpenTelemetry errors

4. Ensure sampling ratio is not 0.0

### Traces not connected across services

1. Verify trace context headers are being propagated
2. Check that all services use the same OTLP endpoint
3. Ensure W3C Trace Context propagation is enabled

### High memory usage

1. Reduce sampling ratio
2. Configure batch span processor limits
3. Enable span compression in OTLP exporter

## Metrics

Key metrics to monitor:

- `otel.traces.exported`: Number of traces exported
- `otel.traces.dropped`: Number of traces dropped
- `otel.exporter.queue.size`: Export queue size
- `otel.exporter.latency`: Export latency

## References

- [OpenTelemetry Documentation](https://opentelemetry.io/docs/)
- [Jaeger Documentation](https://www.jaegertracing.io/docs/)
- [W3C Trace Context](https://www.w3.org/TR/trace-context/)
