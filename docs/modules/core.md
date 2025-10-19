# Core

Der `core`-Dienst bildet die öffentliche HTTP/API-Schicht von HausKI. Er verbindet Policies, Index und Modellaufrufe über `axum` und stellt dabei Telemetrie- und Governance-Funktionen bereit.

## Verantwortung

- Startet den Axum-Server (`main.rs`) mit konfigurierbarer Bind-Adresse und CORS-Headern.
- Lädt Limits, Modellkatalog, Routing- und Feature-Flags aus YAML-Dateien (`config.rs`).
- Orchestriert den eingebetteten `indexd`-State und exportiert `/index`-Routen.
- Erzwingt Latenzbudgets via `tower::ServiceBuilder` (Timeout + Concurrency-Limit) und schreibt Metriken nach Prometheus (`lib.rs`).

## Konfiguration

| Variable | Default | Beschreibung |
| --- | --- | --- |
| `HAUSKI_BIND` | `127.0.0.1:8080` | Bind-Adresse; Loopback-Pflicht sobald `HAUSKI_EXPOSE_CONFIG=1`. |
| `HAUSKI_LIMITS` | `./policies/limits.yaml` | Budget- und Latenzgrenzen. |
| `HAUSKI_MODELS` | `./configs/models.yml` | Modell- und Quantisierungsprofile. |
| `HAUSKI_ROUTING` | `./policies/routing.yaml` | Freigegebene Egress-Ziele und Strategien. |
| `HAUSKI_FLAGS` | `./configs/flags.yaml` | Feature-Flags für experimentelle Pfade. |
| `HAUSKI_ALLOWED_ORIGIN` | `http://127.0.0.1:8080` | CORS-Allow-Header. |
| `HAUSKI_EXPOSE_CONFIG` | `false` | Schaltet schreibgeschützte Config-Endpunkte frei (nur auf Loopback!). |

## Endpunkte

| Route | Methode | Zweck |
| --- | --- | --- |
| `/health` | GET | Liveness; zählt Telemetrie und prüft Index-Limits. |
| `/healthz` | GET | Lightweight-Probe für Load-Balancer. |
| `/ready` | GET | Readiness; aktiv nach erfolgreichem Boot. |
| `/metrics` | GET | Prometheus-Metriken inkl. HTTP-Zählern und Histogrammen. |
| `/ask` | GET | Beispiel-Endpoint für orchestrierte Anfragen (Ask-Flow). |
| `/v1/chat` | POST | Chat-Stub (Antwort: `501 Not Implemented`, JSON-Schema sichtbar). |
| `/index/upsert` | POST | Dokument-Chunks registrieren (weitergereicht an `indexd`, leere/fehlende Namespaces → `default`). |
| `/index/search` | POST | Volltext-/Substring-Suche gegen den In-Memory-Index (leere/fehlende Namespaces → `default`). |
| `/docs`, `/docs/openapi.json` | GET | Menschliche bzw. maschinenlesbare API-Dokumentation. |
| `/config/*` | GET | Optional freigeschaltete Config-Inspektion (Limits, Models, Routing). |

Die `/index/*`-Routen stammen aus `hauski-indexd` und nutzen denselben Metrics-Recorder, damit Budgetverletzungen zentral sichtbar sind.

## Typischer Workflow

1. Konfiguration per YAML anpassen (Modelle, Limits, Routing).
2. Dienst starten: `cargo run -p hauski-cli -- serve` oder `just run-core`.
3. Health/Ready prüfen (`curl http://127.0.0.1:8080/healthz`).
4. Index mit Chunks füllen (`POST /index/upsert`), danach `POST /ask` für Retrieval-gestützte Antworten testen.
5. `/v1/chat` per `POST` aufrufen (erwarteter Status: `501`), sobald die LLM-Anbindung aktiv ist, liefert dieser Endpoint Antworten.
6. Observability über `/metrics` oder Prometheus-Scrape einbinden.

## Sicherheit & Governance

- `EgressGuard` erlaubt nur explizit whiteliste Ziele und loggt Verstöße.
- Feature-Flags deaktivieren riskante Adapter standardmäßig.
- Readiness wird erst nach vollständigem Boot gesetzt, damit Orchestratoren korrekt warten.

Weiterführende Details entnimmst du dem Quellcode der `core`-Crate sowie dem Stack-Dokument.
