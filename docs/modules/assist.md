# Assist-Router (MVP)

Der Assist-Router wählt **heuristisch** zwischen einem _Code-Agent_ und einem _Knowledge-Agent_.
In Phase-2 dient dies als Gerüst, um später semantAH-Suche, Tools und Policies einzuhängen.

## Endpoint

`POST /assist`

**Request**
```json
{ "question": "Wie richte ich /docs ein?", "mode": "knowledge" }
```

**Response (MVP)**
```json
{
  "answer": "Router wählte knowledge. (MVP-Stub)",
  "citations": [{"title":"docs/api.md","score":0.83}],
  "trace": [{"step":"router","decision":"knowledge","reason":"heuristic"}],
  "latency_ms": 12
}
```

> Der `mode`-Parameter ist optional. Wenn gesetzt (`code|knowledge`), überschreibt er die Heuristik.

## Konfiguration

| Variable | Default | Zweck |
| --- | --- | --- |
| `HAUSKI_EVENT_SINK` | _(leer)_ | Wenn gesetzt, werden Events (JSONL) hierhin angehängt. |

## Nächste Schritte
- Knowledge-Agent mit **semantAH Top-K** und Zitaten anbinden (füllt `citations[]` mit Titeln/IDs & Scores).
- Code-Agent: Lint/Build/Run-Tools und Kurzdiagnosen.
- Events (`core.assist.request|response`) nach `contracts/events.schema.json`.
