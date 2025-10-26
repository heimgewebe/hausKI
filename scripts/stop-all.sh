#!/usr/bin/env bash
set -euo pipefail

# Stoppt laufende Upstream/Core-Prozesse.
# - Wenn tmux-Session 'hauski' existiert, wird sie beendet.
# - Sonst werden Prozesse über bekannte PIDs/Ports gesucht und beendet.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${ROOT}"

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
  if [[ "${loaded}" -eq 1 ]]; then
    echo "• .env geladen"
  fi
}

load_env

SESSION="hauski"

ARG_PORT="${1:-}"
ENV_PORT="${PORT-}"

if [[ -n "${ARG_PORT}" ]]; then
  PORT="${ARG_PORT}"
  PORT_SOURCE="Argument"
elif [[ -n "${ENV_PORT}" ]]; then
  PORT="${ENV_PORT}"
  PORT_SOURCE=".env"
else
  PORT="8081"
  PORT_SOURCE="Default"
fi

echo "• Verwende Port ${PORT} (${PORT_SOURCE})"

if command -v tmux >/dev/null 2>&1 && tmux has-session -t "${SESSION}" 2>/dev/null; then
  echo "⏹ tmux-Session '${SESSION}' wird beendet …"
  tmux kill-session -t "${SESSION}"
  exit 0
fi

echo "⏹ Versuche Prozesse ohne tmux zu beenden …"
# Versuche: Upstream auf PORT
if command -v lsof >/dev/null 2>&1; then
  PIDS=$(lsof -tiTCP:"${PORT}" -sTCP:LISTEN || true)
  if [[ -n "${PIDS:-}" ]]; then
    echo "• beende Upstream PIDs auf Port ${PORT}: ${PIDS}"
    kill -TERM ${PIDS} 2>/dev/null || true
    sleep 1
    if kill -0 ${PIDS} 2>/dev/null; then
      echo "• Erzwinge Upstream-Kill (${PIDS})"
      kill -KILL ${PIDS} 2>/dev/null || true
    fi
  fi
else
  echo "Hinweis: 'lsof' nicht gefunden – versuche pgrep-Fallback." >&2
  PIDS=$(pgrep -f "llama-server.*--port[= ]${PORT}" || pgrep -f "llama-server" || true)
  if [[ -n "${PIDS:-}" ]]; then
    echo "• beende gefundene 'llama-server'-Prozesse: ${PIDS}"
    kill -TERM ${PIDS} 2>/dev/null || true
    sleep 1
    if kill -0 ${PIDS} 2>/dev/null; then
      echo "• Erzwinge Upstream-Kill (${PIDS})"
      kill -KILL ${PIDS} 2>/dev/null || true
    fi
  fi
fi

# Optional: hauski-core Prozesse (heuristisch)
pids_core=$(pgrep -f "hauski-cli.*serve" || true)
if [[ -n "${pids_core}" ]]; then
  echo "• beende hauski-core PIDs: ${pids_core}"
  kill -TERM ${pids_core} 2>/dev/null || true
  sleep 1
  if kill -0 ${pids_core} 2>/dev/null; then
    echo "• Erzwinge hauski-core-Kill (${pids_core})"
    kill -KILL ${pids_core} 2>/dev/null || true
  fi
fi

echo "✓ Stop versucht. Prüfe ggf. mit: ps aux | egrep '(llama-server|hauski-cli)'"
