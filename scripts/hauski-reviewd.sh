#!/usr/bin/env bash
set -Eeuo pipefail

LOCK_FILE="$HOME/.hauski/review/hauski.lock"
LOG_DIR="$HOME/.hauski/review/hauski"
mkdir -p "$(dirname "$LOCK_FILE")" "$LOG_DIR"

exec {fd}>"$LOCK_FILE" || exit 0
if ! flock -n "$fd"; then
  echo "hauski-reviewd: already running"
  exit 0
fi

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR_DEFAULT="$(cd "$SCRIPT_DIR/.." && pwd)"
if [[ -n "${REPO_DIR:-}" && -d "${REPO_DIR}/.git" ]]; then
  cd "$REPO_DIR"
else
  cd "$REPO_DIR_DEFAULT"
fi

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  . ./.env || true
  set +a
fi

log_file="$LOG_DIR/review-$(date +%Y%m%d-%H%M%S).log"
echo "hauski-reviewd: starting review run at $(date --iso-8601=seconds)" | tee -a "$log_file"

if ! command -v just >/dev/null 2>&1; then
  echo "hauski-reviewd: 'just' command not found" | tee -a "$log_file"
  exit 127
fi

if just --show review-quick >/dev/null 2>&1; then
  review_cmd=(just review-quick)
  echo "hauski-reviewd: running 'just review-quick'" | tee -a "$log_file"
else
  review_cmd=(just review)
  echo "hauski-reviewd: running 'just review' (fallback)" | tee -a "$log_file"
fi

set +e
"${review_cmd[@]}" 2>&1 | tee -a "$log_file"
status=${PIPESTATUS[0]}
set -e

echo "hauski-reviewd: finished with status $status" | tee -a "$log_file"
exit "$status"
