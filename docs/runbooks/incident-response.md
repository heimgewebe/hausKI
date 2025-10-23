# Runbook: Incident-Response

**Ziel:** Dieses Runbook beschreibt die Schritte zur Diagnose und Behandlung kritischer Störungen im hausKI-Stack.

---

## 1. Auslöser erkennen

Typische Signale:

- Prometheus-Alarm zu Latenz-Budgetverletzung (`latency_llm_p95_ms > 400`)
- Health-Check `/health` liefert `503`
- Crash-Loop oder nicht erreichbarer Port 8080
- HausKI-Review-Daemon meldet „stalled“ Runs

---

## 2. Sofortmaßnahmen

| Bereich | Prüfung | Kommando / Tool |
|----------|----------|-----------------|
| **Service-Status** | Läuft der Core-Prozess? | `ps aux | grep hauski-cli` |
| **Healthcheck** | API verfügbar? | `curl -sf localhost:8080/health` |
| **Metriken** | Exporte verfügbar? | `curl -sf localhost:8080/metrics | head` |
| **Logs** | Fehlerquelle | `journalctl -u hauski.service -n 200` |

> 🔧 **Hinweis:** In dev-Umgebungen kann mit `RUST_LOG=hauski=trace` temporär mehr Detailtiefe aktiviert werden.

---

## 3. Typische Ursachen

- **Speicherlimit erreicht:** `index.db` oder `/tmp` voll → `du -sh ~/.hauski` prüfen.  
- **GPU-Thermik:** `nvidia-smi` zeigt > 80 °C → Policy-Guard greift.  
- **Policy-Violation:** Eintrag in `~/.hauski/review/index.json` → BudgetCheck-Failure.  
- **Netzwerkblockade:** `egress`-Policy verhindert externen Aufruf.  

---

## 4. Wiederherstellung

1. Dienst stoppen: `systemctl --user stop hauski`
2. Logs sichern: `journalctl -u hauski.service > hauski-crash.log`
3. Index ggf. komprimieren: `sqlite3 ~/.hauski/index.db VACUUM;`
4. Dienst starten: `systemctl --user start hauski`

---

## 5. Nachbereitung

- Review-Report erzeugen (`hauski review`)  
- Incident im `leitstand` protokollieren  
- ggf. ADR-Ergänzung oder Limit-Anpassung dokumentieren  

---

**Letzte Aktualisierung:** 2025-10-23

---

## Verknüpfte Dokumente

- [Modul: Observability](../modules/observability.md) — beschreibt Metriken, Budgets und Health-Endpunkte  
- [Audit-Bericht](../audit-hauski.md) — Überblick über CI-/Governance-Audits und Folgeempfehlungen  

Diese Seite ist Teil des operativen Doku-Kreislaufs von **hausKI** (Runbooks ↔ Module ↔ Audit).
