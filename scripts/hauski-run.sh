#!/usr/bin/env bash
set -Eeuo pipefail
IFS=$'\n\t'

# HausKI Local Runner
# - prüft/liest Upstream (Ollama) + Modell aus ENV oder configs/flags.yaml
# - räumt Port 8080
# - startet hauski-cli im Vordergrund (Logs: ~/hauski-api.log)

REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
LOG="${HOME}/hauski-api.log"
FLAGS_FILE="${REPO_DIR}/configs/flags.yaml"

err() {
  printf "\033[1;31m[err]\033[0m %s\n" "$*" >&2
  exit 1
}
inf() { printf "\033[1;34m[info]\033[0m %s\n" "$*"; }

command -v curl >/dev/null 2>&1 || err "curl wird benötigt, aber nicht gefunden"

# --- 1) Upstream & Modell ermitteln (ENV > flags.yaml > Defaults) ---
URL="${HAUSKI_CHAT_UPSTREAM_URL:-}"
MODEL="${HAUSKI_CHAT_MODEL:-}"

# aus flags.yaml ziehen, wenn ENV leer
if [[ -z "${URL}" && -f "${FLAGS_FILE}" ]]; then
  URL="$(awk -F': *' '/^chat_upstream_url:/ {print $2}' "${FLAGS_FILE}" | tr -d '"' || true)"
  if [[ "${URL}" == "null" ]]; then URL=""; fi
fi
if [[ -z "${MODEL}" && -f "${FLAGS_FILE}" ]]; then
  MODEL="$(awk -F': *' '/^chat_model:/ {print $2}' "${FLAGS_FILE}" | tr -d '"' || true)"
  if [[ "${MODEL}" == "null" ]]; then MODEL=""; fi
fi

# konservativer Default für URL; Modell lieber explizit
URL="${URL:-http://127.0.0.1:11434}"
[[ -n "${MODEL}" ]] || err "Kein Modell gesetzt. Exportiere HAUSKI_CHAT_MODEL oder setze 'chat_model:' in ${FLAGS_FILE}"

inf "Upstream: ${URL}"
inf "Modell:   ${MODEL}"

# --- 2) Ollama prüfen und Modell bereitstellen ---
if ! curl -fsS "${URL%/}/api/tags" >/dev/null 2>&1; then
  inf "Ollama scheint nicht zu laufen – versuche Systemdienst zu starten…"
  if command -v systemctl >/dev/null 2>&1; then
    if ! systemctl is-active --quiet ollama; then
      sudo systemctl start ollama || true
    fi
  fi
  sleep 1
fi
curl -fsS "${URL%/}/api/tags" >/dev/null 2>&1 || err "Kein Kontakt zu ${URL}. Läuft Ollama?"

if command -v ollama >/dev/null 2>&1; then
  if ! ollama show "${MODEL}" >/dev/null 2>&1; then
    inf "Pull ${MODEL}…"
    ollama pull "${MODEL}"
  fi
fi

# --- 3) Port 8080 freiräumen ---
if command -v lsof >/dev/null 2>&1; then
  PIDS="$(lsof -ti:8080 || true)"
else
  PIDS="$(ss -lntp | awk '/:8080 /{print $NF}' | sed -E 's/.*pid=([0-9]+).*/\1/' || true)"
fi
if [[ -n "${PIDS:-}" ]]; then
  inf "Räume 8080 (kill ${PIDS})…"
  kill ${PIDS} 2>/dev/null || true
  sleep 1
  kill -9 ${PIDS} 2>/dev/null || true
fi

# --- 4) Starten ---
cd "${REPO_DIR}"
export HAUSKI_CHAT_UPSTREAM_URL="${URL}"
export HAUSKI_CHAT_MODEL="${MODEL}"
inf "Starte HausKI… (Logs: ${LOG})"
RUST_LOG="${RUST_LOG:-info,hauski_core=debug}" \
  cargo run -p hauski-cli -- serve 2>&1 | tee -a "${LOG}"
