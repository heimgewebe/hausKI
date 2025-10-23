# ADR-0001: Einheitliche Toolchain-Strategie (Rust, Python, UV)

**Status:** Accepted  
**Datum:** 2025-10-23  
**Autor:** hausKI Team

## Kontext
In der Vergangenheit drifteten Toolchain-Versionen zwischen lokaler Entwicklung, CI
und DevContainer auseinander (Rust-Toolchain, Python, UV). Das führte zu
unreproduzierbaren Builds und inkonsistenten Abhängigkeiten.

## Entscheidung
Wir führen eine zentrale, maschinenlesbare Datei ein (z. B. `toolchain.versions.yml`),
aus der **CI-Workflows** und **DevContainer** ihre Versionen lesen. Ziel: keine
hartcodierten Versionsangaben mehr an mehreren Stellen.

Beispiel:
```yaml
rust: "stable"        # oder fix: "1.81.0"
python: "3.12"
uv: "0.7.0"
```

### CI
- GitHub Actions lesen (bei Bedarf via `actions/github-script` oder einfachem `yq/jq`)
  die Werte und setzen `RUST_TOOLCHAIN`, `PYTHON_VERSION`, `UV_VERSION`.
- Fallback bleibt `stable`, falls Datei nicht existiert.

### DevContainer/Local
- DevContainer (oder `.wgx/profile.yml`/`pyproject.toml`) übernimmt die zentralen
  Versionen und pinnt diese deterministisch.

## Konsequenzen
- **Pro:** Reproduzierbare Builds, weniger Aufwand bei Upgrades, einheitliche
  Dokumentation.
- **Contra:** Kleiner Initialaufwand zur Umstellung der Workflows und Container.

## Alternativen
- Versionsangaben weiterhin dezentral pflegen (verworfen wegen Drift-Risiko).

## Folgearbeiten
1. `toolchain.versions.yml` im Repo anlegen (Werte s. oben).
2. CI-Workflows so anpassen, dass sie daraus lesen (ohne harte Defaults).
3. DevContainer/`.wgx` an zentrale Datei koppeln.

