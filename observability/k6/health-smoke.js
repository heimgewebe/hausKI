import http from 'k6/http';
import { sleep } from 'k6';

export const options = {
  vus: 5,
  duration: '5s',
  thresholds: {
    'http_req_duration{p(95)}': ['<500'], // 95% of requests should complete below 500ms
  },
};

export default function () {
  http.get('http://localhost:8080/health');
  sleep(0.05);
}
