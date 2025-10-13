# Release-Checkliste

Dieses Dokument bündelt die Schritte, die vor einem offiziellen Release (Tag + Binary/Snapshot) durchzuführen sind.

## 1. Code einfrieren

- Offene PRs mergen oder verschieben.
- `just fmt`, `just lint`, `just test` laufen lassen – alle Checks müssen grün sein.
- Python-Hilfswerkzeuge mit `just py-lint`, `just py-test` verifizieren.

## 2. Version & Changelog

- Versionsbump in den relevanten `Cargo.toml`-Dateien vornehmen.
- Changelog-Abschnitt ergänzen (z. B. in `docs/canvas/CHANGELOG.md`, sofern vorhanden).
- Prüfen, ob Policies oder Modelle aktualisiert wurden und entsprechend dokumentiert sind.

## 3. Artefakte bauen

- `just build` für den vollständigen Workspace.
- `just vendor` und optional `just vendor-archive`, um reproduzierbare Snapshots zu liefern.
- Dokumentation: `just py-docs-build` (MkDocs im `site/`-Ordner aktualisiert).

## 4. Smoke-Tests

- Lokalen Core starten: `just run-core`.
- `/health`, `/ready`, `/metrics` abrufen; bei aktivierter Config-Freigabe zusätzlich `/config/*` prüfen.
- Optional: API-Spotchecks (`/index/upsert`, `/ask`) mit realen Payloads.

## 5. Release veröffentlichen

- Git-Tag setzen (`git tag -a vX.Y.Z -m "HausKI vX.Y.Z"`).
- Tag pushen (`git push origin vX.Y.Z`).
- Release-Notizen in GitHub erstellen, Vendor-Archiv und ggf. fertige Binaries anhängen.

## 6. Nachbereitung

- Monitoring-Dashboards (Prometheus/Grafana) auf neue Metriken prüfen.
- Offene Tickets für Nacharbeiten oder Hotfixes erfassen.

Mit dieser Checkliste bleibt der Weg von „grünem Branch“ zu „veröffentlichtem Snapshot“ reproduzierbar und auditiert.
