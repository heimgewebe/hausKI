# indexd: Vergessen, Decay & semantische Hygiene

## Überblick

Diese Implementierung fügt kontrollierte Vergessensmechanismen zu indexd hinzu, um semantische Drift zu vermeiden und die Gedächtnishygiene zu wahren.

## Implementierte Features

### 1. Time-Decay (Zeitlicher Relevanzverlust)

Dokumente verlieren kontinuierlich an Relevanz basierend auf ihrem Alter:

```rust
// Decay-Faktor wird automatisch beim Suchen angewendet
final_score = similarity_score × decay_factor
decay_factor = 0.5 ^ (age_seconds / half_life_seconds)
```

**Konfiguration:**
```rust
state.set_retention_config(
    "chronik".into(),
    RetentionConfig {
        half_life_seconds: Some(2592000), // 30 Tage
        ..Default::default()
    },
).await;
```

### 2. Namespace-Retention (Strukturelles Vergessen)

Pro-Namespace-Limits mit konfigurierbaren Purge-Strategien:

- `max_items`: Maximale Anzahl Dokumente
- `max_age_seconds`: Maximales Alter in Sekunden
- `purge_strategy`: `Oldest` oder `LowestScore`

**Beispiel:** Siehe `policies/indexd_retention.yaml`

### 3. Intentional Forget (Manuelles Vergessen)

Explizite Löschung über API mit Filtern:

```bash
# Alle Dokumente aus namespace "chronik" löschen
curl -X POST http://localhost:8080/index/forget \
  -H "Content-Type: application/json" \
  -d '{
    "filter": {
      "namespace": "chronik"
    },
    "reason": "Cleanup after migration",
    "confirm": true,
    "dry_run": false
  }'
```

**Filter-Optionen:**
- `namespace`: Filtere nach Namespace
- `older_than`: Filtere nach Zeitstempel (ISO 8601)
- `source_ref_origin`: Filtere nach Herkunft (z.B. "chronik", "osctx")
- `doc_id`: Filtere nach spezifischer Dokument-ID

### 4. Decay Preview (Dry-Run-Simulation)

Simuliere Decay-Effekte ohne Änderungen:

```bash
curl -X POST http://localhost:8080/index/decay/preview \
  -H "Content-Type: application/json" \
  -d '{"namespace": "chronik"}'
```

## API-Endpunkte

| Endpoint | Methode | Beschreibung |
|----------|---------|--------------|
| `/index/forget` | POST | Dokumente löschen (Bestätigung erforderlich) |
| `/index/retention` | GET | Aktive Retention-Policies anzeigen |
| `/index/decay/preview` | POST | Decay-Effekte simulieren |

## Sicherheitsgarantien

1. **Bestätigung erforderlich**: Nicht-dry-run-Löschungen erfordern `confirm: true`
2. **Dry-Run-Modus**: Alle Operationen unterstützen `dry_run: true`
3. **Strukturiertes Logging**: Alle Löschvorgänge werden geloggt
4. **Keine impliziten Löschungen**: Kein automatisches Vergessen bei Index-Rebuilds

## Tests

23 Tests decken alle Funktionen ab:

- **6 Unit-Tests**: Bestehende Funktionalität
- **9 Decay/Forget-Tests**: Neue Vergessenslogik
- **5 API-Tests**: HTTP-Endpunkte
- **3 Integration-Tests**: Bestehende Integration

Alle Tests: `cargo test --package hauski-indexd`

## Beispiel-Workflow

```rust
use hauski_indexd::{IndexState, RetentionConfig, PurgeStrategy};

// 1. State initialisieren
let state = IndexState::new(60, metrics_recorder);

// 2. Retention-Config setzen
state.set_retention_config(
    "chronik".into(),
    RetentionConfig {
        half_life_seconds: Some(2592000),  // 30 Tage
        max_items: Some(10000),
        max_age_seconds: Some(7776000),    // 90 Tage
        purge_strategy: Some(PurgeStrategy::Oldest),
    },
).await;

// 3. Dokumente einfügen
state.upsert(upsert_request).await;

// 4. Suchen (Decay wird automatisch angewendet)
let results = state.search(&search_request).await;

// 5. Decay-Preview abrufen
let preview = state.preview_decay(Some("chronik".into())).await;

// 6. Intentional Forget (mit Bestätigung)
let result = state.forget(
    ForgetFilter {
        namespace: Some("chronik".into()),
        older_than: Some(cutoff_timestamp),
        ..Default::default()
    },
    false, // nicht dry_run
).await;
```

## Zukünftige Erweiterungen

- **Metriken-Integration**: Prometheus-Metriken für Vergessensoperationen
- **Automatisches Purging**: Retention-Policies bei Upsert automatisch durchsetzen
- **Semantisches Vergessen**: Relevanzbasiertes Vergessen (niedriger Score über Zeit)
- **Persistenz**: SQLite-Persistenz für Retention-Configs

## Referenzen

- **Issue**: #2 (indexd – Vergessen, Decay & semantische Hygiene)
- **Dokumentation**: `docs/modules/indexd.md`
- **Policies**: `policies/indexd_retention.yaml`
- **Tests**: `crates/indexd/tests/`
