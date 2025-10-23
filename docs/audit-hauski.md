# hausKI – Architektur- und CI-Audit (Nachtrag Oktober 2025)

Dieses Dokument ergänzt die bestehende Analyse durch gezielte Empfehlungen zur Toolchain-Konsistenz, Workflow-Modularität und Review-Automatisierung.

## 1. Toolchain-Konsistenz über Sprachen hinweg
- Aktuell sind `rust-toolchain.toml`, `.wgx/profile.yml` (UV) und `pyproject.toml` nicht synchronisiert.
- Empfehlung: zentralen **`toolchain.versions.yml`** einführen, z. B.:

  ```yaml
  rust: "stable"      # oder feste Version wie "1.81.0"
  python: "3.12"
  uv: "0.7.0"
  ```

  CI-Workflows und DevContainer lesen diesen zentral aus, um Drift zu vermeiden.

## 2. Vendor-Check als Reusable Workflow
- Das lokale Script `scripts/check-vendor.sh` sollte in einen wiederverwendbaren Workflow überführt werden:

  `.github/workflows/reusable-validate-vendor.yml`

  ```yaml
  name: validate-vendor
  on: workflow_call
  jobs:
    vendor:
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v5
        - run: bash scripts/check-vendor.sh
  ```

  So können alle Repos (`heimlern`, `leitstand`, `aussensensor`) denselben Check einbinden.

## 3. Review-Zyklus in CI einbinden
- Die hausKI-spezifische Review-Pipeline (`.hauski-reports`, `flock`, `hauski-reviewd`) sollte einen optionalen CI-Workflow auslösen:

  `.github/workflows/review-cycle-check.yml`

  Dieser Workflow validiert, ob lokale Reports erzeugt wurden und das globale
  `~/.hauski/review/index.json` aktuell ist.

## 4. Priorität der Folgearbeiten
- 🔧 P0: Synchronisierung `toolchain.versions.yml` in alle Repos übernehmen
- 🧩 P1: Reusable Workflow für `check-vendor.sh`
- 📈 P2: Review-Zyklus als CI-Hook ergänzen

Diese Ergänzungen konsolidieren Toolchains, vereinheitlichen CI-Checks und
verankern den hausKI-Review-Zyklus direkt in der CI/CD-Pipeline.


---

## 5. Verknüpfte Architekturentscheidungen

Die vorliegende Audit-Erweiterung ist mit folgenden Architekturentscheidungen verknüpft:

| ADR | Titel | Status | Bezug |
|-----|--------|---------|--------|
| [ADR-0001](adrs/ADR-0001-toolchain-strategy.md) | Einheitliche Toolchain-Strategie | `Accepted` | Definiert zentrale Toolchain-Versionen für CI und lokale Entwicklung |

Diese Entscheidung dient als Basis für alle weiteren CI- und Container-Anpassungen.  
Künftige ADRs (z. B. für Speicher- und Kommunikationsschichten) sollten diesem Schema folgen.

## 6. Begleitender Smoke-Test

Als technische Verprobung des CI-Health-Flows wurde ein minimaler Integrationstest
unter `crates/core/tests/metrics_smoke.rs` hinzugefügt.  
Dieser prüft die Verfügbarkeit von `/metrics` und dient als Ankerpunkt für
spätere API-Tests (z. B. `/ask`, `/chat`, `/health`).

Beispiel:
```bash
HAUSKI_TEST_BASE_URL="http://127.0.0.1:8080" \
  cargo test -p hauski-core --test metrics_smoke -- --ignored --nocapture
```
