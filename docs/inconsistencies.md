# Inkonsistenzen zwischen Dokumentation und Code

Dieses Dokument listet gefundene Abweichungen zwischen der Architektur-Dokumentation
(z. B. `hauski-skizze.md`, `hauski-stack.md`) und der tatsächlichen Implementierung im
`crates/` Verzeichnis auf – inklusive Auswirkungen und Vorschlag für das weitere Vorgehen.

---

## 1. Indexierung (`indexd`)

**Dokumentation**

- `indexd` wird als Persistenzschicht beschrieben, die SQLite in Kombination mit einem
  `VectorStore`-Trait nutzt.
- Als Backends werden `tantivy+hnsw` (Default) und `Qdrant` (optional) genannt.
- Erwartung: Vektorsuche + persistente Indizes.

**Code (`crates/indexd/src/lib.rs`)**

- Aktuelle Implementierung: reine In-Memory `HashMap` (`IndexState`).
- Suche: einfacher Substring-Match (`substring_match_score`).
- Es existiert weder ein `VectorStore`-Trait noch eine Anbindung an SQLite, Tantivy oder Qdrant.
- Vektorsuche findet nicht statt, Persistenz ist nur in Ansätzen vorhanden/fehlend.

**Auswirkung**

- Externe Architektur-Dokumente suggerieren Fähigkeiten, die der Code nicht hat:
  - Vektorbasierte Relevanz, skalierbare Indizes, persistente Suchindizes.
- Nutzer, die „Indexd“ als echte Such-Engine erwarten, bekommen faktisch nur einen
  In-Memory-Filter mit String-Matching.
- Performance- und Qualitätsaussagen in der Dokumentation sind aktuell nicht haltbar.

**Empfohlene Maßnahmen**

1. Kurzfristig: In der Doku klar zwischen **aktueller Implementierung** und **Zielbild**
   unterscheiden (z. B. Abschnitt „Ist-Stand vs. Roadmap“).
2. Mittelfristig: Minimal-Version des angekündigten Designs implementieren:
   - `VectorStore`-Trait definieren,
   - ein einfaches SQLite-Backend + Dummy-Vektorbackend anschließen,
   - Doku auf diese Minimal-Realität abgleichen.
3. Wenn die Roadmap unsicher ist: explizit als „geplante Architektur“ markieren und
   nicht als bereits existierend beschreiben.

---

## 2. Fehlende Module (`llm`, `asr`, `tts`, `audio`)

**Dokumentation**

- `hauski-skizze.md` listet unter „2.2 Module“ u. a.:
  - `llm/` (llama.cpp Bindings),
  - `asr/` (whisper-rs),
  - `tts/` (piper-rs),
  - `audio/` (Profile, Audio-Pipeline).
- Diese Module werden als Teil der Gesamtarchitektur dargestellt.

**Code**

- Unter `crates/` existieren diese Verzeichnisse nicht.
- Im Root-`Cargo.toml` werden sie bestenfalls unter „später“ kommentiert.
- Rust-seitig gibt es aktuell keine Implementierung für Inference oder Audio.

**Auswirkung**

- Die Dokumentation vermittelt ein Bild eines „vollständigen AI-Stacks“, das de facto
  nicht vorhanden ist.
- Das erhöht die kognitive Last: Leser müssen ständig raten, was Vision und was Realität ist.
- Fehlersuche („wo ist der llama.cpp-Wrapper?“) ist vorprogrammiert.

**Empfohlene Maßnahmen**

1. Die nicht existierenden Crates in der Doku klar als **Future Modules** kennzeichnen
   (inkl. Hinweis „noch nicht implementiert“).
2. Optional: Dummy-Crates mit minimalem `lib.rs` anlegen, die nur `todo!("geplant")`
   enthalten – dann passt Workspace-Struktur zu den Skizzen.
3. Alternativ: Die Module aus der Architekturzeichnung in ein eigenes Kapitel
   „Langfristige Erweiterungen“ verschieben.

---

## 3. Unimplementierte Routen (Core: Plugins & Cloud)

**Dokumentation**

- Die Architektur beschreibt Plugin-Schnittstellen und Cloud-Fallback-Routing:
  - Plugins sollen zusätzliche Fähigkeiten bereitstellen,
  - Cloud-Fallback für Fälle, in denen lokale Ressourcen nicht genügen.

**Code (`crates/core/src/lib.rs`)**

- `plugin_routes()` und `cloud_routes()` sind Platzhalter:
  - Sie liefern leere `Router`-Instanzen zurück.
  - Kommentare: `// TODO: Implement plugin routes`, `// TODO: Implement cloud routes`.

**Auswirkung**

- Das HTTP-API wirkt von außen „fertig“, bietet intern aber keinen realen
  Erweiterungspunkt für Plugins oder Cloud-Fallback.
- Integratoren, die sich auf die Plugin-/Cloud-Erweiterbarkeit verlassen, laufen ins Leere.
- Doku erzeugt Erwartungen, die maximal als „Design-Absicht“ gelten.

**Empfohlene Maßnahmen**

1. Minimal-Implementierung:
   - Routen anlegen, die wenigstens eine **stabile Fehlerantwort** liefern
     (z. B. `501 Not Implemented`, mit Hinweis auf den geplanten Umfang).
2. Doku ergänzen:
   - Kapitel „Plugin-Schnittstellen“ und „Cloud-Fallback“ mit Status: `planned`.
3. Sobald ein erster realer Anwendungsfall da ist:
   - Kleines MVP-Plugin implementieren (z. B. ein Stub, der nur Metriken ausliest),
   - Cloud-Fallback zunächst als explizite Feature-Flag-Route.

---

## 4. Nutzung von `heimlern` (Bandits / Policy-Learning)

**Status laut Repository**

- `vendor/heimlern-core` und `vendor/heimlern-bandits` sind:
  - Teil des Workspaces,
  - lokal im `vendor/`-Verzeichnis vorhanden.
- Die Crates sind damit kompilierbar und versioniert.

**Inkonsistenz**

- `hauski-core` bindet diese Crates bisher nicht ein.
- Sie werden nur optional im Crate `hauski-policy-api` referenziert (Feature `heimlern`).
- Die in Architektur-Skizzen dargestellte „intelligente Steuerung per Bandits“ ist
  im zentralen Core-Server nicht aktiv.

**Auswirkung**

- Die versprochene „lernende“ Policy-Steuerung existiert derzeit nur als Option am Rand,
  nicht im eigentlichen Herzstück.
- Nutzer, die die Skizze lesen, erwarten adaptive Entscheidungen, bekommen aber
  eine weitgehend statische Policy-Engine.

**Empfohlene Maßnahmen**

1. In der Architektur-Doku deutlich machen:
   - `heimlern` ist aktuell ein **optional aktivierbares Experiment**, kein Standardteil.
2. In `hauski-core` an mindestens einer klar definierten Stelle einen Hook vorsehen,
   über den `heimlern` injected werden kann (z. B. `PolicyEngine::new(…heimlern…)`).
3. Ein kleines, messbares Szenario definieren:
   - „Wenn heimlern aktiv ist, wird X über Bandits entschieden, sonst über statische Policy.“
   - Dokumentation mit diesem realen, nachvollziehbaren Beispiel ergänzen.

---

## Aktualisierungs-Historie

**2025-12-03:** Dokumentation aktualisiert zur Klärung Ist-Stand vs. Roadmap

Die folgenden Maßnahmen wurden umgesetzt:

1. **Neues Dokument erstellt:** [`docs/ist-stand-vs-roadmap.md`](./ist-stand-vs-roadmap.md)
   - Vollständige Übersicht über implementierte Features (✅) und geplante Erweiterungen (🔮)
   - Priorisierung: P1 (kurzfristig), P2 (mittelfristig), P3 (langfristig)
   - Detaillierte Status-Tabelle für alle Hauptkomponenten

2. **Architektur-Dokumente aktualisiert:**
   - `hauski-skizze.md`: Hinweis am Anfang hinzugefügt, dass es sich um eine Vision handelt
   - `hauski-stack.md`: Status-Marker (✅/🔮) für alle Komponenten ergänzt
   - Modul-Übersicht in beiden Dokumenten mit klarer Trennung Ist/Roadmap

3. **Status-Übersicht:**
   - **Indexd:** In-Memory-Implementierung dokumentiert, Vektor-/Persistenz-Features als P2 geplant
   - **LLM/ASR/TTS/Audio:** Explizit als "nicht implementiert, geplant P1" gekennzeichnet
   - **Plugins & Cloud-Fallback:** Status "leere Platzhalter" dokumentiert, als P2 geplant
   - **Heimlern:** Als "optionales Feature in policy_api" dokumentiert, Integration in core als P2 geplant

Diese Änderungen erfüllen die in diesem Dokument unter "Empfohlene Maßnahmen" (jeweils Punkt 1)
beschriebenen kurzfristigen Schritte: klare Unterscheidung zwischen aktuellem Stand und Zielbild.
