#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${PROJECT_ROOT}"

# Ensure Cargo ignores any user-level configuration that might interfere with the
# vendoring process. ShellCheck previously warned (SC2030/SC2031) that setting
# this variable inside subshells (e.g. within command invocations) would not be
# visible to later commands. We export it once here so every Cargo invocation in
# this script inherits the value without reassigning it in subshell context.
export CARGO_NO_LOCAL_CONFIG=1

if bash "${SCRIPT_DIR}/check-vendor.sh" >/dev/null 2>&1; then
  exit 0
fi

echo "Vendor snapshot incomplete or missing; refreshing Cargo.lock and regenerating vendor snapshot..."
mkdir -p vendor

GEN_LOCK_LOG="$(mktemp /tmp/cargo-generate-lockfile.XXXXXX.log)"
trap 'rm -f "${GEN_LOCK_LOG}" "${LOGFILE:-}"' EXIT

if ! cargo generate-lockfile > "${GEN_LOCK_LOG}" 2>&1; then
  cat "${GEN_LOCK_LOG}" >&2
  echo "Failed to refresh Cargo.lock via cargo generate-lockfile." >&2
  echo "Bitte aktualisiere die Lock-Datei manuell und versuche es erneut." >&2
  exit 1
fi

LOGFILE="$(mktemp /tmp/cargo-vendor.XXXXXX.log)"

if ! cargo vendor --locked > "${LOGFILE}" 2>&1; then
  cat "${LOGFILE}" >&2
  echo "Failed to regenerate vendor snapshot." >&2
  exit 1
fi

cat "${LOGFILE}"

# Re-run the check to ensure everything is now present.
"${SCRIPT_DIR}/check-vendor.sh"
