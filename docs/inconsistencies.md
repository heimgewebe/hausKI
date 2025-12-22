# Inkonsistenzen zwischen Dokumentation und Code

Dieses Dokument listet bewusste Abweichungen zwischen der Architektur-Dokumentation
(z. B. `hauski-skizze.md`, `hauski-stack.md`) und der tatsächlichen Implementierung im
`crates/` Verzeichnis auf.

> **Wichtiger Hinweis:**
> Die hier gelisteten Punkte sind keine Bugs, sondern bewusste Grenzen („conscious boundaries“).
> Sie dokumentieren den Unterschied zwischen dem *langfristigen Zielbild* (Vision) und der
> *aktuellen, stabilen Realität* (Code).

---

## 1. Indexierung (`indexd`)

**Status:** `accepted limitation`

**Dokumentation**
- `indexd` wird als Persistenzschicht beschrieben, die SQLite in Kombination mit einem
  `VectorStore`-Trait nutzt.
- Als Backends werden `tantivy+hnsw` (Default) und `Qdrant` (optional) genannt.

**Realität (Code)**
- Aktuelle Implementierung: reine In-Memory `HashMap` (`IndexState`).
- Suche: einfacher Substring-Match (`substring_match_score`).

**Begründung**
- Eine vollwertige Vektorsuche würde die Komplexität und die Dependencies massiv erhöhen.
- Für aktuelle lokale Testszenarien reicht die In-Memory-Lösung aus.

---

## 2. Fehlende Module (`llm`, `asr`, `tts`, `audio`)

**Status:** `planned gap`

**Dokumentation**
- `hauski-skizze.md` listet Module wie `llm/` (llama.cpp), `asr/` (whisper-rs) etc. auf.

**Realität (Code)**
- Diese Verzeichnisse existieren nicht. Rust-seitig gibt es keine Inference-Implementierung.

**Begründung**
- Die Integration nativer KI-Bindings ist der nächste große Entwicklungsschritt (P1).
- Bis dahin wird Inference über externe APIs oder Python-Microservices (z.B. via `uv`) gelöst.

---

## 3. Unimplementierte Routen (Core: Plugins & Cloud)

**Status:** `accepted limitation` (Plugins) / `stubbed` (Cloud)

**Dokumentation**
- Beschreibt Plugin-Schnittstellen und Cloud-Fallback-Routing.

**Realität (Code)**
- **Plugins:** Rudimentäre Registry, liefert leere Listen/404.
- **Cloud:** Routen existieren, geben aber fest verdrahtet `501 Not Implemented` zurück.

**Begründung**
- Das API-Schema ("Contract") ist definiert, um Frontend-Entwicklung zu ermöglichen.
- Die Backend-Logik wird erst bei konkretem Bedarf (z.B. Multidevice-Sync) implementiert.

---

## 4. Nutzung von `heimlern` (Bandits / Policy-Learning)

**Status:** `experimental` / `deprecated assumption`

**Dokumentation**
- Suggeriert eine "intelligente Steuerung" des Core-Servers durch Banditen-Algorithmen.

**Realität (Code)**
- `vendor/heimlern-*` Crates sind vorhanden, aber in `hauski-core` nicht eingebunden.
- Sie werden nur optional in `hauski-policy-api` genutzt.

**Begründung**
- Die Komplexität von Reinforcement Learning im Core-Loop ist für den aktuellen Reifegrad zu hoch.
- Dies ist ein Experimentierfeld, keine Kernfunktionalität.
