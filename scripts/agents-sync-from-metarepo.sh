#!/usr/bin/env bash
set -euo pipefail

# Flaches Einblenden von templates/agent-kit aus dem metarepo.
# Strategien:
#  1) Lokales Checkout via METAREPO_DIR
#  2) Shallow Clone (sparse) via GIT_METAREPO_URL (fallback)
#
# Zielpfad: ./agent-kit (flache Kopie, keine Git-Historie)

here="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$here/.." && pwd)"
dest="$root/agent-kit"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

src_dir=""

if [[ -n "${METAREPO_DIR:-}" && -d "${METAREPO_DIR}/templates/agent-kit" ]]; then
  src_dir="${METAREPO_DIR}/templates/agent-kit"
else
  url="${GIT_METAREPO_URL:-https://github.com/heimgewebe/metarepo.git}"
  echo "→ Sparse-clone metarepo (nur templates/agent-kit) ..."
  git -c advice.detachedHead=false clone --depth=1 --no-tags --filter=blob:none --sparse "$url" "$tmp/metarepo" >/dev/null
  (cd "$tmp/metarepo" && git sparse-checkout set "templates/agent-kit")
  src_dir="$tmp/metarepo/templates/agent-kit"
fi

test -d "$src_dir" || {
  echo "Quelle nicht gefunden: $src_dir"
  exit 1
}

echo "→ Sync $src_dir  →  $dest"
rm -rf "$dest"
mkdir -p "$dest"
if command -v rsync >/dev/null 2>&1; then
  rsync -a --delete --exclude '.git/' "$src_dir/" "$dest/"
else
  echo "→ rsync nicht gefunden – fallback via tar"
  tar -C "$src_dir" --exclude='.git' -cf - . | tar -C "$dest" -xf -
fi

echo "✓ agent-kit aktualisiert."
