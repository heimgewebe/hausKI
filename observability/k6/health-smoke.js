import http from 'k6/http';
import { sleep } from 'k6';

export const options = {
  vus: 5,
  duration: '5s',
  thresholds: {
    // Basis-Schwelle; CI liest das echte Budget zusätzlich aus policies/limits.yaml aus
    'http_req_duration{p(95)}': ['<400'],
    'http_req_failed': ['rate<0.01'],
  },
};

export default function () {
  http.get('http://localhost:8080/health');
  sleep(0.05);
}
