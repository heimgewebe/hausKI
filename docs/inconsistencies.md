# Inkonsistenzen zwischen Dokumentation und Code

Dieses Dokument listet gefundene Abweichungen zwischen der Architektur-Dokumentation (z. B. `hauski-skizze.md`, `hauski-stack.md`) und der tatsächlichen Implementierung im `crates/` Verzeichnis auf.

## 1. Indexierung (`indexd`)

*   **Dokumentation:** Beschreibt `indexd` als Persistenzschicht, die SQLite in Kombination mit einem `VectorStore`-Trait nutzt. Als Backends werden `tantivy+hnsw` (Default) und `Qdrant` (optional) genannt.
*   **Code (`crates/indexd/src/lib.rs`):** Die aktuelle Implementierung ist eine reine In-Memory `HashMap` (`IndexState`). Die Suche erfolgt über einen einfachen Substring-Match (`substring_match_score`). Es gibt weder ein `VectorStore`-Trait noch eine Anbindung an SQLite, Tantivy oder Qdrant. Vektorsuche findet nicht statt.

## 2. Fehlende Module

*   **Dokumentation:** In `hauski-skizze.md` (Abschnitt "2.2 Module") werden folgende Crates als existierend aufgeführt:
    *   `llm/` (llama.cpp Binding)
    *   `asr/` (whisper-rs)
    *   `tts/` (piper-rs)
    *   `audio/` (Profile)
*   **Code:** Diese Verzeichnisse existieren nicht unter `crates/`. Im Root-`Cargo.toml` sind sie unter "weitere später" kommentiert. Die Funktionalität für Inference und Audio ist im aktuellen Rust-Code nicht enthalten.

## 3. Unimplementierte Routen (Core)

*   **Dokumentation:** Die Architektur sieht Plugin-Schnittstellen und Cloud-Fallback-Routing vor.
*   **Code (`crates/core/src/lib.rs`):** Die Funktionen `plugin_routes()` und `cloud_routes()` sind Platzhalter, die leere Router zurückgeben. Sie enthalten `TODO`-Kommentare ("Implement plugin routes", "Implement cloud routes").

## 4. Nutzung von `heimlern`

*   **Status:** Die Crates `heimlern-core` und `heimlern-bandits` liegen im `vendor/`-Verzeichnis und sind Workspace-Member.
*   **Inkonsistenz:** Das Haupt-Crate `hauski-core` bindet diese Bibliotheken nicht ein. Sie werden lediglich optional im `hauski-policy-api` Crate referenziert (`[features] heimlern`). Die in der Skizze angedeutete intelligente Steuerung (z. B. via Bandits) ist im Core-Server somit nicht aktiv.
