# Sprachrichtlinie für HausKI

Diese Richtlinie beschreibt Tonalität, Wortwahl und Formatkonventionen für das gesamte Repository. Sie gilt für Markdown-Dokumentation, Commit-Messages, Pull-Request-Texte sowie alle Kommentare in Quellcode-Dateien.

## Grundsatz

- **Primärsprache:** Deutsch nach aktueller Duden-Empfehlung, klar und knapp formuliert, konsistente Du-Ansprache.
- **Gender-Schreibweisen:** Keine Sonderzeichen wie `*`, `:`, `·`, `_` oder Binnen-I (`EntwicklerInnen`). Verwende neutrale Formulierungen.
- **Anglizismen:** Nur wenn sie etablierte Fachbegriffe oder API-Namen sind.

## Kontextabhängige Sprache

| Bereich | Sprache | Hinweise |
| --- | --- | --- |
| Benutzeroberfläche, Produkttexte, ADRs, Architektur-Notizen | Deutsch | Prägnant, aktiv, ohne Floskeln. |
| Commit-Messages & PR-Beschreibungen | Deutsch | Conventional-Commits-Schema beibehalten (`feat: …`, `fix: …`). |
| Code-Kommentare, TODOs, Testnamen, Log-Meldungen | Englisch | Kurze Hauptsätze, Beschreibungen im Präsens, keine Umgangssprache. |
| APIs & Konfigurationsschlüssel | Englisch | CamelCase oder snake_case nach Kontext, keine Umlaute. |

## Format- und Datumsangaben

- **UI & Texte:** Datum im Format `TT.MM.JJJJ`, Uhrzeit 24-Stunden (`14:35`), Dezimalzahlen mit Komma (`3,14`).
- **Code, Protokolle & APIs:** ISO-8601 (`2024-06-01T14:35:00Z`), Dezimalpunkt (`3.14`), SI-Einheiten mit Leerzeichen (`12 ms`).
- **Schreibweisen:** Markennamen original belassen (`GitHub`, `Rust`, `NVIDIA`).

## Durchsetzung & Tools

Vale prüft sowohl Prosa als auch Kommentare. Die Konfiguration liegt in [.vale.ini](../../.vale.ini).

- `vale .` lokal ausführen oder den entsprechenden CI-Schritt abwarten.
- Markdown-Dateien unterliegen dem Style `hauski/GermanProse` (Deutsch erlaubt, Gender-Sonderzeichen verboten).
- Quellcode-Kommentare werden mit `hauski/GermanComments` geprüft (erzwingt Englisch in Kommentaren).
- Warnungen und Fehler im CI müssen vor dem Merge behoben werden.

## Review-Checkliste (Kurzfassung)

1. Passt die Sprache zur Zielgruppe (Deutsch vs. Englisch) und zum Kontext?
2. Vermeidet der Text Gender-Sonderzeichen und hält sich an die Formatvorgaben?
3. Bestehen Vale, `cargo fmt`, `cargo clippy`, `cargo test` und `cargo-deny` lokal sowie im CI?
4. Sind Commit-Messages deutsch, folgen aber dem Conventional-Commits-Präfix?

> **Hinweis:** Diese Richtlinie lehnt sich an die entsprechenden Dokumente in den Schwesterprojekten an und sorgt für eine einheitliche Sprache über alle Repositories hinweg.
