#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${PROJECT_ROOT}"

if bash "${SCRIPT_DIR}/check-vendor.sh" >/dev/null 2>&1; then
  exit 0
fi

echo "Vendor snapshot incomplete or missing; regenerating via cargo vendor..."
mkdir -p vendor

if ! (
  export CARGO_NO_LOCAL_CONFIG=1
  cargo vendor --locked > /tmp/cargo-vendor.log 2>&1
); then
  cat /tmp/cargo-vendor.log >&2
  echo "Failed to regenerate vendor snapshot." >&2
  exit 1
fi

cat /tmp/cargo-vendor.log

# Re-run the check to ensure everything is now present.
"${SCRIPT_DIR}/check-vendor.sh"
