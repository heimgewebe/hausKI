#!/usr/bin/env bash
set -euo pipefail
mkdir -p configs
[[ -f configs/hauski.yml ]] || cat > configs/hauski.yml <<'YAML'
data_dir: "~/.local/state/hauski"
models_dir: "./models"
server:
  port: 8080
obsidian:
  vault_path: "~/vault-gewebe"
plugins:
  enabled:
    - obsidian_index
YAML
echo "bootstrap: configs/hauski.yml bereit."

mkdir -p policies
[[ -f policies/routing.yaml ]] || cat > policies/routing.yaml <<'YAML'
default: deny
allow: []
YAML
echo "bootstrap: policies/routing.yaml bereit."
