#!/usr/bin/env bash
set -euo pipefail

# Start: (1) Upstream (llama.cpp --server) (optional) + (2) HausKI Core
# Features:
#   - OpenAI-kompatibler Upstream (PORT, MODEL, --upstream-url)
#   - tmux-Start (--tmux) mit Session "hauski"
#   - .env-Unterstützung (.env oder configs/.env)
#   - Logs (~/.local/state/hauski/logs) mit einfacher Rotation
#   - HAUSKI_FLAGS: chat_upstream_url wird automatisch gesetzt
#   - --no-upstream überspringt den lokalen llama.cpp-Start
# Usage:
#   scripts/start-all.sh --model ~/models/model.gguf [--port 8081]
#                        [--upstream-url http://host:port] [--tmux]
#                        [--no-upstream]

MODEL="${MODEL:-}"
PORT="${PORT:-8081}"
UPSTREAM_URL="${UPSTREAM_URL:-}"
USE_TMUX="${USE_TMUX:-0}"
NO_UPSTREAM="${NO_UPSTREAM:-0}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT}"

LLAMA_PID=""

cleanup() {
  local exit_code="${1:-0}"
  if [[ -n "${LLAMA_PID}" ]]; then
    if kill -0 "${LLAMA_PID}" 2>/dev/null; then
      echo "⏹ Down: llama-server (${LLAMA_PID})"
      kill -TERM "${LLAMA_PID}" 2>/dev/null || true
      if sleep 2 && kill -0 "${LLAMA_PID}" 2>/dev/null; then
        echo "• Erzwinge Beenden von llama-server (${LLAMA_PID})"
        kill -KILL "${LLAMA_PID}" 2>/dev/null || true
      fi
      wait "${LLAMA_PID}" 2>/dev/null || true
    fi
  fi
  return "${exit_code}"
}

cleanup_and_exit() {
  local exit_code="${1:-0}"
  cleanup "${exit_code}"
  exit "${exit_code}"
}

trap 'cleanup $?' EXIT
trap 'cleanup_and_exit 130' INT
trap 'cleanup_and_exit 143' TERM

# .env laden (falls vorhanden)
load_env() {
  local loaded=0
  if [[ -f "${ROOT}/.env" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "${ROOT}/.env"
    set +a
    loaded=1
  fi
  if [[ -f "${ROOT}/configs/.env" ]]; then
    set -a
    # shellcheck disable=SC1090
    source "${ROOT}/configs/.env"
    set +a
    loaded=1
  fi
  if [[ "$loaded" -eq 1 ]]; then
    echo "• .env geladen"
  fi
}

load_env
MODEL="${MODEL:-}"
PORT="${PORT:-8081}"
UPSTREAM_URL="${UPSTREAM_URL:-}"
USE_TMUX="${USE_TMUX:-0}"
NO_UPSTREAM="${NO_UPSTREAM:-0}"

normalize_bool() {
  local raw="${1:-0}"
  case "${raw}" in
    1|true|TRUE|True|yes|YES|Yes|on|ON|On)
      echo "1"
      ;;
    *)
      echo "0"
      ;;
  esac
}

USE_TMUX="$(normalize_bool "${USE_TMUX}")"
NO_UPSTREAM="$(normalize_bool "${NO_UPSTREAM}")"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --model) MODEL="${2:-}"; shift 2 ;;
    --port) PORT="${2:-}"; shift 2 ;;
    --upstream-url) UPSTREAM_URL="${2:-}"; shift 2 ;;
    --tmux) USE_TMUX="1"; shift ;;
    --no-upstream) NO_UPSTREAM="1"; shift ;;
    -h|--help)
      sed -n '3,/^$/p' "$0" | sed 's/^#\s\{0,1\}//'
      exit 0
      ;;
    *)
      echo "Unbekannte Option: $1" >&2
      exit 2
      ;;
  esac
done

source scripts/lib/logging.bash
init_logs
UPSTREAM_LOG="${LOG_DIR}/upstream.log"
CORE_LOG="${LOG_DIR}/core.log"
CONFIG_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/hauski"
mkdir -p "${CONFIG_DIR}"

DEFAULT_UPSTREAM_URL="http://127.0.0.1:${PORT}"
UPSTREAM_URL_RAW="${UPSTREAM_URL}"

if [[ -n "${UPSTREAM_URL}" && "${UPSTREAM_URL}" != "${DEFAULT_UPSTREAM_URL}" && "${NO_UPSTREAM}" = "0" ]]; then
  echo "ℹ️  Externe Upstream-URL erkannt – überspringe lokalen llama.cpp-Start. Nutze --no-upstream explizit, um das Verhalten festzulegen."
  NO_UPSTREAM="1"
fi

if [[ -z "${UPSTREAM_URL}" ]]; then
  UPSTREAM_URL="${DEFAULT_UPSTREAM_URL}"
fi

if [[ "${NO_UPSTREAM}" = "1" && -z "${UPSTREAM_URL_RAW}" ]]; then
  echo "⚠️  --no-upstream ohne --upstream-url – nutze ${UPSTREAM_URL}."
fi

if [[ "${NO_UPSTREAM}" = "1" ]]; then
  echo "⏭️  Lokaler Upstream-Start übersprungen – verwende Upstream ${UPSTREAM_URL}."
fi

if [[ -z "${MODEL}" && "${NO_UPSTREAM}" = "0" ]]; then
  echo "Fehler: --model <pfad/zum/model.gguf> ist erforderlich (oder --no-upstream setzen)." >&2
  exit 2
fi

join_cmd() {
  local -a parts=("$@")
  local joined=""
  local element
  for element in "${parts[@]}"; do
    joined+="$(printf '%q' "${element}") "
  done
  printf '%s' "${joined% }"
}

LLAMA_CMD=("llama-server" "--port" "${PORT}" "--model" "${MODEL}" "--ctx-size" "4096" "--batch-size" "512")
CORE_CMD=("just" "run-core")
if ! (command -v just >/dev/null 2>&1 && just --list 2>/dev/null | grep -qE '^\s*run-core\b'); then
  CORE_CMD=("cargo" "run" "-p" "hauski-cli" "--" "serve")
fi

rotate_log "${CORE_LOG}"
if [[ "${NO_UPSTREAM}" = "0" ]]; then
  rotate_log "${UPSTREAM_LOG}"
fi

if [[ -z "${HAUSKI_FLAGS-}" ]]; then
  HAUSKI_FLAGS="${CONFIG_DIR}/hauski-flags.yaml"
  old_umask=$(umask)
  umask 077
  printf 'chat_upstream_url: "%s"\n' "${UPSTREAM_URL}" >"${HAUSKI_FLAGS}"
  umask "${old_umask}"
  export HAUSKI_FLAGS
  echo "• HAUSKI_FLAGS Datei geschrieben: ${HAUSKI_FLAGS}"
else
  echo "• Nutze bestehendes HAUSKI_FLAGS=${HAUSKI_FLAGS}"
fi

echo "▶ Starte HausKI-Core mit HAUSKI_FLAGS='${HAUSKI_FLAGS}' …"

if [[ "${USE_TMUX}" = "1" ]]; then
  if ! command -v tmux >/dev/null 2>&1; then
    echo "tmux nicht gefunden. Installiere tmux oder lasse --tmux weg."
    exit 2
  fi
  echo "▶ Starte tmux-Session 'hauski' …"
  tmux kill-session -t hauski >/dev/null 2>&1 || true
  tmux new-session -d -s hauski
  tmux set-environment -t hauski HAUSKI_FLAGS "${HAUSKI_FLAGS}" >/dev/null 2>&1 || true
  if [[ "${NO_UPSTREAM}" = "0" ]]; then
    tmux rename-window -t hauski:1 'upstream'
    tmux send-keys -t hauski:upstream "$(join_cmd "${LLAMA_CMD[@]}") | tee -a $(printf '%q' "${UPSTREAM_LOG}")" C-m
    tmux new-window -t hauski -n 'core'
  else
    tmux rename-window -t hauski:1 'core'
  fi
else
  echo "▶ Upstream (llama.cpp) prüfen…"
  if [[ "${NO_UPSTREAM}" = "0" ]]; then
    if command -v llama-server >/dev/null 2>&1; then
      echo "• llama-server gefunden: $(command -v llama-server)"
      "${LLAMA_CMD[@]}" >>"${UPSTREAM_LOG}" 2>&1 &
      LLAMA_PID=$!
    else
      echo "⚠️  Kein 'llama-server' im PATH gefunden."
      echo "   Starte den Upstream bitte separat, z. B.:"
      echo "     llama-server --port ${PORT} --model '${MODEL}' --ctx-size 4096 --batch-size 512"
      echo "   oder (falls definiert):"
      echo "     just llama-server MODEL='${MODEL}' PORT='${PORT}'"
    fi
  fi
fi

wait_for_upstream() {
  local attempt
  for attempt in {1..30}; do
    if bash -lc "exec 3<>/dev/tcp/127.0.0.1/${PORT}" 2>/dev/null; then
      echo "✓ Upstream erreichbar auf 127.0.0.1:${PORT}"
      return 0
    fi
    sleep 0.3
  done

  echo "⚠️  Upstream auf Port ${PORT} nach 30 Versuchen nicht erreichbar." >&2
  return 1
}

if [[ "${NO_UPSTREAM}" = "0" ]]; then
  echo "⏳ Warte kurz, bis Port ${PORT} lauscht…"
  wait_for_upstream || true
fi

if [[ "${USE_TMUX}" = "1" ]]; then
  tmux send-keys -t hauski:core "$(join_cmd "${CORE_CMD[@]}") | tee -a $(printf '%q' "${CORE_LOG}")" C-m
  echo "tmux läuft. Attach mit: tmux attach -t hauski"
else
  (cd "${ROOT}" && "${CORE_CMD[@]}") >>"${CORE_LOG}" 2>&1
fi
