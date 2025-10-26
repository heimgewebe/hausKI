#!/usr/bin/env bash
set -euo pipefail

# Einfaches Logging + Mini-Rotation (bis 2 Backups) bei > 5 MB
# Usage:
#   source scripts/lib/logging.bash
#   init_logs                      # setzt LOG_DIR
#   rotate_log "$LOG_DIR/core.log"
#   run_with_log "cmd ..." "$LOG_DIR/core.log"

LOG_DIR_DEFAULT="${HOME}/.local/state/hauski/logs"
MAX_SIZE=$((5*1024*1024)) # 5 MB

init_logs() {
  LOG_DIR="${LOG_DIR_DEFAULT}"
  mkdir -p "${LOG_DIR}"
  export LOG_DIR
}

rotate_log() {
  local file="$1"
  [[ -f "$file" ]] || return 0
  local size
  size=$(wc -c < "$file" || echo 0)
  if (( size > MAX_SIZE )); then
    # shift: .1 -> .2
    [[ -f "${file}.1" ]] && mv -f "${file}.1" "${file}.2" || true
    mv -f "${file}" "${file}.1"
    : > "$file"
  fi
}

run_with_log() {
  local cmd="$1"
  local logfile="$2"
  rotate_log "$logfile"
  bash -lc "$cmd" >>"$logfile" 2>&1 &
  echo $!
}
