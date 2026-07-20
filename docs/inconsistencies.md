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
**Gültigkeit:** Bis zur Einführung echter Vektorsuche (Roadmap P2).

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
**Gültigkeit:** Bis zur Implementierung der nativen Inference-Layer (Roadmap P1).

**Dokumentation**
- `hauski-skizze.md` listet Module wie `llm/` (llama.cpp), `asr/` (whisper-rs) etc. auf.

**Realität (Code)**
- Diese Verzeichnisse existieren nicht. Rust-seitig gibt es keine Inference-Implementierung.

**Begründung**
- Die Integration nativer KI-Bindings ist der nächste große Entwicklungsschritt.
- Bis dahin wird Inference über externe APIs oder Python-Microservices (z.B. via `uv`) gelöst.

---

## 3. Unimplementierte Routen (Core: Plugins & Cloud)

**Status:** `accepted limitation` (Plugins) / `stubbed` (Cloud)
**Gültigkeit:** Unbegrenzt, bis konkrete Feature-Anforderungen (z.B. Sync) entstehen.

**Dokumentation**
- Beschreibt Plugin-Schnittstellen und Cloud-Fallback-Routing.

**Realität (Code)**
- **Plugins:** Rudimentäre Registry, liefert leere Listen/404.
- **Cloud:** Routen existieren, geben aber fest verdrahtet `501 Not Implemented` zurück.

**Begründung**
- Das API-Schema ("Contract") ist definiert, um Frontend-Entwicklung zu ermöglichen.
- Die Backend-Logik wird erst bei konkretem Bedarf implementiert.

---

## 4. Historische `heimlern`-Kompatibilität

**Status:** `frozen local compatibility`
**Gültigkeit:** Keine aktive Lern-, Routing- oder Policy-Autorität.

**Realität (Code)**
- `hauski-policy-api` besitzt weiterhin das optionale Feature `heimlern`.
- Das Feature verwendet ausschließlich die lokalen Crates `vendor/heimlern-core` und
  `vendor/heimlern-bandits`; es ist standardmäßig deaktiviert.
- Die Crates sind kleine HausKI-Kompatibilitätsschichten und kein Spiegel der historischen
  Heimlern-Implementierung.
- `scripts/verify_heimlern_freeze.py` prüft bei jedem Build-, Lint- und Testeinstieg die lokale
  Cargo-Auflösung, gebundene Dateidigestwerte und das Fehlen von Remote- oder Direktpfaden.

**Begründung**
- Die bestehende API-Kompatibilität bleibt reproduzierbar, ohne das historische Heimlern-Repository
  als Laufzeit-, Fetch- oder Entwicklungsabhängigkeit fortzuführen.
- Eine Erweiterung zu aktiver Lernlogik benötigt einen neuen, separat registrierten Nachweis und
  darf nicht durch die Kompatibilitätsschicht eingeschmuggelt werden.
