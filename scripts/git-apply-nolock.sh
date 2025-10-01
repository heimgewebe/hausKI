#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 <patch-file>" >&2
  exit 1
}

if [[ $# -ne 1 ]]; then
  usage
fi

patchfile="$1"

if [[ ! -f "$patchfile" ]]; then
  echo "Patch file '$patchfile' not found." >&2
  exit 1
fi

# Apply the patch while excluding Cargo.lock to avoid conflicts with generated files.
git apply --reject --whitespace=fix --exclude=Cargo.lock "$patchfile"

# Regenerate the lockfile to capture any dependency updates required by the patch.
cargo update

# Stage all changes produced by the patch and cargo update.
git add -u

git commit -m "Apply patch '$patchfile' (excluding Cargo.lock) and refresh lock"

echo "✅ Patch applied (without Cargo.lock). Lockfile regenerated and committed."
