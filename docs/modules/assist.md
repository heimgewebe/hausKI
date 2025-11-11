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
  "citations": [],
  "trace": [{"step":"router","decision":"knowledge"}],
  "latency_ms": 12
}
```

> Der `mode`-Parameter ist optional. Wenn gesetzt (`code|knowledge`), überschreibt er die Heuristik.

### Code-Pfad (Stub)
Bis der Code-Agent angebunden ist, liefert der Router für `mode=code` (oder heuristisch erkannten Code)
einen `501 Not Implemented` mit kurzer Diagnose im `trace`:
```json
{
  "answer": "CodeAgent ist noch nicht verdrahtet (501).",
  "citations": [],
  "trace": [{
    "step":"code_stub",
    "decision":"code",
    "language_guess":"python",
    "next":"not_implemented",
    "advice":"Prüfe Traceback-Root-Cause; venv/uv nutzen, Abhängigkeiten synchronisieren."
  }],
  "latency_ms": 7
}
```

## Konfiguration

| Variable | Default | Zweck |
| --- | --- | --- |
| `HAUSKI_EVENT_SINK` | _(leer)_ | Wenn gesetzt, werden Events (JSONL) hierhin angehängt. |
+| `HAUSKI_INTERNAL_BASE` | `http://127.0.0.1:8080` | Basis-URL für interne HTTP-Aufrufe (hier: `/index/search`). |

## Nächste Schritte
- Knowledge-Agent mit **semantAH Top-K** und Zitaten anbinden (füllt `citations[]` mit Titeln/IDs & Scores).
- Code-Agent: Lint/Build/Run-Tools und Kurzdiagnosen.
- Events (`core.assist.request|response`) nach `contracts/events.schema.json`.

## Guards & Limits

| Variable | Default | Zweck |
| --- | --- | --- |
| `HAUSKI_ASSIST_ENABLED` | `true` | Globales Feature-Gate. Wenn `false`, liefert `/assist` `503`. |
| `HAUSKI_ASSIST_MAX_PER_MIN` | `60` | Globales Prozess-Limit (Requests/Minute). Bei Überschreitung `429` mit `trace.retry_after_sec`. |
