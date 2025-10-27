# HausKI Minimal Server

Dieses Projekt stellt einen minimalen Python/FastAPI-Dienst bereit, der die hausKI Policy-, Ingest- und Event-Log-Schnittstellen abbildet. Der Fokus liegt auf einem schnellen lokalen Start und einer erklärbaren Shadow-Mode-Policy.

## Status

* **Phase:** a) Verzeichnis & Requirements
* **Implementierung:** Struktur und Abhängigkeitsdefinitionen

## Nächste Schritte

1. Implementierung der FastAPI-Anwendung inklusive Shadow-Mode-Policy.
2. Aufbau der JSONL-Event-Logik.
3. Ergänzung automatisierter Tests und Beispielkonfigurationen.

## Entwicklung

Die Abhängigkeiten werden mit `uv` verwaltet. Das lokale Setup erfolgt via:

```bash
cd services/minimal-server
uv sync --frozen
```

Weitere Details folgen in den nächsten Implementierungsphasen.
