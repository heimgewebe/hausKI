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
  mapfile -t pids_upstream < <(lsof -tiTCP:"${PORT}" -sTCP:LISTEN 2>/dev/null || true)
  if ((${#pids_upstream[@]})); then
    echo "• Beende Upstream-PIDs auf Port ${PORT}: ${pids_upstream[*]}"
    kill -TERM "${pids_upstream[@]}" 2>/dev/null || true
    sleep 1
    if kill -0 "${pids_upstream[@]}" 2>/dev/null; then
      echo "• Erzwinge Upstream-Kill (${pids_upstream[*]})"
      kill -KILL "${pids_upstream[@]}" 2>/dev/null || true
    fi
  fi
else
  echo "Hinweis: 'lsof' nicht gefunden – versuche pgrep-Fallback." >&2
  mapfile -t pids_upstream < <(pgrep -f "llama-server.*--port[= ]${PORT}" 2>/dev/null || true)
  if ! ((${#pids_upstream[@]})); then
    mapfile -t pids_upstream < <(pgrep -f "llama-server" 2>/dev/null || true)
  fi
  if ((${#pids_upstream[@]})); then
    echo "• Beende gefundene 'llama-server'-Prozesse: ${pids_upstream[*]}"
    kill -TERM "${pids_upstream[@]}" 2>/dev/null || true
    sleep 1
    if kill -0 "${pids_upstream[@]}" 2>/dev/null; then
      echo "• Erzwinge Upstream-Kill (${pids_upstream[*]})"
      kill -KILL "${pids_upstream[@]}" 2>/dev/null || true
    fi
  fi
fi

# Optional: hauski-core Prozesse (heuristisch)
mapfile -t pids_core < <(pgrep -f "hauski-cli.*serve" 2>/dev/null || true)
if ((${#pids_core[@]})); then
  echo "• Beende hauski-core PIDs: ${pids_core[*]}"
  kill -TERM "${pids_core[@]}" 2>/dev/null || true
  sleep 1
  if kill -0 "${pids_core[@]}" 2>/dev/null; then
    echo "• Erzwinge hauski-core-Kill (${pids_core[*]})"
    kill -KILL "${pids_core[@]}" 2>/dev/null || true
  fi
fi

echo "✓ Stop-Vorgang abgeschlossen. Prüfe ggf. mit: ps aux | egrep '(llama-server|hauski-cli)'"
