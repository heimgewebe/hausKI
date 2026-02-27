# HausKI Shadow Policy API

Dieses Projekt stellt einen minimalen Python/FastAPI-Dienst bereit, der die hausKI Policy-, Ingest- und Event-Log-Schnittstellen abbildet. Der Fokus liegt auf einem schnellen lokalen Start und einer erklärbaren Shadow-Mode-Policy.

## Status

* **Phase:** b) Implementiert & Konsolidiert
* **Implementierung:** FastAPI-Anwendung mit Shadow-Mode-Policy und JSONL-Event-Logik.

## Features

- **Policy Decisions:** `/v1/policy/decide` für zeitbasierte Shadow-Empfehlungen.
- **Metrics Ingest:** `/v1/ingest/metrics` zur Aufnahme von Systemmetriken.
- **Feedback:** `/v1/policy/feedback` für das Sammeln von Nutzer-Feedback zu Entscheidungen.
- **Event Logging:** Automatische Speicherung aller Ereignisse als JSONL unter `~/.hauski/events/`.

## Entwicklung

Die Abhängigkeiten werden mit `uv` verwaltet. Das lokale Setup erfolgt via:

```bash
cd services/policy_shadow
uv sync --frozen
```

### Starten

Vom Root-Verzeichnis aus:

```bash
just shadow
```

Oder direkt:

```bash
uv run uvicorn services.policy_shadow.app:app --reload --port 8085
```
