#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "Usage: $0 <patch-file>" >&2
  exit 1
fi

PATCHFILE="$1"

# Apply the patch while excluding Cargo.lock to avoid conflicts with generated files.
git apply --reject --whitespace=fix --exclude=Cargo.lock "$PATCHFILE"

# Regenerate the lockfile to capture any dependency updates required by the patch.
cargo update

# Stage all changes produced by the patch and cargo update.
git add -u

echo "✅ Patch applied (without Cargo.lock). Lockfile regenerated."
