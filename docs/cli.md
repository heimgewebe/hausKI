---
title: "wgx CLI Referenz"
description: "Kurzreferenz für wichtige wgx-Unterbefehle und Hinweise zur Nutzung."
---

# wgx CLI Referenz

Die folgenden Abschnitte fassen die wichtigsten Unterbefehle des `wgx`-Werkzeugs
zusammen. Für die vollständige, stets aktuelle Übersicht empfiehlt sich ein Blick
in die integrierte Hilfe via `wgx --help` beziehungsweise `wgx --list`.

```text
Usage:
  wgx [command]

Commands (Auszug):
  release    Automatisierte Veröffentlichungsprozesse.
  reload     Aktualisiert lokale Abhängigkeiten anhand des Basis-Branches.
  selftest   Führt Selbsttests und Diagnosen aus.
  send       Versendet vordefinierte Artefakte oder Statusmeldungen.
  setup      Richtet das lokale Projektumfeld ein.
  start      Startet Dienste oder Entwicklungsumgebungen.
  status     Zeigt den aktuellen Projektstatus an.
  sync       Synchronisiert Arbeitsstände.
  task       Arbeitet mit Aufgaben aus Runbooks oder Tickets.
  tasks      Listet verfügbare Aufgaben.
  test       Führt Testsuiten aus.
  validate   Prüft Artefakte oder Konfigurationen.
  version    Zeigt die Werkzeugversion an.

Env:
  WGX_BASE       Basis-Branch für reload (default: main)

More:
  wgx --list     Nur verfügbare Befehle anzeigen
```

## Commands

### audit

```text
Usage:
  wgx audit verify [--strict]

Verwaltet das Audit-Ledger von wgx.
```

### clean

```text
Usage:
  wgx clean [--safe] [--build] [--git] [--deep] [--dry-run] [--force]

Options:
  --safe       Entfernt temporäre Cache-Verzeichnisse (Standard).
  --build      Löscht Build-Artefakte (dist, build, target, ...).
  --git        Räumt gemergte Branches und Remote-Referenzen auf (nur sauberer Git-Tree).
  --deep       Führt ein destruktives `git clean -xfd` aus (erfordert --force, nur sauberer Git-Tree).
  --dry-run    Zeigt nur an, was passieren würde.
  --force      Bestätigt destruktive Operationen (für --deep).
```

### config

```text
Usage:
  wgx config [--show] [--set KEY=VALUE]
```

Weitere Unterbefehle folgen den gleichen Grundprinzipien: mit `--help` erhält man
die ausführliche Dokumentation zu Parametern und Seiteneffekten. Ergänzungen oder
Korrekturen zu dieser Übersicht können direkt in dieser Datei vorgenommen werden.
