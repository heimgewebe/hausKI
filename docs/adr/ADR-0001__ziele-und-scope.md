# ADR-0001: Ziele und Scope

- Status: Accepted
- Datum: 2025-09-29

## Kontext

Die HausKI soll als integriertes System für Smart-Home-Assistenten entstehen. Das Zielbild wird in der [HausKI-Skizze](../../hauski-skizze.md) beschrieben und dient als Grundlage für Architektur- und Produktentscheidungen.

## Entscheidung

Wir etablieren ein zweigeteiltes Zielbild:

- **Lokale HausKI-Instanz**: Kernfunktionen laufen on-premise auf dedizierter Hardware beim Endnutzer, um Datenschutz, Latenz und Offline-Fähigkeit sicherzustellen.
- **HausKI-Orchestrator**: Eine optionale Orchestrator-Schicht koordiniert mehrere lokale Instanzen, ermöglicht zentrale Verwaltung und aggregiert nicht-personenbezogene Telemetrie für Verbesserungen.

Diese Struktur bildet den Rahmen für folgende ADRs zu Security, Memory und Audio.

## Out of Scope

- Jede Form von Cloud-Lock-in oder verpflichtender externer Abhängigkeiten fällt außerhalb des Scopes. Cloud-Komponenten dürfen nur optional und austauschbar sein.

## Abwägung

- **Lokal**: Bietet maximale Kontrolle über Daten, niedrige Latenzen und hohe Resilienz gegen Ausfälle externer Dienste. Erfordert jedoch höheren Installations- und Wartungsaufwand pro Haushalt.
- **Cloud**: Ermöglicht schnelle Skalierung, vereinfachte Updates und zentralisierte Services, birgt jedoch Datenschutzrisiken, potentielle Kostenfallen sowie Abhängigkeiten von Drittanbietern.

Wir priorisieren die lokale Ausführung als Default und erlauben optionale Cloud-Orchestrierung nur, wenn sie den Prinzipien der Austauschbarkeit und Datensouveränität genügt.
