import http from 'k6/http';
import { sleep } from 'k6';
import { textSummary } from 'https://jslib.k6.io/k6-summary/0.0.4/index.js';

export const options = {
  vus: 5,
  duration: '5s',
  thresholds: {
    // Base threshold; CI additionally reads the actual budget from policies/limits.yaml
    http_req_duration: ['p(95)<400'],
    'http_req_failed': ['rate<0.01'],
  },
};

export default function () {
  http.get('http://localhost:8080/health');
  sleep(0.05);
}

export function handleSummary(data) {
  return {
    stdout: textSummary(data, { indent: ' ', enableColors: true }),
    'observability/k6/summary.json': JSON.stringify(data, null, 2),
  };
}
