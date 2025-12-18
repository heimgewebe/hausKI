# Runbook: Upgradepfade

Status: _draft_ • Owner: Platform • Ziel: sichere, reproduzierbare Upgrades (App + Vendor + Policies)

## 1. Arten von Upgrades
- **App-Release**: neue Binaries (hauski-core, hauski-cli, indexd)
- **Vendor-Snapshot**: Aktualisierte Rust-Dependencies (`vendor/`)
- **Policies**: Änderungen an `policies/limits.yaml`, `routing.yaml`, `models.yaml`
- **Toolchain**: Rust/Python/UV (gemäß ADR-0001, `toolchain.versions.yml`)

## 2. Vorbedingungen (Preflight)
1. **CI grün**: `ci.yml` (fmt, clippy, tests, security)
2. **Vendor konsistent**:
   - `scripts/check-vendor.sh` muss sauber sein
   - ggf. `cargo vendor --locked` → Commit
3. **Dokumentation**:
   - `mkdocs` baut lokal (optional)
   - Changelog/Release-Notes vorhanden
4. **Policies validiert**:
   - Lint/Schema (falls vorhanden)
   - Budget-Folgen abgeschätzt

## 3. Ablauf – App-Release
1. **Build**
   ```bash
   cargo build --workspace --release --locked
   ```
2. **Smoke lokal**
   ```bash
   target/release/hauski-core >server.log 2>&1 &
   pid=$!; sleep 1
   curl -sf http://127.0.0.1:8080/ready
   curl -sf http://127.0.0.1:8080/health
   kill $pid || true
   ```
3. **Prometheus-/Health-Checks** (falls Stage-Env verfügbar)
4. **Rollout**
   - Blau/Grün oder Canary, je nach Umgebung
   - Beobachtung p95-Latenz (ms)/Fehlerraten 15–30 min
5. **Fallback**
   - Bei Regression: Rollback (letztes Release)

## 4. Ablauf – Vendor-Snapshot
1. **Update vorbereiten**
   ```bash
   rm -rf vendor/
   cargo vendor --locked --respect-source-config > /dev/null
   git add vendor
   ```
2. **Validierung**
   - `bash scripts/check-vendor.sh`
   - `cargo build --locked`
3. **Commit & PR**
   - PR-Title: `vendor: refresh snapshot (YYYY-MM-DD)`
   - `git commit -m "vendor: refresh snapshot"`
   - CI abwarten

## 5. Ablauf – Policies
1. **Änderung in Branch**
2. **Validierung**
   - Schema/Lint (sofern vorhanden), sonst Peer-Review
   - Im Zweifel Stage mit erhöhten Budgets testen
3. **Rollout**
   - Core neustarten oder Hot-Reload (wenn vorhanden)
   - p95-Latenz (ms)/Fehlerraten beobachten
4. **Rollback**
   - Vorherige Policy-Datei wiederherstellen

## 6. Toolchain-Anpassungen
Gemäß [ADR-0002](../adr/ADR-0002__toolchain-strategie.md):
- `toolchain.versions.yml` anpassen
- CI-Variablen (RUST_TOOLCHAIN, PYTHON_VERSION, UV_VERSION) ziehen zentral aus Datei
- DevContainer / `.wgx` synchronisieren

## 7. Checks nach dem Upgrade (Postflight)
- `/ready`, `/health` OK
- `/metrics`: p95-Latenz (ms) innerhalb Budget, Fehlerraten unverändert/verbessert
- Logs/Tracing ohne anhaltende Errors/Warnings
- Nutzer-Feedback/Monitoring in den ersten 24 h

## 8. Troubleshooting
- Build bricht: `--locked` prüfen, Vendor-Snapshot konsistent?
- Timeout/Errors: `routing.yaml` Änderungen/Downstream prüfen
- p95-Latenz (ms) hoch: Limits/Budgets temporär justieren, Regression identifizieren (Profiling/Tracing)

## 9. Referenzen
- [Incident-Response](./incident-response.md)
- ADR: [Toolchain-Strategie](../adr/ADR-0002__toolchain-strategie.md)
- Observability: `../modules/observability.md`

## 10. Checkliste
- [ ] Preflight-Prüfungen (CI, Vendor, Doku, Policies) abgeschlossen
- [ ] Rollout-Plan (App/Vendor/Policy/Toolchain) und Fallback geklärt
- [ ] Postflight-Checks (`/health`, `/ready`, `/metrics`, Logs) ohne Befunde
- [ ] Lessons Learned und Follow-ups im Ticket/Postmortem dokumentiert
