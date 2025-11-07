# Event-Schema

Dieses Dokument beschreibt das **einheitliche Ereignisformat** für HausKI. Ziel: identische Struktur für
Logs, Audits und eventuelle Message-Bus-Publikationen.

## Ziele
- **Stabile Felder** (`id`, `ts`, `kind`, `level`, `source`, `node`) für Korrelation & Filter.
- **Tracing-Anschluss** über `trace_id` / `span_id`.
- **Freie Labels** (`labels`) für schnelle Facettenfilter in Observability.
- **Nutzlast** in `data` ohne feste Struktur, aber validierbar.

## JSON-Schema
Das Schema liegt unter: [`contracts/events.schema.json`](../../contracts/events.schema.json) (Draft 2020-12) und ein Beispiel unter [`contracts/examples/event.sample.json`](../../contracts/examples/event.sample.json).

Wichtige Regeln:
- `id` ist ein **ULID** (26 Zeichen, sortierbar).
- `ts` ist **RFC 3339** (idealerweise UTC).
- `level ∈ {debug, info, warn, error, audit}`.
- Bei `level=audit` müssen **Labels vorhanden** sein (mindestens eins).

## Beispiel

```json
{ "id":"01JBDH3Q2H3Y7F3R0Q6G7V6K8Z",
  "ts":"2025-10-30T17:05:12Z",
  "version":"1.0.0",
  "kind":"core.chat.response",
  "level":"info",
  "source":"hauski-core",
  "node":"dev-laptop",
  "trace_id":"3f75c7f6c4a74ccd8d0a7a54a6bcb3f2",
  "span_id":"6f9b2a8d3c4e5f6a",
  "labels":{"model":"llama3.1-8b-q4","latency_ms":132,"http_status":200},
  "data":{"prompt_tokens":31,"completion_tokens":27,"content_preview":"Hallo! Wie kann ich helfen?","budget_ok":true}
}
```

## Emission (Richtlinie)
- **Eine Zeile pro Event** (JSONL), keine mehrzeiligen Strings – erleichtert Shipping.
- Felder in `labels` kurz und flach halten; Zahlen als `number` anstatt String.
- **Keine** sensitiven Rohdaten in `labels`; wenn nötig, **maskieren** und nur in `data`.
- Für Audits `level=audit` verwenden und aussagekräftige `labels` setzen (z. B. `policy`, `rule_id`).

## Validierung
Für lokale Checks kann z. B. `jsonschema` (Python) oder jede Draft-2020-12-fähige Lib genutzt werden:

```bash
uv run python - <<'PY'
import json, sys, pathlib
from jsonschema import validate, Draft202012Validator
root = pathlib.Path(__file__).resolve().parents[1]
schema = json.loads((root/"contracts/events.schema.json").read_text())
sample = json.loads((root/"contracts/examples/event.sample.json").read_text())
Draft202012Validator.check_schema(schema)
validate(sample, schema)
print("ok")
PY
```

## Kompatibilität
Das Schema ist **vorwärts kompatibel** durch `labels` und `data`. Breaking Changes nur über neue
`version` und Migrationshinweise.
