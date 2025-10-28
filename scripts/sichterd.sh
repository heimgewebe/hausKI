#!/usr/bin/env bash
set -Eeuo pipefail

LOCK_FILE="$HOME/sichter/review/sichterd.lock"
LOG_DIR="$HOME/sichter/review/hausKI"
mkdir -p "$(dirname "$LOCK_FILE")" "$LOG_DIR"

exec {fd}>"$LOCK_FILE" || exit 0
if ! flock -n "$fd"; then
  echo "sichterd: already running"
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
echo "sichterd: starting review run at $(date --iso-8601=seconds)" | tee -a "$log_file"

if ! command -v just >/dev/null 2>&1; then
  echo "sichterd: 'just' command not found" | tee -a "$log_file"
  exit 127
fi

if just --show review-quick >/dev/null 2>&1; then
  review_cmd=(just review-quick)
  echo "sichterd: running 'just review-quick'" | tee -a "$log_file"
elif just --show review >/dev/null 2>&1; then
  review_cmd=(just review)
  echo "sichterd: running 'just review' (fallback)" | tee -a "$log_file"
else
  echo "sichterd: no review recipe available → skipping" | tee -a "$log_file"
  exit 0
fi

set +e
"${review_cmd[@]}" 2>&1 | tee -a "$log_file"
status=${PIPESTATUS[0]}
set -e

echo "sichterd: finished with status $status" | tee -a "$log_file"
exit "$status"
