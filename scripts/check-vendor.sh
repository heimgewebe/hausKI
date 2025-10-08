#!/usr/bin/env bash
set -euo pipefail

# Minimal list of crates that must exist in the vendored directory for offline builds.
# We check for axum because it is the first dependency Cargo attempts to resolve
# during workspace builds. If it is missing, Cargo will fail with a confusing
# "no matching package" error because the workspace is configured to replace the
# crates.io registry with the local vendor tree.
REQUIRED_CRATES=(
  "axum"
  "tokio"
  "serde"
)

missing=()
for crate in "${REQUIRED_CRATES[@]}"; do
  if ! compgen -G "vendor/${crate}-*" > /dev/null && [ ! -d "vendor/${crate}" ]; then
    missing+=("${crate}")
  fi
done

if [ ${#missing[@]} -eq 0 ]; then
  exit 0
fi

missing_display=$(printf '%s ' "${missing[@]}")
missing_display=${missing_display% }

if [[ ${HAUSKI_ENFORCE_VENDOR:-0} != "0" ]]; then
  cat <<MSG
error: vendored crates missing: ${missing_display}

Der Workspace erzwingt Offline-Builds über '.cargo/config.toml'. Wenn Cargo
mit '--locked' läuft (z. B. in CI) und 'Cargo.lock' nicht aktuell ist, schlägt
'cargo vendor' mit der Meldung "the lock file … needs to be updated but --locked
was passed" fehl. Erneuere deshalb zuerst die Lock-Datei ('cargo generate-lockfile'
oder 'cargo update') und führe anschließend 'cargo vendor' aus.

Ohne Netzwerkzugang kannst du den vorbereiteten Tarball entpacken, beispielsweise
mit:
  tar --zstd -xvf /path/to/hauski-vendor-snapshot.tar.zst -C vendor --strip-components=1

Sobald 'vendor/' vollständig ist, erneut ausführen.
MSG
  exit 1
fi

cat <<MSG
warning: vendored crates missing: ${missing_display}

Cargo fällt nun auf crates.io zurück. Setze HAUSKI_ENFORCE_VENDOR=1, wenn der
Build zwingend offline erfolgen muss.
MSG
