# Memory (SQLite + TTL, MVP)

Ein schlanker Key/Value-Speicher mit optionalem TTL und Pin-Flag.
Nur sichtbar, wenn `HAUSKI_EXPOSE_CONFIG=true`.

## Endpunkte

| Route            | Methode | Body                                                           | Antwort                                                          |
|------------------|--------:|----------------------------------------------------------------|------------------------------------------------------------------|
| `/memory/get`    | POST    | `{ "key": "..." }`                                             | `{ "key":"...", "value": "...", "ttl_sec": 300, "pinned": false }` |
| `/memory/set`    | POST    | `{ "key":"...", "value":"...", "ttl_sec":300, "pinned":false }` | `{ "ok": true }`                                                 |
| `/memory/evict`  | POST    | `{ "key":"..." }`                                              | `{ "ok": true }`                                                 |

**TTL-Janitor:** löscht alle 60s Einträge, deren `updated_ts + ttl_sec` überschritten ist und `pinned=0`.

**Werteformat:** `value` wird als UTF-8 String übertragen und intern als `BLOB` gespeichert.
