#!/usr/bin/env bash
set -euo pipefail

# Search the repository for any remaining references to the old project name
# "leitstand" (case-insensitive). The check skips the git directory and uses an
# allowlist for known, intentional mentions (e.g., the separate Leitstand UI).
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

cd "$REPO_ROOT"

ALLOW_GLOBS=(
  '!tools/check_leitstand_references.sh'
  '!docs/audit-hauski.md'
  '!**/leitstand-ui/**'
)

RG_ARGS=("-uuu" "-i" "--hidden" "--glob" "!.git/*")
for g in "${ALLOW_GLOBS[@]}"; do
  RG_ARGS+=("--glob" "$g")
done

# ripgrep returns exit code 1 when no matches are found, which is fine for us
# but we need to normalize it to 0 to avoid failing the script when the repo is clean.
if rg "${RG_ARGS[@]}" 'leitstand' >/tmp/leitstand_refs.txt; then
  echo "Unexpected references to 'leitstand' were found:" >&2
  cat /tmp/leitstand_refs.txt >&2
  exit 1
else
  # Exit code 1 from rg means no matches
  rm -f /tmp/leitstand_refs.txt
  echo "No remaining 'leitstand' references found." >&2
  exit 0
fi
