# Decision-Weighting in indexd

## Überblick

Decision-Weighting ist ein deterministischer, dokumentierter Layer zwischen semantischer Suche und Entscheidungsfindung. 
Statt Treffer nur nach Ähnlichkeit zu sortieren, berücksichtigt hausKI drei Faktoren:

1. **Trust** – Vertrauen in die Quelle
2. **Recency** – Aktualität des Dokuments
3. **Context** – Kontext-spezifische Relevanz

**Formel:**
```
final_score = similarity × trust_weight × recency_weight × context_weight
```

> **Hinweis zum Status:**
> Die Gewichtungslogik lädt Policies beim Start aus `policies/trust.yaml` und `policies/context.yaml`.
> **Validierung:** Beim Laden werden Gewichte ≤ 0.0 und fehlende Pflichtfelder (z. B. "high", "default") abgelehnt.
> **Fallback:** Bei Validierungsfehlern oder fehlenden Dateien werden **sichere harte Defaults** verwendet.
> **Hash-Prüfung:** Der Hash der geladenen Policies wird in `/index/stats` als `policy_hash` zurückgegeben, um Drift zu erkennen.

## Trust-Gewichtung

Basiert auf dem `TrustLevel` der Quelle (konfigurierbar in `policies/trust.yaml`):

| Trust Level | Weight | Typische Quellen |
|-------------|--------|------------------|
| High        | 1.0    | chronik events, verifizierte interne Quellen |
| Medium      | 0.7    | OS-Kontext, Anwendungslogs |
| Low         | 0.3    | Externe Quellen, User-Input, Tool-Output |

### Beispiel

```rust
// Chronik-Event (High Trust)
source_ref: SourceRef {
    origin: "chronik",
    id: "event-123",
    trust_level: TrustLevel::High,  // weight: 1.0
    ...
}

// User-Input (Low Trust)
source_ref: SourceRef {
    origin: "user",
    id: "input-456",
    trust_level: TrustLevel::Low,   // weight: 0.3
    ...
}
```

Bei identischer Similarity und gleichem Alter wird der Chronik-Treffer 3× höher gewertet als User-Input.

## Recency-Gewichtung

Verwendet die bestehende Time-Decay-Logik (exponentieller Abfall basierend auf `ingested_at`).

**Konfiguration** erfolgt über `RetentionConfig.half_life_seconds`:

```rust
// Beispiel: 7 Tage Half-Life
RetentionConfig {
    half_life_seconds: Some(604800),  // 7 Tage
    ...
}
```

**Berechnung:**
```
recency_weight = 0.5^(age_seconds / half_life_seconds)
```

### Beispiel

Mit `half_life_seconds = 604800` (7 Tage):

| Alter | Recency Weight |
|-------|----------------|
| 0d    | 1.0            |
| 7d    | 0.5            |
| 14d   | 0.25           |
| 21d   | 0.125          |

Alte Wahrheiten bleiben sichtbar, aber leise.

## Context-Gewichtung

Passt Gewichtung basierend auf Namespace, Origin und Intent-Profil an.
Diese Logik wird nun dynamisch aus `policies/context.yaml` geladen.

**Logik:**
1. Prüfe Gewichtung für **Namespace**.
2. Wenn Namespace `default` ist, prüfe Gewichtung für **Origin** (semantische Quelle).
3. Fallback auf Profile-Default.

**Profile** sind in `policies/context.yaml` definiert:

### Profile

#### `incident_response` – Incident Response
Priorisiert frische Chronik-Events:

| Namespace | Weight |
|-----------|--------|
| chronik   | 1.2    |
| osctx     | 1.0    |
| insights  | 0.8    |
| code/docs | 0.5    |

#### `code_analysis` – Code-Analyse
Priorisiert Docs und Code:

| Namespace | Weight |
|-----------|--------|
| docs      | 1.2    |
| code      | 1.2    |
| osctx     | 0.8    |
| chronik   | 0.6    |

#### `reflection` – Reflexion
Priorisiert Insights:

| Namespace | Weight |
|-----------|--------|
| insights  | 1.2    |
| chronik   | 1.0    |
| osctx     | 0.8    |
| code/docs | 0.5    |

### Beispiel

```bash
# Ohne Context-Profile (balanced)
curl -X POST http://localhost:8080/index/search \
  -H 'Content-Type: application/json' \
  -d '{
    "query": "security update",
    "namespace": "default"
  }'

# Mit Context-Profile
curl -X POST http://localhost:8080/index/search \
  -H 'Content-Type: application/json' \
  -d '{
    "query": "security update",
    "namespace": "default",
    "context_profile": "incident_response"
  }'
```

## Transparenz: Weight Breakdown

Mit `include_weights: true` werden die einzelnen Gewichte zurückgegeben:

```bash
curl -X POST http://localhost:8080/index/search \
  -H 'Content-Type: application/json' \
  -d '{
    "query": "system error",
    "namespace": "chronik",
    "include_weights": true
  }'
```

**Response:**
```json
{
  "matches": [
    {
      "doc_id": "event-123",
      "namespace": "chronik",
      "score": 0.84,
      "weights": {
        "similarity": 0.85,
        "trust": 1.0,
        "recency": 0.95,
        "context": 1.0
      },
      ...
    }
  ]
}
```

**Interpretation:**
- `similarity: 0.85` – Text-Ähnlichkeit
- `trust: 1.0` – High Trust (chronik)
- `recency: 0.95` – Fast neu (leichter Decay)
- `context: 1.0` – Default Profile (kein Boost)
- `score: 0.84` = 0.85 × 1.0 × 0.95 × 1.0

## Use Cases

### 1. Incident Response

Bei einem Incident sind **frische Chronik-Events** am wichtigsten:

```json
{
  "query": "connection timeout",
  "context_profile": "incident_response",
  "include_weights": true
}
```

→ Chronik-Events (trust: 1.0, context: 1.2) werden gegenüber alten Code-Snippets (trust: 0.7, context: 0.5) bevorzugt.

### 2. Code Review

Bei Code-Analyse sind **Docs und Code** wichtiger als Events:

```json
{
  "query": "authentication flow",
  "namespace": "code",
  "context_profile": "code_analysis"
}
```

→ Code (context: 1.2) und Docs (context: 1.2) werden priorisiert.

### 3. Reflexion / Insights

Bei Reflexionen sind **Insights** wichtiger als Events:

```json
{
  "query": "deployment patterns",
  "context_profile": "reflection"
}
```

→ Insights (context: 1.2) werden höher gewertet als Chronik (context: 1.0).

## Fallstricke

### 1. Trust-Level nicht setzen
❌ **Falsch:**
```rust
source_ref: None  // Error: source_ref is required
```

✅ **Richtig:**
```rust
source_ref: Some(SourceRef {
    origin: "chronik",
    id: "event-123",
    trust_level: TrustLevel::High,
    ...
})
```

**Hinweis:** Falls `source_ref` fehlt (z.B. bei Legacy-Daten), wird ein **Medium Trust (0.7)** als sicherer Default angenommen.

### 2. Falsche Context-Profile-Namen
❌ **Falsch:**
```json
{
  "context_profile": "security_check"  // Unbekanntes Profil
}
```

→ Verwendet Default-Gewichtung (1.0)

✅ **Richtig:**
```json
{
  "context_profile": "incident_response"  // Bekanntes Profil
}
```

### 3. Namespace vs. Context mischen
Context-Gewichtung basiert auf dem **Namespace** des Dokuments, nicht auf Metadaten.

❌ **Falsch:**
```rust
// Dokument in "default" Namespace mit metadata.logical_namespace = "chronik"
// → Context-Weight für "default", nicht "chronik"
```

✅ **Richtig:**
```rust
// Dokument in "chronik" Namespace
namespace: "chronik".into()
```

## Erweiterung: Neue Profile

Neue Profile können in `policies/context.yaml` hinzugefügt werden.

## Observability & Audit

### Metriken

Folgende Prometheus-Metriken werden erhoben:

* `decision_weight_applied_total{factor}`: Zählt, wie oft ein Gewichtungsfaktor (trust, recency, context) angewendet wurde.
* `decision_final_score_bucket`: Histogramm der finalen Scores zur Analyse der Verteilung.

### Audit-Logs

Bei aktiviertem DEBUG-Logging protokolliert das System, wenn sich das Top-Ranking durch Gewichtung ändert:

```text
Decision weighting changed top result query="security update" original_top="doc-low-trust" weighted_top="doc-high-trust"
```

### Policy Hash

Um sicherzustellen, dass die erwartete Policy aktiv ist, gibt `/index/stats` einen Hash zurück:

```json
{
  "total_documents": 150,
  "policy_hash": "a1b2c3d4..."
}
```

## Testing

Tests unter `crates/indexd/tests/decision_weighting_test.rs`:

```bash
cargo test --package hauski-indexd decision_weighting
```

**Test-Szenarien:**
- Trust-Gewichtung ändert Ranking
- Context-Profile ändern Namespace-Prioritäten
- Kombinierte Gewichtung (Trust × Recency × Context)
- Weight Transparency (include_weights)

## Referenzen

- Issue: "indexd ↔ policy – Decision-Weighting nach Trust, Recency & Kontext"
- Code: `crates/indexd/src/lib.rs`
- Policies: `policies/trust.yaml`, `policies/context.yaml`
- Tests: `crates/indexd/tests/decision_weighting_test.rs`
