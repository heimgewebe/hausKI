# Decision Feedback System

Dieses Dokument beschreibt das Decision Feedback System in hausKI – die Grundlage für zukünftiges Lernen durch heimlern.

## Überblick

hausKI trifft Entscheidungen basierend auf gewichteten Suchen (trust, recency, context). Um aus diesen Entscheidungen zu lernen, muss hausKI:

1. **Entscheidungen dokumentieren** – Was wurde entschieden und warum?
2. **Feedback-Signale akzeptieren** – War die Entscheidung gut oder schlecht?
3. **Daten für heimlern bereitstellen** – Aber NICHT selbst interpretieren

**Wichtig:** hausKI passt niemals selbst Gewichte an. Das ist die Aufgabe von heimlern (separates Repository).

## Architektur

```
┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│   hausKI     │         │  Snapshots   │         │  heimlern    │
│              │─────────▶│   Storage    │────────▶│              │
│  entscheidet │         │              │         │   lernt      │
│              │◀────────│  Outcomes    │         │              │
└──────────────┘         └──────────────┘         └──────────────┘
      │                         ▲
      │                         │
      └─────────────────────────┘
         Feedback-API
       (validiert, speichert)
```

### Rollenverteilung

| Komponente | Verantwortung | Was sie NICHT tut |
|------------|---------------|-------------------|
| **hausKI** | Entscheidungen treffen, Snapshots emittieren, Feedback validieren & speichern | Gewichte ändern, Feedback interpretieren |
| **heimlern** | Snapshots analysieren, Muster erkennen, Gewichte optimieren | Entscheidungen treffen |

## Datenstrukturen

### DecisionSnapshot

Wird automatisch erzeugt, wenn eine gewichtete Suche durchgeführt wird (`include_weights=true`).

```json
{
  "decision_id": "01HXJ8ZQVN7XYTD9F8KW3RH2PM",
  "intent": "Rust memory safety",
  "timestamp": "2024-01-08T12:00:00Z",
  "namespace": "code",
  "context_profile": "code_analysis",
  "candidates": [
    {
      "id": "doc-rust-guide",
      "similarity": 0.85,
      "weights": {
        "similarity": 0.85,
        "trust": 1.0,
        "recency": 0.95,
        "context": 1.2
      },
      "final_score": 0.9690
    },
    {
      "id": "doc-python-guide",
      "similarity": 0.45,
      "weights": {
        "similarity": 0.45,
        "trust": 0.7,
        "recency": 0.98,
        "context": 0.5
      },
      "final_score": 0.1543
    }
  ],
  "selected_id": "doc-rust-guide",
  "policy_hash": "a3f5d9c2..."
}
```

**Felder:**
- `decision_id`: ULID (zeitbasiert, sortierbar, unique)
- `intent`: Die ursprüngliche Query
- `timestamp`: Wann wurde entschieden
- `namespace`: In welchem Namespace gesucht wurde
- `context_profile`: Welches Profil aktiv war (optional)
- `candidates`: Alle betrachteten Kandidaten mit vollständiger Gewichtung
- `selected_id`: Der gewählte Kandidat (Top-Ergebnis)
- `policy_hash`: Policy-Hash zum Zeitpunkt der Entscheidung (für Drift-Erkennung)

### DecisionOutcome

Feedback-Signal für eine Entscheidung.

```json
{
  "decision_id": "01HXJ8ZQVN7XYTD9F8KW3RH2PM",
  "outcome": "success",
  "signal_source": "user",
  "timestamp": "2024-01-08T12:05:00Z",
  "notes": "User confirmed this was helpful"
}
```

**Felder:**
- `decision_id`: Referenz zum Snapshot
- `outcome`: `success`, `failure`, oder `neutral`
- `signal_source`: `user`, `system`, oder `policy`
- `timestamp`: Wann das Feedback erfasst wurde
- `notes`: Optional – zusätzlicher Kontext

## API-Endpunkte

### Snapshots abrufen

```bash
# Liste aller Snapshots
curl http://localhost:8080/index/decisions/snapshot

# Spezifischer Snapshot
curl http://localhost:8080/index/decisions/snapshot/01HXJ8ZQVN7XYTD9F8KW3RH2PM
```

### Outcome melden

```bash
curl -X POST http://localhost:8080/index/decisions/outcome \
  -H 'Content-Type: application/json' \
  -d '{
    "decision_id": "01HXJ8ZQVN7XYTD9F8KW3RH2PM",
    "outcome": "success",
    "signal_source": "user",
    "timestamp": "2024-01-08T12:05:00Z",
    "notes": "User confirmed this was helpful"
  }'
```

### Outcomes abrufen

```bash
# Liste aller Outcomes
curl http://localhost:8080/index/decisions/outcomes

# Spezifisches Outcome
curl http://localhost:8080/index/decisions/outcome/01HXJ8ZQVN7XYTD9F8KW3RH2PM
```

## Metriken

### Prometheus-Metriken

```
# Anzahl emittierter Snapshots
decision_snapshots_total

# Anzahl gemeldeter Outcomes (nach Ergebnis)
decision_outcomes_total{outcome="success"}
decision_outcomes_total{outcome="failure"}
decision_outcomes_total{outcome="neutral"}
```

### Beispiel-Abfragen

```promql
# Erfolgsrate der letzten Stunde
sum(rate(decision_outcomes_total{outcome="success"}[1h])) 
/ 
sum(rate(decision_outcomes_total[1h]))

# Anzahl Snapshots ohne Feedback
decision_snapshots_total - sum(decision_outcomes_total)
```

## Workflow

### 1. Entscheidung treffen

hausKI führt eine gewichtete Suche durch:

```rust
let results = state.search(&SearchRequest {
    query: "Rust memory safety".into(),
    k: Some(10),
    namespace: Some("code".into()),
    include_weights: true,          // Für Response-Transparenz
    emit_decision_snapshot: true,   // Trigger Snapshot-Emission
    context_profile: Some("code_analysis".into()),
    ..Default::default()
}).await;
```

**Effekt:**
- Suche wird mit allen Gewichten durchgeführt
- Decision Snapshot wird automatisch erzeugt und gespeichert (wegen `emit_decision_snapshot: true`)
- Metrik `decision_snapshots_total` wird inkrementiert

### 2. Feedback erfassen

User oder System meldet Outcome:

```bash
curl -X POST /index/decisions/outcome \
  -d '{"decision_id": "...", "outcome": "success", "signal_source": "user"}'
```

**Effekt:**
- hausKI validiert Schema
- hausKI prüft, ob `decision_id` existiert
- Outcome wird gespeichert
- Metrik `decision_outcomes_total{outcome="success"}` wird inkrementiert

**hausKI tut NICHT:**
- Gewichte anpassen
- Snapshots neu bewerten
- Zukünftige Entscheidungen ändern

### 3. Lernen (heimlern)

heimlern holt regelmäßig Snapshots und Outcomes ab:

```bash
curl http://localhost:8080/index/decisions/snapshot
curl http://localhost:8080/index/decisions/outcomes
```

**heimlern analysiert:**
- Welche Gewichtungen führen zu erfolgreichen Outcomes?
- Gibt es systematische Fehler (z. B. niedrige Trust führt oft zu Failure)?
- Sind bestimmte Context-Profile besser als andere?

**heimlern passt an:**
- Trust-Weights in `policies/trust.yaml`
- Context-Profile in `policies/context.yaml`
- Recency-Decay-Parameter

**heimlern schreibt zurück:**
- Neue Policy-Dateien
- hausKI lädt diese beim nächsten Start

## Sicherheit

### Validierungen

hausKI validiert alle Outcome-Meldungen:

```rust
pub async fn record_outcome(&self, outcome: DecisionOutcome) -> Result<(), IndexError> {
    // 1. Decision ID muss existieren
    let snapshots = self.inner.decision_snapshots.read().await;
    if !snapshots.contains_key(&outcome.decision_id) {
        return Err(IndexError {
            error: format!("Decision ID {} not found", outcome.decision_id),
            code: "decision_not_found".into(),
            ..
        });
    }
    
    // 2. Schema wird durch Serde validiert (outcome, signal_source müssen valide Enums sein)
    
    // 3. Speichern (keine Interpretation)
    let mut outcomes = self.inner.decision_outcomes.write().await;
    outcomes.insert(outcome.decision_id.clone(), outcome.clone());
    
    Ok(())
}
```

### Was hausKI NICHT tut

- ❌ Gewichte basierend auf Outcomes anpassen
- ❌ Outcomes interpretieren oder aggregieren
- ❌ Policies ändern
- ❌ Entscheidungen rückwirkend bewerten

**Grund:** Lernen ohne externes Korrektiv führt zu Drift. heimlern hat die notwendige Distanz.

## Beispiel-Szenarien

### Szenario 1: User-Feedback auf Suchergebnis

1. User sucht "Rust memory safety"
2. hausKI findet 10 Dokumente, wählt `doc-rust-guide`
3. Snapshot wird gespeichert mit `decision_id=01HXJ...`
4. User klickt auf Ergebnis und bewertet: "👍 Hilfreich"
5. Frontend sendet: `POST /decisions/outcome { decision_id: "01HXJ...", outcome: "success", signal_source: "user" }`
6. heimlern analysiert später: "High-Trust-Dokumente mit Context-Profile 'code_analysis' führen oft zu Success"
7. heimlern erhöht Trust-Weight für `chronik`-Origin in Code-Kontext

### Szenario 2: System-Feedback auf falsche Wahl

1. hausKI wählt `doc-python-guide` für Rust-Query (niedrige Similarity, aber hohe Recency)
2. Snapshot: `selected_id=doc-python-guide, final_score=0.78`
3. System erkennt: User hat sofort weitergeklickt (keine Verweildauer)
4. System sendet: `POST /decisions/outcome { decision_id: "...", outcome: "failure", signal_source: "system" }`
5. heimlern analysiert: "Recency-Weight von 1.0 führte zu False Positive – Decay zu aggressiv"
6. heimlern passt `recency.default_half_life_seconds` an

### Szenario 3: Policy-basiertes Feedback

1. hausKI wählt Dokument mit Trust-Level `low`
2. Snapshot: `selected_id=doc-external, trust_weight=0.3`
3. Policy-Engine prüft: Dokument enthält `ContentFlag::PossiblePromptInjection`
4. Policy sendet: `POST /decisions/outcome { decision_id: "...", outcome: "failure", signal_source: "policy", notes: "Security flag triggered" }`
5. heimlern lernt: "Low-Trust-Dokumente mit Security-Flags sollten stärker bestraft werden"
6. heimlern senkt Low-Trust-Weight von 0.3 auf 0.1

## Offene Punkte (für heimlern)

Diese Features sind **nicht** Teil von hausKI, sondern Aufgabe von heimlern:

- [ ] Regelmäßiges Abrufen von Snapshots und Outcomes
- [ ] Aggregation und Analyse von Feedback-Signalen
- [ ] Erkennung von Mustern (z. B. "Trust=1.0 + Context='incident_response' → 90% Success")
- [ ] Optimierung von Gewichten (Gradient Descent, Bayesian Optimization, etc.)
- [ ] Policy-Drift-Erkennung (Policy-Hash-Vergleich über Zeit)
- [ ] Schreiben neuer Policy-Dateien
- [ ] A/B-Testing verschiedener Gewichtungen

## Implementierungsdetails

### Snapshot-Erzeugung

Snapshots werden in `IndexState::search()` erzeugt, wenn `include_weights=true`:

```rust
// Am Ende von search()
if request.include_weights && !matches.is_empty() {
    let decision_id = Ulid::new().to_string();
    
    let candidates: Vec<DecisionCandidate> = matches.iter().map(|m| {
        DecisionCandidate {
            id: m.doc_id.clone(),
            similarity: m.weights.as_ref().unwrap().similarity,
            weights: m.weights.clone().unwrap(),
            final_score: m.score,
        }
    }).collect();
    
    let snapshot = DecisionSnapshot {
        decision_id: decision_id.clone(),
        intent: request.query.clone(),
        timestamp: Utc::now().to_rfc3339(),
        namespace: namespace.to_string(),
        context_profile: request.context_profile.clone(),
        candidates,
        selected_id: Some(matches[0].doc_id.clone()),
        policy_hash: self.inner.policies.hash.clone(),
    };
    
    let mut snapshots = self.inner.decision_snapshots.write().await;
    snapshots.insert(decision_id, snapshot);
}
```

### Outcome-Speicherung

Outcomes werden in `HashMap<String, DecisionOutcome>` gespeichert:

```rust
pub async fn record_outcome(&self, outcome: DecisionOutcome) -> Result<(), IndexError> {
    // Validierung
    let snapshots = self.inner.decision_snapshots.read().await;
    if !snapshots.contains_key(&outcome.decision_id) {
        return Err(IndexError { /* ... */ });
    }
    drop(snapshots);
    
    // Speichern
    let mut outcomes = self.inner.decision_outcomes.write().await;
    outcomes.insert(outcome.decision_id.clone(), outcome.clone());
    
    // Metriken
    self.inner.prom_decision_outcomes_total
        .get_or_create(&OutcomeLabels {
            outcome: outcome.outcome.to_string(),
        })
        .inc();
    
    Ok(())
}
```

## Persistenz

**Aktueller Stand:** In-Memory-Storage (HashMap)

**Zukünftig (geplant):**
- SQLite für langfristige Speicherung
- TTL-basierte Bereinigung alter Snapshots
- Export zu heimlern über Event-Stream oder API-Pull

## Referenzen

- [Decision Weighting](./decision-weighting.md) – Wie Gewichtungen funktionieren
- Issue #5 – Original-Beschreibung des Features
