# hausmAIster — lokaler PC-Hausmeister-Modus von hausKI

## 1. Begriff und Rolle

- Sichtbarer Name: `hausmAIster`
- Technischer Namespace: `hausmaister`
- Repo vorerst: `hausKI`
- Später möglich: eigenes Repo `hausmaister`, aber erst bei eigenständiger Runtime.

### Etymologie

„Hausmeister“ besteht aus „Haus“ und „Meister“: jemand, der Zustand, Ordnung, Wartung und praktische Abläufe eines Hauses betreut. hausmAIster setzt AI in die Mitte. Das ist als Produktname gut, als technischer Pfad aber etwas eitel. Dateisysteme mögen Nüchternheit; Wortspiele gehören an die Tür, nicht in den Sicherungskasten.

### Definition

hausmAIster ist der lokale Ordnungs-, Diagnose- und Wartungsmodus von hausKI. Er fragt lokale Beobachtungsquellen ab, bewertet Zustände, erstellt verständliche Findings, schlägt Maßnahmen vor und darf später nur nach expliziter Freigabe begrenzte Commands auslösen.

---

## 2. Kernentscheidung

hausmAIster ist nicht der Scanner.

Er ist:

- Beobachtungs-Koordinator
- Diagnose-Übersetzer
- Priorisierer
- Plan-Ersteller
- Freigabe-Gate
- später: Command-Antragsteller

Er ist nicht:

- roher Dateisystem-Scanner
- Shell-Agent
- autonomer Aufräumer
- Git-Mutator
- Paketmanager
- Systemdienst-Verwalter

Warum: Scanner sollen deterministisch, eng begrenzt und auditierbar sein. hausKI/hausmAIster soll bewerten und entscheiden, nicht heimlich durch /home/<user> stapfen wie ein Staubsauger mit Promotionsrecht.

---

## 3. Systeminvariante

- heim-pc / Atlas / Lenskit / mitschreiber beobachten.
- chronik protokolliert.
- semantAH verdichtet.
- hausKI entscheidet.
- hausmAIster erklärt und plant.
- leitstand zeigt.
- WGX prüft.
- Commands handeln nur nach Freigabe.

Kurzform:

`Observation ≠ Decision ≠ Command`

Diese Trennung ist die zentrale Sicherheits- und Architekturregel.

---

## 4. Zielbild

```text
┌──────────────────────────────────────────────┐
│ Nutzer                                        │
│ "Räum meinen PC auf / Was ist hier los?"      │
└──────────────────────┬───────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────┐
│ hausKI                                       │
│ Rolle: hausmAIster                           │
│                                              │
│ - versteht Auftrag                           │
│ - fragt Beobachtungsquellen ab               │
│ - bewertet Zustand                           │
│ - erstellt Plan                              │
│ - fordert Freigabe für Commands              │
└──────────────────────┬───────────────────────┘
                       │
        read-only      │
                       ▼
┌──────────────────────────────────────────────┐
│ heim-pc / Atlas / Lenskit / mitschreiber      │
│                                              │
│ - Dateisystem-Snapshots                      │
│ - Repo-Zustände                              │
│ - Dumps / Bundles                            │
│ - Artefakte                                  │
│ - Logs / Kontext                             │
│ - keine Mutation durch Agent                 │
└──────────────────────┬───────────────────────┘
                       │
        events          │
                       ▼
┌──────────────────────────────────────────────┐
│ chronik                                      │
│                                              │
│ - observation.requested                      │
│ - observation.completed                      │
│ - finding.created                            │
│ - plan.proposed                              │
│ - command.requested                          │
│ - command.approved / rejected                │
└──────────────────────┬───────────────────────┘
                       │
                       ▼
┌──────────────────────────────────────────────┐
│ leitstand                                    │
│                                              │
│ - Übersicht                                  │
│ - Risiken                                    │
│ - offene Pläne                               │
│ - Freigaben                                  │
│ - Historie                                   │
└──────────────────────────────────────────────┘
```

---

## 5. Rollenmodell

| Komponente | Rolle | Darf lesen | Darf schreiben | Darf handeln |
| --- | --- | --- | --- | --- |
| hausKI | Orchestrator | ja | Artefakte/Entscheidungen | nur über Gates |
| hausmAIster | lokale PC-Hausmeister-Rolle | ja, über Adapter | Findings/Plans | nein, nur beantragen |
| heim-pc | lokales Orientierungssystem | ja | Snapshots/Reports | nein |
| Atlas/Lenskit | Knowledge Engine / Scan- und Retrieval-Schicht | ja | eigene Artefakte | nein |
| mitschreiber | OS-Kontext-Erfassung | ja | Kontext-Artefakte | nein |
| chronik | Ereignisgedächtnis | Events | Event-Log | nein |
| semantAH | semantische Verdichtung | ja | Insights | nein |
| leitstand | UI/Observer | ja | UI-State | nein |
| WGX | Guard/Smoke/Fleet-Motorik | ja | Reports | CI-Handlung, nicht PC-Handlung |
| Command-Executor | spätere Aktionsschicht | begrenzt | ja | ja, nach Freigabe |

---

## 6. Hauptfähigkeiten von hausmAIster

### Phase A — Überblick

Fragen wie:

- Was ist auf meinem PC unordentlich?
- Welche Repos sind dirty?
- Welche Ordner wachsen stark?
- Welche Downloads kann ich prüfen?
- Welche alten Artefakte wirken löschbar?
- Wo liegen große Dateien?
- Welche Dienste laufen?
- Welche Projekte haben Drift?
- Welche TODOs hängen?

Ergebnis:

- `FindingReport`
- `RiskReport`
- `CleanupProposal`
- `RepoHealthSummary`
- `StoragePressureReport`

### Phase B — Diagnose

Beispiele:

- Warum ist mein Speicher voll?
- Warum startet ein Dienst nicht?
- Welche Repos sind nicht synchron?
- Welche Snapshots sind veraltet?
- Welche Artefakte fehlen?
- Welche lokalen Zustände widersprechen der Doku?

Ergebnis:

- `Diagnosis`
- `Hypotheses`
- `EvidenceRefs`
- `StopCriteria`
- `NextChecks`

### Phase C — Plan

Beispiele:

- Erstelle einen sicheren Aufräumplan.
- Bereite einen Sync-Plan vor.
- Welche Dateien könnten archiviert werden?
- Welche Repos brauchen Commit/Pull/Push?

Ergebnis:

- `ActionPlan`
- `CommandProposal`
- `RiskClass`
- `RequiredApproval`
- `RollbackHint`

### Phase D — Freigabe

Keine Aktion ohne explizite Zustimmung.

Plan: Archiviere alte Logs älter als 90 Tage.
Risiko: niedrig bis mittel.
Befehl: ...
Freigabe nötig: ja.

### Phase E — spätere Ausführung

Nur begrenzte Commands:

```text
mkdir
cp
rsync --dry-run / später begrenzt
git status
git fetch
systemctl status
du
find ohne -delete, -exec, -ok oder schreibende Nebenwirkung
```

Nicht automatisch:

```text
rm
mv
chmod
chown
git reset
git clean
systemctl restart
apt install/remove
docker prune
```

---

## 7. Contracts-first: vorgeschlagene Contract-Struktur

Im hausKI-Repo:

Die folgenden Dateinamen sind Arbeitsnamen für den Folge-Contract-PR. Die finale Benennung muss an die bestehende hausKI-Contract-Konvention angepasst werden; insbesondere darf Versionierung nicht doppelt kodiert werden, falls das bestehende Schema bereits ein eigenes version-Feld nutzt.

```text
contracts/hausmaister/
  hausmaister-task-request.v1.schema.json
  hausmaister-observation-request.v1.schema.json
  hausmaister-observation-report.v1.schema.json
  hausmaister-finding.v1.schema.json
  hausmaister-plan.v1.schema.json
  hausmaister-command-proposal.v1.schema.json
  hausmaister-command-approval.v1.schema.json
  hausmaister-risk.v1.schema.json
  hausmaister-policy.v1.schema.json
```

Technische Namespaces nüchtern:

```text
crates/core/src/hausmaister/
docs/hausmaister/
contracts/hausmaister/
```

Sichtbare UI-Sprache darf hausmAIster nutzen.

---

## 8. Zentrale Artefakte

IDs werden als ULID verstanden; die Beispiele verwenden exemplarische ULID-Werte. Zeitstempel werden als RFC3339/UTC verstanden.

### HausmaisterTaskRequest

```json
{
  "task_id": "01J8ZQ5M7K6M9R2T4C8V1B3N5P",
  "requested_by": "user",
  "goal": "string",
  "scope": {
    "machine": "heim-pc",
    "zones": ["repos", "downloads", "documents"],
    "mode": "read_only"
  },
  "constraints": {
    "no_mutation": true,
    "max_depth": 3,
    "include_hidden": false
  }
}
```

### ObservationReport

```json
{
  "report_id": "01J8ZQ5M7K6M9R2T4C8V1B3N5Q",
  "source": "heim-pc|atlas|lenskit|mitschreiber",
  "generated_at": "2026-05-10T12:00:00Z",
  "scope": {},
  "findings": [],
  "evidence_refs": [],
  "freshness": "fresh|stale|unknown"
}
```

### Finding

```json
{
  "finding_id": "01J8ZQ5M7K6M9R2T4C8V1B3N5R",
  "category": "storage|repo|config|service|security|drift|unknown",
  "severity": "info|low|medium|high|critical",
  "claim": "string",
  "evidence_refs": [],
  "uncertainty": {
    "score": 0.0,
    "causes": []
  }
}
```

### Plan

```json
{
  "plan_id": "01J8ZQ5M7K6M9R2T4C8V1B3N5S",
  "goal": "string",
  "steps": [],
  "risk": {
    "class": "low|medium|high",
    "possible_harms": []
  },
  "requires_approval": true,
  "commands": []
}
```

### CommandProposal

```json
{
  "command_id": "01J8ZQ5M7K6M9R2T4C8V1B3N5T",
  "intent": "archive|inspect|sync|cleanup|status",
  "command": "string",
  "dry_run_available": true,
  "requires_user_approval": true,
  "forbidden_if": [
    "path_outside_scope",
    "contains_delete",
    "contains_chmod",
    "contains_git_reset"
  ]
}
```

---

## 9. Event-Modell

Events sind Beobachtung und Verlauf. Commands sind Handlungsabsicht. Nicht vermischen.

Die Event-Namen sind Arbeitsnamen. Im Folge-Contract-PR ist gegen die bestehende hausKI-Event-Konvention zu prüfen, ob Versionierung im Event-Kind, im separaten version-Feld oder bewusst in beiden Formen geführt wird.

```text
hausmaister.task.requested.v1
hausmaister.observation.requested.v1
hausmaister.observation.completed.v1
hausmaister.finding.created.v1
hausmaister.plan.proposed.v1
hausmaister.command.proposed.v1
hausmaister.command.approved.v1
hausmaister.command.rejected.v1
hausmaister.command.executed.v1
hausmaister.command.failed.v1
```

Wichtig:
plan.proposed ist kein command.approved.
Ein Plan ist ein Gedanke mit Formular. Ein Command ist ein Schraubenzieher. Man sollte beides nicht verwechseln, sonst philosophiert der Schraubenzieher plötzlich am Mainboard.

---

## 10. Sicherheitsmodell

### Grundsatz

- Read-only first.
- Dry-run second.
- Approval third.
- Execution last.

### Risikoklassen

| Klasse | Beispiele | Freigabe |
| --- | --- | --- |
| R0 read-only | status, du, git status, ls | keine oder pauschal |
| R1 ungefährlich schreibend | mkdir in Arbeitsordner, Report speichern | einfache Freigabe |
| R2 reversibel | cp, rsync ohne delete, archive | explizite Freigabe |
| R3 riskant | mv, git checkout, service restart | starke Freigabe |
| R4 verboten/default | rm, chmod rekursiv, git reset, docker prune | gesperrt |

### Verbotene Default-Capabilities

```text
filesystem_delete
filesystem_move
chmod
chown
git_reset
git_clean
package_install
package_remove
systemctl_restart
docker_prune
secret_read
browser_profile_read
ssh_key_read
```

### Schutz gegen typische Fehler

1. Pfadverwechslung: jeder Command braucht Scope-Prüfung.
2. Stale Data: jeder Plan braucht Freshness-Angabe.
3. Prompt Injection: Repo-Inhalte dürfen keine Commands direkt erzeugen.
4. Übereifrige Ordnung: „unbenutzt“ heißt nicht „löschbar“.
5. False Confidence: jede Diagnose braucht Beleg oder Leerstelle.

---

## 11. Datenfluss

```text
Nutzerauftrag
  ↓
hausmAIster TaskRequest
  ↓
ObservationRequest an heim-pc / Atlas / Lenskit / mitschreiber
  ↓
ObservationReport
  ↓
Findings
  ↓
RiskAssessment
  ↓
PlanProposal
  ↓
CommandProposal
  ↓
UserApproval
  ↓
CommandExecutor
  ↓
ExecutionReport
  ↓
chronik + leitstand
```

---

## 12. Wo Atlas/Lenskit hineingehören

Atlas/Lenskit sollten genutzt werden, aber nicht als hausmAIster selbst.

Hinweis: `Atlas` wird hier als read-only Beobachtungs-/Kartierungsschicht verstanden. Ob Atlas als Lenskit-Komponente, heim-pc-naher Adapter oder später eigenes Modul realisiert wird, ist in diesem Blueprint noch offen.

Besser:

- Atlas/Lenskit = Augen, Karte, Archiv
- hausmAIster = Hirn, Hausordnung, Dialog
- Command-Executor = Hand, aber mit Schlüsselbox

Warum Atlas/Lenskit stark sind

Plausibel aus der bisherigen Blaupause:

- Repo-Überblick
- Dumps
- Artefakte
- Retrieval
- Evidence-Bundles
- Atlas-Snapshots
- lokale Dateisystem- und Projektorientierung

Warum sie nicht direkt handeln sollen

Weil ein Scanner, der schreiben darf, seine Beobachtung durch Handlung verändert. Das ist epistemisch unsauber: Der Zeuge fängt an, den Tatort aufzuräumen.

---

## 13. Architektur im Heimgewebe

```text
heim-pc
  produziert:
    filesystem.snapshots
    repo.maps
    local.state.reports
lenskit / atlas
  produziert:
    knowledge.bundles
    retrieval.context
    evidence.refs
    architecture.snapshots
mitschreiber
  produziert:
    os.context.state
    os.context.text
chronik
  produziert:
    event.records
    timelines
semantAH
  produziert:
    insights.daily
    knowledge.observatory
hausKI / hausmAIster
  konsumiert:
    snapshots
    reports
    evidence.refs
    os.context
    events
  produziert:
    findings
    plans
    command.proposals
    decision.preimages
leitstand
  konsumiert:
    findings
    plans
    health
    events
```

---

## 14. Phasenplan

### Phase 0a — Begriff und Blueprint

Ziel: hausmAIster als Rolle sauber definieren und als Doku-Anker kanonisch ablegen.

Datei in diesem PR:

```text
docs/hausmaister/blueprint.md
```

Nicht enthalten:

- kein Scanner
- kein Executor
- kein Runtime-Code
- kein MCP
- kein Tailscale-Funnel
- keine Contracts

Stop-Kriterium:

- Rollen sind klar.
- Events, Decisions und Commands sind getrennt.
- Der Blueprint widerspricht dem Follow-up-Schnitt nicht.

### Phase 0b — Security-Modell, Events-vs-Commands und Contracts

Ziel: die Blueprint-Entscheidungen in separate Detaildokumente und JSON-Schemas überführen.

Follow-up-Dateien:

```text
docs/hausmaister/security-model.md
docs/hausmaister/events-vs-commands.md
contracts/hausmaister/*.schema.json
```

Stop-Kriterium:

- Contracts sind valide.
- Security-Modell und Events-vs-Commands sind separat referenzierbar.
- Keine Runtime-, Scanner- oder Executor-Logik wird eingeführt.

---

### Phase 1 — Read-only Observation Adapter

Ziel: hausKI kann ObservationReports lesen.

Mögliche Quellen:

- heim-pc snapshots
- Lenskit/Atlas reports
- mitschreiber OS-Kontext
- manuell abgelegte JSON-Fixtures

Implementierung:

```text
crates/core/src/hausmaister/
  mod.rs
  types.rs
  policy.rs
  report.rs
  findings.rs
```

Stop-Kriterium:

- Fixtures rein → Findings raus.
- Keine echten Dateioperationen im Test.

---

### Phase 2 — Findings und Priorisierung

Ziel: aus Reports werden verständliche Findings.

Beispiele:

- Repo dirty
- Repo ahead/behind
- große Datei
- veralteter Snapshot
- stale bundle
- unklare Ownership
- potenzielles Secret
- inkonsistente Doku

Stop-Kriterium:

Finding enthält claim, evidence_ref, severity, uncertainty.

---

### Phase 3 — Plan-Erstellung

Ziel: hausmAIster schlägt Maßnahmen vor.

Beispiel:

```text
"Downloads aufräumen"
→ analysiere nur
→ gruppiere Kandidaten
→ markiere Risiken
→ erstelle Archivplan
→ keine Ausführung
```

Stop-Kriterium:

Plan enthält Schritte, Risiko, benötigte Freigabe, keine direkte Mutation.

---

### Phase 4 — Leitstand-Integration

Ziel: Pläne und Findings sichtbar machen.

UI-Elemente:

- Offene Findings
- Risikoampel
- Frische der Daten
- Vorgeschlagene Pläne
- Freigabe erforderlich
- Historie

Stop-Kriterium:

leitstand kann hausmAIster-Reports darstellen.

---

### Phase 5 — Command-Proposals

Ziel: konkrete Commands werden vorgeschlagen, aber nicht ausgeführt.

Beispiel:

```text
du -h --max-depth=1 ~/Downloads
git -C ~/repos/foo status --short
mkdir -p ~/Archive/old-logs
rsync --dry-run ...
```

Stop-Kriterium:

Jeder Command hat Scope, Risiko, Dry-run, Freigabestatus.

---

### Phase 6 — Begrenzter Executor

Ziel: ausgewählte Commands nach Freigabe ausführen.

Nur erlaubte Klassen:

- read-only inspection
- dry-run
- mkdir in erlaubten Zonen
- copy/archive ohne delete

Stop-Kriterium:

- Command-Ausführung wird protokolliert.
- Keine verbotenen Befehle möglich.

---

### Phase 7 — Lokale Assistenz

Ziel: Du kannst sagen:

- hausmAIster, prüf meinen PC.
- hausmAIster, warum ist mein Speicher voll?
- hausmAIster, welche Repos muss ich anfassen?
- hausmAIster, bereite einen sicheren Aufräumplan vor.
- hausmAIster, führe Schritt 1 aus.

Stop-Kriterium:

Dialog → Plan → Freigabe → Ausführung → Report.

---

## 15. MVP-Schnitt

Der erste sinnvolle PR sollte nicht Runtime bauen.

PR 1a

Titel:

```text
docs(hausmaister): define local PC caretaker blueprint
```

Inhalt:

```text
docs/hausmaister/blueprint.md
```

Nicht enthalten:

- kein Code
- kein Scanner
- kein Executor
- keine Tailscale-Änderung
- keine ChatGPT-Integration
- keine Contracts

Follow-up-PRs:

```text
docs/hausmaister/security-model.md
docs/hausmaister/events-vs-commands.md
contracts/hausmaister/*.schema.json
```

Warum: Das verhindert, dass ein Agent direkt in Code springt und aus „Hausmeister“ einen Abrissunternehmer mit JSON-Ausgabe baut.

---

## 16. Konkrete Aufgaben, die hausmAIster später leisten soll

### Ordnung

- Downloads gruppieren
- große Dateien zeigen
- alte Archive erkennen
- Dubletten-Kandidaten markieren
- temporäre Dateien finden
- Screenshots sortieren

### Repo-Übersicht

- dirty repos
- unpushed commits
- untracked files
- stale branches
- CI-Fails
- offene PR-Vorbereitungen
- Contract-Drift

### Betrieb

- laufende Dienste anzeigen
- fehlgeschlagene systemd-Units melden
- Portübersicht erklären
- Tailscale-Status bewerten
- Speicher-/CPU-/RAM-Druck erkennen

### Wissensordnung

- Dumps katalogisieren
- Lenskit-Bundles prüfen
- stale Artefakte markieren
- fehlende Summaries anzeigen
- Architektur-Widersprüche finden

### Sicherheitsnähe

- potenzielle Secrets markieren
- öffentliche Exposition prüfen
- Funnel/Serve-Unterschied sichtbar machen
- zu breite Pfade warnen

---

## 17. Entscheidungsmatrix: hausmAIster als Rolle vs eigenes Repo

| Option | Nutzen | Risiko | Urteil |
| --- | --- | --- | --- |
| Rolle in hausKI | hoch | niedrig | jetzt beste Wahl |
| Modul crates/hausmaister | mittel-hoch | mittel | später sinnvoll |
| eigenes Repo hausmaister | später hoch | jetzt zu früh | noch nicht |
| Umbenennung hausKI → hausmAIster | gering | hoch | nicht machen |
| Scanner direkt in hausKI | kurzfristig bequem | architektonisch riskant | vermeiden |

Entscheidung:
hausmAIster als Rolle und Namespace in hausKI. Scanner bleiben extern/read-only.

---

## 18. Resonanz- und Kontrastprüfung

Deutung 1: „hausmAIster sollte alles selbst machen“

Plausibel, weil ein Hausmeister praktisch handelt. Vorteil: einfacher mentaler Zugriff. Nachteil: Scanner, Entscheider und Executor verschmelzen.

Bewertung: gefährlich, weil Sicherheitsgrenzen verschwimmen.

Deutung 2: „hausmAIster sollte nur reden“

Plausibel, weil read-only sicher ist. Vorteil: geringes Risiko. Nachteil: irgendwann bleibt es bei schönen Diagnosen ohne praktische Wirkung.

Bewertung: zu schwach als Endziel.

Synthese daraus

hausmAIster darf praktisch werden, aber nur über:

- `Plan`
- `RiskAssessment`
- `CommandProposal`
- `Approval`
- `ExecutionReport`

Nicht über autonome Sofort-Aktion.

---

## 19. Belegt / plausibel / spekulativ

### Belegt aus unserer Architekturarbeit

- hausKI ist Orchestrator.
- heim-pc ist lokaler PC-/Orientierungskontext.
- Lenskit/Atlas eignen sich als read-only Wissens- und Beobachtungsschicht.
- Events und Commands müssen getrennt bleiben.
- Contracts-first ist Heimgewebe-Invariante.

### Plausibel

- hausmAIster als Rolle in hausKI ist sauberer als Repo-Rename.
- Scanner sollten außerhalb von hausKI bleiben.
- Command-Ausführung braucht eigenes Gate.
- leitstand sollte Findings/Plans anzeigen.

### Spekulativ

- Ob später ein eigenes Repo hausmaister nötig wird.
- Welche konkreten lokalen Scanner-APIs Atlas/Lenskit final bereitstellen.
- Wie stark ChatGPT später eingebunden werden soll.

---

## 20. Epistemische Leeren

X fehlt, nötig für Y:

- Aktueller Lenskit/Atlas Runtime-Status fehlt, nötig für konkrete Adapterplanung.
- Aktuelle heim-pc Snapshot-Formate fehlen, nötig für endgültige ObservationReport-Schemas.
- Bestehende hausKI Contract-Struktur fehlt im Detail, nötig für exakte Dateipfade.
- Command-Executor-Policy fehlt, nötig für spätere Ausführung.
- leitstand-Datenmodell fehlt, nötig für UI-Integration.

Daher: Dieser PR liefert nur den Blueprint als Doku-Anker; Contracts und Detaildokumente folgen separat, nicht als Runtime-Code.

---

## 21. Risiko- und Nutzenabschätzung

**Nutzen**

- lokale PC-Übersicht
- weniger Chaos in Repos und Dateien
- bessere Diagnosefähigkeit
- sichere Vorbereitung von Aufräumaktionen
- wiederverwendbare Evidence
- iPad-/Tailnet-taugliche Bedienung
- später echte lokale Assistenz

**Risiken**

- Datenleck durch Pfade oder Dateiinhalte
- Prompt Injection aus lokalen Dateien
- falsche Löschvorschläge
- Verwechslung von stale Reports mit Ist-Zustand
- zu breite Shell-Freigaben
- Autonomie-Drift
- Repo- und Contract-Drift

### Risikosenkung

- read-only Start
- keine rohen Commands aus Textquellen
- Contracts-first
- Explizite Freigabe
- Dry-run bevorzugen
- chronik-Protokollierung
- leitstand-Sichtbarkeit
- WGX-Guards

---

## 22. Heimgewebe-Integrität

Status: OK, mit kritischem Rand bei späterer Command-Ausführung.

### Betroffene Achsen

- Events
- Commands
- OS-Kontext
- Contracts
- Semantik
- WGX

### Betroffene Repos

- hausKI
- heim-pc
- lenskit
- mitschreiber
- chronik
- semantAH
- leitstand
- wgx
- metarepo

### Drift-Hinweise

- hausmAIster darf nicht zweite Control-Plane werden.
- Scanner dürfen keine Commands auslösen.
- ObservationReports dürfen nicht als Wahrheit ohne Freshness gelten.
- Lenskit/Atlas dürfen nicht direkt öffentlich exponiert werden.
- Commands brauchen eigene Policy und Approval.

Kohärenzbewertung: gut, wenn hausmAIster als Rolle eingeführt wird und Contracts zuerst kommen.

---

## 23. Optimierungsgrad

Was wird optimiert:
Architekturklarheit, lokale Assistenzfähigkeit, Sicherheit, Erweiterbarkeit.

Wie:
Trennung in Rolle, Scanner, Reports, Findings, Plans, Commands.

Wodurch:
Contracts-first, Events-vs-Commands, read-only Start, explizite Freigabe.

Wirkung:
Hoher strategischer Nutzen. Geringeres Risiko, dass lokale KI-Zugriffe zu unkontrollierter PC-Automation werden.

Nebenwirkung:
Mehr Anfangsstruktur. Weniger „einfach mal machen“. Das ist hier aber kein Nachteil, sondern Brandschutz. Feuerlöscher wirken auch etwas bürokratisch, bis der Teppich brennt.

---

## 24. Empfehlung

Die neue Blaupause sollte kanonisch so heißen:

`docs/hausmaister/blueprint.md`

Titel im Dokument:

```text
# hausmAIster — lokaler PC-Hausmeister-Modus von hausKI
```

### Technischer Grundsatz

- hausmAIster ist eine Rolle in hausKI.
- hausmaister ist der technische Namespace.
- Scanner bleiben read-only und extern.
- Commands kommen später über Approval-Gates.

---

## 25. Nächste Aktion

- Nicht deployen.
- Nicht umbenennen.
- Nicht Executor bauen.


### Essenz

Hebel: hausmAIster als Rolle, nicht als autonomer Scanner.
Entscheidung: hausKI bleibt Orchestrator; hausmAIster wird lokaler PC-Hausmeister-Modus; Atlas/Lenskit/heim-pc liefern read-only Beobachtung.
Nächste Aktion: separater Contracts- und Detaildoku-PR für docs/hausmaister/ und contracts/hausmaister/.

Unsicherheitsgrad: 0.24
Ursachen: Zielarchitektur ist klar; aktuelle Runtime-Details von Lenskit/Atlas/heim-pc und bestehende hausKI-Dateistruktur sind hier nicht vollständig belegt.

Interpolationsgrad: 0.31
Hauptannahmen: hausKI bleibt zentrale Entscheidungsschicht; Lenskit/Atlas können ObservationReports liefern; Command-Ausführung soll später, aber nicht jetzt kommen.

---

## Follow-up

Dieser PR definiert die Rolle und Architektur von `hausmAIster` als Doku-Anker.
Die zugehörigen JSON-Schemas unter `contracts/hausmaister/` folgen in einem separaten Contracts-PR.
Runtime-Code, Scanner, Executor und Gateway-Integration sind ausdrücklich nicht Teil dieses PR.
