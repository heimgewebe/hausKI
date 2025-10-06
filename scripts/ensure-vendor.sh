#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${PROJECT_ROOT}"

if bash "${SCRIPT_DIR}/check-vendor.sh" >/dev/null 2>&1; then
  exit 0
fi

echo "Vendor snapshot incomplete or missing; refreshing Cargo.lock and regenerating vendor snapshot..."
mkdir -p vendor

GEN_LOCK_LOG="$(mktemp /tmp/cargo-generate-lockfile.XXXXXX.log)"
trap 'rm -f "${GEN_LOCK_LOG}" "${LOGFILE:-}"' EXIT

if ! (
  export CARGO_NO_LOCAL_CONFIG=1
  cargo generate-lockfile > "${GEN_LOCK_LOG}" 2>&1
); then
  cat "${GEN_LOCK_LOG}" >&2
  echo "Failed to refresh Cargo.lock via cargo generate-lockfile." >&2
  echo "Bitte aktualisiere die Lock-Datei manuell und versuche es erneut." >&2
  exit 1
fi

LOGFILE="$(mktemp /tmp/cargo-vendor.XXXXXX.log)"

if ! (
  export CARGO_NO_LOCAL_CONFIG=1
  cargo vendor --locked > "${LOGFILE}" 2>&1
); then
  cat "${LOGFILE}" >&2
  echo "Failed to regenerate vendor snapshot." >&2
  exit 1
fi

cat "${LOGFILE}"

# Re-run the check to ensure everything is now present.
"${SCRIPT_DIR}/check-vendor.sh"
