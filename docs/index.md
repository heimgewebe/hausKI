# HausKI-Dokumentation

Willkommen bei **HausKI**, der lokalen KI-Orchestrierungsplattform für Pop!_OS. Dieses Portal bündelt Architekturüberblick, Betriebsleitfäden und modulare Referenzen, damit du schnell von der Installation zur täglichen Nutzung kommst.

## Schnellstart

```bash
just py-init       # Python-Abhängigkeiten für Tools & Docs installieren
just py-docs-serve # MkDocs-Server lokal starten
just py-lint       # Ruff-Linting (Python-Hilfswerkzeuge)
just py-fmt        # Ruff-Formatter ausführen
```

Weitere Einstiegspunkte:

- [README](https://github.com/heimgewebe/hausKI/blob/main/README.md) – Gesamtüberblick, Setup und Devcontainer-Workflows
<!-- - [hauski-stack.md](./hauski-stack.md) – Tech-Stack, Architekturrollen und Roadmap -->
<!-- - [hauski-skizze.md](./hauski-skizze.md) – visuelle Skizze der Systemkomponenten -->

## Inhaltsverzeichnis

| Bereich | Beschreibung |
| --- | --- |
| Architektur <!-- (./hauski-stack.md) --> | Stack-Entscheidungen, Modulzuschnitt und Budgets |
| [Module](modules/index.md) | Fokusseiten zu Kern-Crates wie `core`, `memory` und `audio` |
| [Runbooks](runbooks/index.md) | Schritt-für-Schritt-Anleitungen für Setup und Betrieb |
| [semantAH](semantah.md) | Deep Dive in Embeddings, KPIs und Deploy |
| [Prozesse](process/README.md) | Sprachleitfaden, Tests, Release-Checks |

## Aktuelle Schwerpunkte

1. **Hot-Path in Rust:** Latenzkritische Pfade laufen konsequent in Rust, mit klaren FFI-Grenzen zu KI-Modellen.
2. **Budget-Governance:** Observability und harte p95-Limits sichern Performance, Strombudget und Thermik.
3. **Lokaler Vorrang:** Offline-Modelle sind Default; Cloud-Zugriffe benötigen Policies und Audit.

## Nächste Schritte

- Lokale Modelle vorbereiten? → siehe [`runbooks/semantah_local.md`](runbooks/semantah_local.md).
- API testen? → starte `just run-core` und rufe den Stub `/v1/chat` auf (`501 Not Implemented` bis zur LLM-Anbindung).
- Audio-Setup anpassen? → folge den Profilen und CLI-Workflows in [Audio](modules/audio.md).

Viel Erfolg beim Bauen, Betreiben und Erweitern von HausKI!
