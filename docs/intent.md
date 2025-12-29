# Intent Resolver

Der `intent-resolver` analysiert den Kontext (Git, GitHub Events), um die Absicht der aktuellen Änderung zu bestimmen. Dies hilft `hausKI` zu entscheiden, welche Tools oder Prozesse verwendet werden sollen (z.B. Coding vs. Writing vs. CI Triage).

## Contract Specification

This section defines the canonical schema and behavior for the Intent artifact. Ideally, this should be formalized in a `metarepo` contract (e.g. `contracts/events/hauski.intent.v1.schema.json`).

### Schema (JSON)

The `hauski intent` command produces a JSON object with the following fields. This schema is covered by regression tests to ensure stability.

```json
{
  "intent": "coding",       // Enum String: coding, writing, ci_triage, contracts_work, unknown
  "confidence": 0.7,        // Float: 0.0 - 1.0
  "signals": [              // Array of signals contributing to the decision
    {
      "kind": "changed_path", // Type of signal
      "ref": "crates/core/src/lib.rs", // Reference (path, comment, etc.)
      "weight": 0.9         // Weight of this specific signal
    }
  ],
  "created_at": "2023-10-27T10:00:00Z", // ISO 8601 Timestamp
  "context": {}             // Raw context data (optional)
}
```

### Intent Types (Enum)

*   `coding`: Source code changes (`src/`, `crates/`, `.rs`, `.py`).
*   `writing`: Documentation changes (`docs/`, `*.md`).
*   `ci_triage`: CI configuration changes (`.github/workflows/`) or via PR comments.
*   `contracts_work`: Contract definition changes (`contracts/`).
*   `unknown`: No clear signals identified.

### Confidence Logic

*   **Base Confidence**: 0.55
*   **Boost (+0.15)**: Strong path signals (> 80% of changes align with one type).
*   **Penalty (-0.20)**: Mixed or unclear signals (< 60% dominance).
*   **Range**: Clamped between 0.0 and 1.0.

The resolver aggregates signals from:
1.  **Changed Files**: Paths and extensions.
2.  **Workflow Name**: If running in CI.
3.  **PR Comments**: Commands like `/quick` or `/review`.

## Verwendung

### CLI

```bash
hauski intent
```

Ausgabe (Beispiel):

```json
{
  "intent": "coding",
  "confidence": 0.7,
  "signals": [
    {
      "kind": "changed_path",
      "ref": "crates/core/src/lib.rs",
      "weight": 0.9
    }
  ],
  "created_at": "2023-10-27T10:00:00Z",
  "context": {}
}
```

Mit Datei-Output:

```bash
hauski intent --output out/intents/intent-current.json
```

### CI Integration

In GitHub Actions kann der Intent Resolver genutzt werden, um nachfolgende Schritte zu steuern.

```yaml
- name: Determine Intent
  run: |
    cargo run -p hauski-cli -- intent --output intent.json
    echo "INTENT=$(jq -r .intent intent.json)" >> $GITHUB_ENV
```
