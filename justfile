set shell := ["bash", "-euc"]

# Standard: Server starten (lokal)
default: run

# Server
run:
    cargo run -p hauski-cli -- serve

# Systemcheck
doctor:
    cargo run -p hauski-cli -- doctor

# Plugins
plugins-list:
    cargo run -p hauski-cli -- plugins list

plugins-obsidian-once:
    cargo run -p hauski-cli -- plugins run --id obsidian_index --once true

# Logs
logs:
    cargo run -p hauski-cli -- logs --lines 200

# Hygiene
fmt:
    cargo fmt --all

clippy:
    cargo clippy --all-targets --all-features -- -D warnings

# Dependency vulnerability and license compliance check
deny:
    cargo deny check
test:
    cargo test --workspace -- --nocapture
