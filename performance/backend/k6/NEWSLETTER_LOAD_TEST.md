# Newsletter Load Test

## Overview

This k6 load test covers the newsletter subscribe, confirm, and unsubscribe endpoints. It simulates realistic user behavior patterns and validates performance under load.

## Test Scenarios

The test simulates 100 concurrent users with the following behavior distribution:
- **60%** - Subscribe to newsletter
- **20%** - Confirm subscription (using tokens from subscribe responses)
- **20%** - Unsubscribe from newsletter

## Load Profile

```
Stage 1: Ramp up to 50 users over 1 minute
Stage 2: Sustain 50 users for 3 minutes
Stage 3: Ramp up to 100 users over 1 minute
Stage 4: Sustain 100 users for 3 minutes
Stage 5: Ramp down to 0 users over 2 minutes
```

Total test duration: ~10 minutes

## Performance Thresholds

### Subscribe Endpoint
- P95 latency: < 500ms
- P99 latency: < 1000ms
- Error rate: < 5%

### Confirm Endpoint
- P95 latency: < 300ms
- P99 latency: < 500ms
- Error rate: < 5%

### Unsubscribe Endpoint
- P95 latency: < 300ms
- P99 latency: < 500ms
- Error rate: < 5%

## Running the Test

### Prerequisites
- k6 installed (https://k6.io/docs/getting-started/installation/)
- Backend API running

### Run the test

```bash
# Run against local API
k6 run performance/backend/k6/newsletter-load-test.js

# Run against specific API URL
API_URL=https://api.example.com k6 run performance/backend/k6/newsletter-load-test.js

# Run with custom VU count and duration
k6 run --vus 200 --duration 5m performance/backend/k6/newsletter-load-test.js
```

### View results

The test generates a summary report in `backend/reports/newsletter-load-test-summary.json` with:
- Request counts per endpoint
- P95/P99 latencies
- Average latencies
- Error rates

## Metrics Collected

### Custom Metrics
- `newsletter_subscribe_latency` - Subscribe endpoint response time
- `newsletter_confirm_latency` - Confirm endpoint response time
- `newsletter_unsubscribe_latency` - Unsubscribe endpoint response time
- `newsletter_subscribe_errors` - Subscribe error rate
- `newsletter_confirm_errors` - Confirm error rate
- `newsletter_unsubscribe_errors` - Unsubscribe error rate
- `newsletter_subscribe_count` - Total subscribe requests
- `newsletter_confirm_count` - Total confirm requests
- `newsletter_unsubscribe_count` - Total unsubscribe requests

### Standard k6 Metrics
- `http_req_duration` - HTTP request duration
- `http_req_failed` - Failed HTTP requests
- `http_reqs` - Total HTTP requests

## Baseline Comparison

Results are compared against baseline thresholds. If thresholds are exceeded, the test fails and alerts should be triggered.

To update baselines after performance improvements:

```bash
# Run test and save results
k6 run performance/backend/k6/newsletter-load-test.js --out json=results.json

# Compare against previous baseline
node performance/scripts/compare-results.js results.json
```

## Troubleshooting

### High error rates
- Check API server logs for errors
- Verify database connectivity
- Check rate limiting configuration

### High latencies
- Monitor server CPU and memory usage
- Check database query performance
- Review network latency

### Connection errors
- Verify API URL is correct
- Check firewall rules
- Ensure API server is running

## Integration with CI/CD

This test runs automatically in CI/CD pipelines on schedule. See `.github/workflows/performance.yml` for configuration.
