# Modul-Übersicht

Die Cargo-Workspace-Struktur von HausKI folgt klaren Domänen. Die folgenden Fokusseiten beschreiben Funktionsumfang, APIs und typische Workflows der wichtigsten Crates.

- [Core](core.md) – HTTP-API, Authentifizierung und Policy-Enforcement
- [Memory](memory.md) – Speicher-Schichten für kurzfristige und langfristige Kontexte
- [Audio](audio.md) – PipeWire-Facade, Profile und CLI-Workflows

Weitere Module wie `embeddings`, `indexd` oder `policy` orientieren sich an den gleichen Prinzipien: klare Ownership, Feature-Flags für riskante Integrationen und harte Performance-Grenzen.
