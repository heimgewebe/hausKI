#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

# CI Smoke-Test:
# - startet Mock-Ollama (scripts/mock_ollama.py)
# - startet HausKI-Core mit Env-Verdrahtung auf den Mock
# - prüft /health (200) und /v1/chat (Modell + Content)

ROOT="$(git rev-parse --show-toplevel)"
LOG_CORE="${ROOT}/target/smoke-core.log"
LOG_MOCK="${ROOT}/target/smoke-mock.log"
MODEL="${MODEL:-mock-model}"
HOST="${HOST:-http://127.0.0.1:8080}"
BIND="${HOST#*://}"
if [[ -z "${BIND}" || "${BIND}" == "${HOST}" ]]; then
  echo "unable to derive bind address from HOST='${HOST}'" >&2
  exit 1
fi

# Tool-Prüfung
command -v python3 >/dev/null 2>&1 || { echo "python3 missing" >&2; exit 127; }
command -v cargo   >/dev/null 2>&1 || { echo "cargo missing"   >&2; exit 127; }
command -v curl    >/dev/null 2>&1 || { echo "curl missing"    >&2; exit 127; }
command -v jq      >/dev/null 2>&1 || { echo "jq missing"      >&2; exit 127; }

cleanup() {
  set +e
  [[ -n "${CORE_PID:-}" ]] && kill "${CORE_PID}" 2>/dev/null || true
  [[ -n "${MOCK_PID:-}" ]] && kill "${MOCK_PID}" 2>/dev/null || true
}
trap cleanup EXIT

mkdir -p "${ROOT}/target"

echo "[smoke] start mock-ollama…"
python3 "${ROOT}/scripts/mock_ollama.py" >"${LOG_MOCK}" 2>&1 &
MOCK_PID=$!

echo "[smoke] wait mock /api/tags…"
for i in {1..50}; do
  if curl -fsS "http://127.0.0.1:11434/api/tags" >/dev/null; then break; fi
  sleep 0.1
  [[ $i -eq 50 ]] && { echo "mock did not start"; tail -n 200 "${LOG_MOCK}" || true; exit 1; }
done

export HAUSKI_CHAT_UPSTREAM_URL="http://127.0.0.1:11434"
export HAUSKI_CHAT_MODEL="${MODEL}"

echo "[smoke] build & run hauski core…"
cargo build -q -p hauski-cli
RUST_LOG="${RUST_LOG:-warn}" "${ROOT}/target/debug/hauski-cli" serve --bind "${BIND}" >"${LOG_CORE}" 2>&1 &
CORE_PID=$!

echo "[smoke] wait /health…"
for i in {1..100}; do
  if curl -fsS "${HOST}/health" >/dev/null; then break; fi
  sleep 0.1
  [[ $i -eq 100 ]] && { echo "core did not start"; tail -n 200 "${LOG_CORE}" || true; exit 1; }
done

echo "[smoke] check /health"
curl -fsS "${HOST}/health" | grep -qi "ok"

echo "[smoke] check /v1/chat"
RESP="$(curl -sSf --max-time 10 -X POST "${HOST}/v1/chat" \
  -H 'Content-Type: application/json' \
  -d '{"messages":[{"role":"user","content":"Ping?"}]}')"

python3 - "$RESP" "$MODEL" <<'PY'
import json, sys
resp, model = sys.argv[1], sys.argv[2]
payload = json.loads(resp)
if payload.get("model") != model:
    print("unexpected model:", payload.get("model"), "expected:", model)
    raise SystemExit(1)
content = payload.get("content","")
if not content or "(mock)" not in content:
    print("unexpected content:", content)
    raise SystemExit(1)
PY

echo "[smoke] success."
