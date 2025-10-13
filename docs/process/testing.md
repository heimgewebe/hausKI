# Tests & Qualitätssicherung

HausKI kombiniert Rust-, Python- und ggf. Frontend-Komponenten. Die folgenden Schritte sichern Konsistenz vor Commits und Releases.

## Standard-Checkliste

1. `just fmt` – formatiert alle Rust-Crates via `cargo fmt`.
2. `just lint` – prüft Vendor-Locks, führt `cargo clippy` (mit `-D warnings`) sowie `cargo deny` aus.
3. `just build` – stellt sicher, dass alle Crates kompilieren.
4. `just test` – führt Workspace-Tests mit zusätzlichem Logging (`--nocapture`) aus.
5. `just py-lint` / `just py-fmt` – Linting & Formatierung der Python-Tools mittels `ruff`.
6. `just py-test` – optionale Python-Tests (`pytest`), sofern `tests/` vorhanden ist.

Diese Kommandos laufen auch in CI-Gates; lokal sollten sie vor Pull Requests durchlaufen werden.

## Schnelle Zyklen

- `just test-quick` eignet sich für schnelle Feedback-Schleifen: ruft `cargo test`, `pytest` und `npm test` (falls vorhanden) im stillen Modus auf.
- `just test-full` wiederholt die gleiche Matrix ohne Kurzschluss und dokumentiert dadurch fehlende Toolchains.

## Artefakte & Telemetrie

- `scripts/check-vendor.sh` verifiziert vor jedem Build/Test die Konsistenz des `vendor/`-Ordners.
- Prometheus-Metriken aus `core` dokumentieren Testzugriffe auf `/health`, `/ready` etc. und helfen, Budgetverletzungen früh zu erkennen.

## Fehlerbehebung

- Schlägt `cargo deny` wegen Lizenz-Konflikten fehl, passe `deny.toml` an oder pinne Abhängigkeiten.
- Bei fehlendem `vendor/`-Snapshot `just vendor` ausführen, bevor Tests wiederholt werden.
- Für reproduzierbare Python-Umgebungen: `just py-init` (uv-sync) vor dem ersten Lauf ausführen.
