#!/usr/bin/env bash
set -euo pipefail
# Test-/Example-Ordner im vendor/ entfernen (nur Quellen, keine Build-Dateien)
if [ -d vendor ]; then
  find vendor -type d \( -name tests -o -name testdata -o -name examples \) -print0 | xargs -0 rm -rf
fi
