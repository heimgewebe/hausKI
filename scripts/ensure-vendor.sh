#!/usr/bin/env bash
set -euo pipefail

# ------------------------------------------------------------------------------
# ensure-vendor.sh  —  Robust vendoring helper for CI and local dev
#
# Features:
# - Honors NO_NETWORK=1 to strictly verify offline snapshot without touching network
# - NEUTRALIZE_PROXY=1 to temporarily clear proxy env during vendoring
# - Uses sparse registry protocol and versioned-dirs for stable snapshots
# - Ignores user-level cargo config (CARGO_NO_LOCAL_CONFIG=1)
# - Works with a companion check script: scripts/check-vendor.sh
# ------------------------------------------------------------------------------

# --- Locate repo root & script dir ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_ROOT}"

# Ensure Cargo ignores user-level config that could interfere with vendoring
export CARGO_NO_LOCAL_CONFIG=1

# --- Options (can be overridden via env) ---
NO_NETWORK="${NO_NETWORK:-0}"             # 1 = strictly offline verification only (no network calls)
NEUTRALIZE_PROXY="${NEUTRALIZE_PROXY:-1}" # 1 = temporarily clear proxy env during vendoring

# --- Logging & helpers ---
log() { printf "%s\n" "$*" >&2; }
die() { log "ERR: $*"; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "Fehlt: $1"; }

need cargo

# --- Paths & temp state ---
ROOT="${PROJECT_ROOT}"
CONFIG_PATH="$ROOT/.cargo/config.toml"
TMP_CONFIG=""
TMP_VENDOR=""
CONFIG_BACKUP=""

cleanup() {
  if [[ -n "${TMP_CONFIG}" && -f "${TMP_CONFIG}" ]]; then
    rm -f "${TMP_CONFIG}"
    TMP_CONFIG=""
  fi
  if [[ -n "${TMP_VENDOR}" && -d "${TMP_VENDOR}" ]]; then
    rm -rf "${TMP_VENDOR}"
    TMP_VENDOR=""
  fi
  if [[ -n "${CONFIG_BACKUP}" && -f "${CONFIG_BACKUP}" ]]; then
    mv -f "${CONFIG_BACKUP}" "${CONFIG_PATH}"
    CONFIG_BACKUP=""
  fi
}
trap cleanup EXIT

# --- Proxy management (only used when vendoring) ---
orig_http_proxy="${http_proxy:-}"
orig_https_proxy="${https_proxy:-}"
orig_HTTP_PROXY="${HTTP_PROXY:-}"
orig_HTTPS_PROXY="${HTTPS_PROXY:-}"
restore_proxy() {
  export http_proxy="${orig_http_proxy}"
  export https_proxy="${orig_https_proxy}"
  export HTTP_PROXY="${orig_HTTP_PROXY}"
  export HTTPS_PROXY="${orig_HTTPS_PROXY}"
}
neutralize_proxy() {
  export http_proxy=""
  export https_proxy=""
  export HTTP_PROXY=""
  export HTTPS_PROXY=""
}

# --- Simple presence checks (fast path) ---
has_lock() { [[ -f Cargo.lock ]]; }
has_vendor() {
  [[ -d vendor ]] || return 1
  [[ -f vendor/config.toml ]] && return 0
  [[ -d vendor/registry ]] && return 0
  compgen -G "vendor/*" >/dev/null 2>&1
}
missing_axum() {
  shopt -s nullglob
  shopt -s globstar
  for path in vendor/**/axum-*; do
    [[ -d "$path" ]] && return 1
  done
  return 0
}

# --- If offline verification is requested, do not touch the network ---
if [[ "${NO_NETWORK}" == "1" ]]; then
  log "NO_NETWORK=1 → Offline-Prüfung des bestehenden vendor/ Snapshots (keine Netz-Zugriffe)…"
  has_lock   || die "Cargo.lock fehlt im Offline-Modus."
  has_vendor || die "vendor/ fehlt im Offline-Modus."

  # Prefer a dedicated consistency check if available
  if [[ -x "${SCRIPT_DIR}/check-vendor.sh" ]]; then
    if ! bash "${SCRIPT_DIR}/check-vendor.sh"; then
      die "check-vendor.sh meldet Probleme im Offline-Modus."
    fi
  fi

  # Optional sanity: ensure at least one well-known dep is vendored if erwartet
  if missing_axum; then
    log "Hinweis: axum im vendor/ nicht gefunden. Falls axum erwartet wird, ist der Snapshot unvollständig."
  fi

  log "✅ Offline-Check ok."
  exit 0
fi

# --- Online (or network-allowed) vendoring path ---
# Fast path: if snapshot already valid, we’re done
if [[ -x "${SCRIPT_DIR}/check-vendor.sh" ]]; then
  if bash "${SCRIPT_DIR}/check-vendor.sh" >/dev/null 2>&1; then
    log "Vendor-Snapshot bereits vollständig. Nichts zu tun."
    exit 0
  fi
fi

log "Vendor snapshot unvollständig/fehlend; Cargo.lock aktualisieren und Snapshot regenerieren…"

# Ensure we have a lockfile (without performing broad upgrades)
if ! has_lock; then
  log "Erzeuge Cargo.lock via cargo generate-lockfile…"
  if [[ "${NEUTRALIZE_PROXY}" == "1" ]]; then neutralize_proxy; fi
  if ! cargo generate-lockfile > /dev/null 2>&1; then
    restore_proxy
    die "cargo generate-lockfile fehlgeschlagen."
  fi
  if [[ "${NEUTRALIZE_PROXY}" == "1" ]]; then restore_proxy; fi
fi

# Prepare temp vendor dir
if ! TMP_VENDOR="$(mktemp -d "${PWD}/vendor.tmp.XXXXXX")"; then
  die "mktemp für vendor.tmp fehlgeschlagen"
fi

# We prefer stable, reproducible layout:
#   --locked           → honor Cargo.lock strictly
#   --versioned-dirs   → include version in dir names to avoid collisions
args=(vendor --locked --versioned-dirs "${TMP_VENDOR}")

# Use a minimal temporary cargo config that enables sparse protocol and reduces network flakiness
if [[ -f "${CONFIG_PATH}" ]]; then
  CONFIG_BACKUP="${CONFIG_PATH}.ensure-vendor.bak"
  mv -f "${CONFIG_PATH}" "${CONFIG_BACKUP}"
fi
if TMP_CONFIG="$(mktemp)"; then
  cat >"${TMP_CONFIG}" <<'CFG'
[net]
git-fetch-with-cli = true
retry = 1

[registries.crates-io]
protocol = "sparse"
CFG
fi

# Perform vendoring with proxy neutralization if requested
if [[ "${NEUTRALIZE_PROXY}" == "1" ]]; then neutralize_proxy; fi
if ! CARGO_SOURCE_CRATES_IO_REPLACE_WITH="" CARGO_CONFIG="${TMP_CONFIG:-}" cargo "${args[@]}" > /dev/null 2>&1; then
  restore_proxy
  die "cargo vendor fehlgeschlagen."
fi
if [[ "${NEUTRALIZE_PROXY}" == "1" ]]; then restore_proxy; fi

# Atomically replace vendor/ with fresh snapshot
rm -rf vendor
mv "${TMP_VENDOR}" vendor
TMP_VENDOR=""

# Restore any pre-existing cargo config
if [[ -n "${CONFIG_BACKUP}" ]]; then
  mv -f "${CONFIG_BACKUP}" "${CONFIG_PATH}"
  CONFIG_BACKUP=""
fi
if [[ -n "${TMP_CONFIG}" ]]; then
  rm -f "${TMP_CONFIG}"
  TMP_CONFIG=""
fi

# Final verification
if [[ -x "${SCRIPT_DIR}/check-vendor.sh" ]]; then
  "${SCRIPT_DIR}/check-vendor.sh"
else
  # Minimal fallback check
  has_vendor || die "vendor/ nach vendoring nicht gefunden."
fi

# Optional visibility: basic sanity that a known dep exists, if applicable
if missing_axum; then
  log "Hinweis: axum wurde im vendor/ nicht gefunden."
  log "Prüfe ggf.: cargo tree -e features | grep -i axum"
fi

log "✅ vendor/ Snapshot steht."
