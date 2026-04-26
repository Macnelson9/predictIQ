import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';
import { randomIntBetween, randomString } from 'https://jslib.k6.io/k6-utils/1.2.0/index.js';

// Custom metrics
const subscribeLatency = new Trend('newsletter_subscribe_latency');
const confirmLatency = new Trend('newsletter_confirm_latency');
const unsubscribeLatency = new Trend('newsletter_unsubscribe_latency');
const subscribeErrors = new Rate('newsletter_subscribe_errors');
const confirmErrors = new Rate('newsletter_confirm_errors');
const unsubscribeErrors = new Rate('newsletter_unsubscribe_errors');
const subscribeCount = new Counter('newsletter_subscribe_count');
const confirmCount = new Counter('newsletter_confirm_count');
const unsubscribeCount = new Counter('newsletter_unsubscribe_count');

export const options = {
  stages: [
    { duration: '1m', target: 50 },   // Ramp up to 50 users
    { duration: '3m', target: 50 },   // Stay at 50 users
    { duration: '1m', target: 100 },  // Ramp up to 100 users
    { duration: '3m', target: 100 },  // Stay at 100 users
    { duration: '2m', target: 0 },    // Ramp down
  ],
  thresholds: {
    // P95 latency should be under 500ms for subscribe
    'newsletter_subscribe_latency': ['p(95)<500', 'p(99)<1000'],
    // P95 latency should be under 300ms for confirm
    'newsletter_confirm_latency': ['p(95)<300', 'p(99)<500'],
    // P95 latency should be under 300ms for unsubscribe
    'newsletter_unsubscribe_latency': ['p(95)<300', 'p(99)<500'],
    // Error rates should be low
    'newsletter_subscribe_errors': ['rate<0.05'],  // Less than 5% errors
    'newsletter_confirm_errors': ['rate<0.05'],
    'newsletter_unsubscribe_errors': ['rate<0.05'],
    // Overall HTTP errors
    'http_req_failed': ['rate<0.05'],
  },
};

const BASE_URL = __ENV.API_URL || 'http://localhost:3001';

// Store tokens for confirm flow
const confirmTokens = [];

export default function () {
  // Simulate user behavior patterns
  const scenario = randomIntBetween(1, 100);
  
  if (scenario <= 60) {
    // 60% - Subscribe to newsletter
    subscribeToNewsletter();
  } else if (scenario <= 80) {
    // 20% - Confirm subscription (if tokens available)
    confirmSubscription();
  } else {
    // 20% - Unsubscribe
    unsubscribeFromNewsletter();
  }
  
  sleep(randomIntBetween(1, 3));
}

function subscribeToNewsletter() {
  const email = `user_${randomString(8)}@example.com`;
  const source = ['landing-page', 'blog', 'social-media'][randomIntBetween(0, 2)];
  
  const payload = JSON.stringify({
    email: email,
    source: source,
  });
  
  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
    tags: {
      endpoint: 'newsletter_subscribe',
      operation: 'subscribe',
    },
  };
  
  const res = http.post(`${BASE_URL}/api/v1/newsletter/subscribe`, payload, params);
  
  subscribeLatency.add(res.timings.duration);
  subscribeCount.add(1);
  
  const success = check(res, {
    'subscribe status is 200': (r) => r.status === 200,
    'subscribe response has success field': (r) => {
      try {
        const body = JSON.parse(r.body);
        return body.success === true;
      } catch {
        return false;
      }
    },
    'subscribe response time < 500ms': (r) => r.timings.duration < 500,
  });
  
  if (!success) {
    subscribeErrors.add(1);
  }
  
  // Extract confirmation token if available (for testing confirm flow)
  try {
    const body = JSON.parse(res.body);
    if (body.token) {
      confirmTokens.push(body.token);
    }
  } catch {
    // Ignore parse errors
  }
}

function confirmSubscription() {
  if (confirmTokens.length === 0) {
    return;
  }
  
  // Use a random token from the pool
  const token = confirmTokens[randomIntBetween(0, confirmTokens.length - 1)];
  
  const params = {
    tags: {
      endpoint: 'newsletter_confirm',
      operation: 'confirm',
    },
  };
  
  const res = http.get(`${BASE_URL}/api/v1/newsletter/confirm?token=${token}`, params);
  
  confirmLatency.add(res.timings.duration);
  confirmCount.add(1);
  
  const success = check(res, {
    'confirm status is 200 or 400': (r) => r.status === 200 || r.status === 400,
    'confirm response has success field': (r) => {
      try {
        const body = JSON.parse(r.body);
        return 'success' in body;
      } catch {
        return false;
      }
    },
    'confirm response time < 300ms': (r) => r.timings.duration < 300,
  });
  
  if (!success) {
    confirmErrors.add(1);
  }
}

function unsubscribeFromNewsletter() {
  const email = `user_${randomString(8)}@example.com`;
  
  const payload = JSON.stringify({
    email: email,
  });
  
  const params = {
    headers: {
      'Content-Type': 'application/json',
    },
    tags: {
      endpoint: 'newsletter_unsubscribe',
      operation: 'unsubscribe',
    },
  };
  
  const res = http.del(`${BASE_URL}/api/v1/newsletter/unsubscribe`, payload, params);
  
  unsubscribeLatency.add(res.timings.duration);
  unsubscribeCount.add(1);
  
  const success = check(res, {
    'unsubscribe status is 200 or 404': (r) => r.status === 200 || r.status === 404,
    'unsubscribe response has success field': (r) => {
      try {
        const body = JSON.parse(r.body);
        return 'success' in body;
      } catch {
        return false;
      }
    },
    'unsubscribe response time < 300ms': (r) => r.timings.duration < 300,
  });
  
  if (!success) {
    unsubscribeErrors.add(1);
  }
}

export function handleSummary(data) {
  const subscribeMetrics = data.metrics.newsletter_subscribe_latency;
  const confirmMetrics = data.metrics.newsletter_confirm_latency;
  const unsubscribeMetrics = data.metrics.newsletter_unsubscribe_latency;
  
  const summary = {
    subscribe: {
      count: data.metrics.newsletter_subscribe_count?.values?.count || 0,
      p95: subscribeMetrics?.values?.['p(95)'] || 0,
      p99: subscribeMetrics?.values?.['p(99)'] || 0,
      avg: subscribeMetrics?.values?.avg || 0,
      errors: data.metrics.newsletter_subscribe_errors?.values?.rate || 0,
    },
    confirm: {
      count: data.metrics.newsletter_confirm_count?.values?.count || 0,
      p95: confirmMetrics?.values?.['p(95)'] || 0,
      p99: confirmMetrics?.values?.['p(99)'] || 0,
      avg: confirmMetrics?.values?.avg || 0,
      errors: data.metrics.newsletter_confirm_errors?.values?.rate || 0,
    },
    unsubscribe: {
      count: data.metrics.newsletter_unsubscribe_count?.values?.count || 0,
      p95: unsubscribeMetrics?.values?.['p(95)'] || 0,
      p99: unsubscribeMetrics?.values?.['p(99)'] || 0,
      avg: unsubscribeMetrics?.values?.avg || 0,
      errors: data.metrics.newsletter_unsubscribe_errors?.values?.rate || 0,
    },
  };
  
  console.log('\n=== Newsletter Load Test Results ===');
  console.log('\nSubscribe Endpoint:');
  console.log(`  Requests: ${summary.subscribe.count}`);
  console.log(`  P95 Latency: ${summary.subscribe.p95.toFixed(2)}ms`);
  console.log(`  P99 Latency: ${summary.subscribe.p99.toFixed(2)}ms`);
  console.log(`  Avg Latency: ${summary.subscribe.avg.toFixed(2)}ms`);
  console.log(`  Error Rate: ${(summary.subscribe.errors * 100).toFixed(2)}%`);
  
  console.log('\nConfirm Endpoint:');
  console.log(`  Requests: ${summary.confirm.count}`);
  console.log(`  P95 Latency: ${summary.confirm.p95.toFixed(2)}ms`);
  console.log(`  P99 Latency: ${summary.confirm.p99.toFixed(2)}ms`);
  console.log(`  Avg Latency: ${summary.confirm.avg.toFixed(2)}ms`);
  console.log(`  Error Rate: ${(summary.confirm.errors * 100).toFixed(2)}%`);
  
  console.log('\nUnsubscribe Endpoint:');
  console.log(`  Requests: ${summary.unsubscribe.count}`);
  console.log(`  P95 Latency: ${summary.unsubscribe.p95.toFixed(2)}ms`);
  console.log(`  P99 Latency: ${summary.unsubscribe.p99.toFixed(2)}ms`);
  console.log(`  Avg Latency: ${summary.unsubscribe.avg.toFixed(2)}ms`);
  console.log(`  Error Rate: ${(summary.unsubscribe.errors * 100).toFixed(2)}%`);
  
  return {
    'backend/reports/newsletter-load-test-summary.json': JSON.stringify(summary),
  };
}
