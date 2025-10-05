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
  if [ ! -d "vendor/${crate}" ]; then
    missing+=("${crate}")
  fi
done

if [ ${#missing[@]} -eq 0 ]; then
  exit 0
fi

missing_display=$(printf '%s ' "${missing[@]}")
missing_display=${missing_display% }

cat <<MSG
error: vendored crates missing: ${missing_display}

The workspace is configured to run fully offline via .cargo/config.toml.
Cargo will therefore look for every dependency inside the local vendor/
directory. When key crates are absent (e.g. axum, tokio, serde) the default
Cargo error message is misleading and does not explain how to fix the setup.

To populate vendor/ you can either:
  * run 'just vendor' (requires network access) and optionally 'just vendor-archive'
    to create a distributable tarball, or
  * download the prebuilt vendor snapshot from CI artifacts and extract it
    into vendor/.

If network access is unavailable in your environment, copy the archived
vendor tarball produced by a trusted build machine into this repository and
extract it manually, for example with:
  tar --zstd -xvf /path/to/hauski-vendor-snapshot.tar.zst -C vendor --strip-components=1

Once vendor/ contains the required crates, rerun this command.
MSG
exit 1
