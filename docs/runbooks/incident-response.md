# Runbook: Incident-Response

Status: _draft_ • Owner: Platform/SRE • Geltungsbereich: hausKI Core + Subdienste (indexd, policy, observability)

## 1. Ziele
- Schnelle Wiederherstellung des Dienstes bei Störungen
- Klare Eskalationswege, Rollback-Optionen und Nachbereitung (Postmortem)

## 2. Definitionen & Schweregrade
| Severity | Wirkung | Beispiel |
|---|---|---|
| **SEV1** | Vollständiger Ausfall / massive SLA-Verletzung | `/ready` 503 > 5 min; p95-Latenz (ms) > Budget×3 |
| **SEV2** | Teil-Ausfall / deutliche Degradation | erhöhte Fehlerrate, p95-Latenz (ms) > Budget×2 |
| **SEV3** | Leichte Degradation | p95-Latenz (ms) > Budget×1.2, vereinzelte 5xx |

**Budget-Referenz:** `policies/limits.yaml` (z. B. LLM p95-Latenz (ms), Index p95-Latenz (ms), Thermal-Limits)

## 3. Erstmaßnahmen (Triage)
1. **Alarm bestätigen** (Pager/Chat) – *wer übernimmt Incident Commander (IC)?*
2. **Gesundheit prüfen**
   - `GET /ready` und `GET /health`
   - Falls `/ready` (noch) fehlt: `/health` als Primärsignal verwenden
   - `GET /metrics` (Prometheus-Text): Fehlerraten, p95-Latenz (ms), Thermik
3. **Kontext sammeln**
   - Letzte Deployments/Config-Änderungen (PR/Changelog)
   - Logs (tracing) mit `RUST_LOG=info` bzw. `debug` temporär
4. **Severity festlegen** (SEV1/2/3) und **Kommunikationskanal** eröffnen (Incident-Thread)

## 4. Eskalation
- **SEV1:** IC + Platform/SRE + Modul-Owner sofort; Kommunikations-Cadence 10 min
- **SEV2:** IC + betroffener Modul-Owner; Cadence 20–30 min
- **SEV3:** reguläre Bearbeitung, Status stündlich

Owner-Beispiele:
- **indexd:** Search/Index Team
- **policy:** Governance Team
- **observability:** Platform Team

## 5. Diagnose-Checkliste
- **Ressourcen/Thermik**
  - `gpu_temperature_c`, `dgpu_power_watt` im `/metrics`
  - Node-Auslastung (CPU/RAM/IO) via Plattform-Tools
- **Fehlerraten & Latenzen**
  - HTTP-Fehlerhistogramme (z. B. `http_request_duration_seconds_bucket`)
  - Index-spezifische Histogramme (Search/Upsert)
- **Konfiguration**
  - Letzte Änderungen in `policies/limits.yaml`, `routing.yaml`, `models.yaml`
  - ENV Overrides (z. B. `HAUSKI_SAFE_MODE`)
- **Abhängigkeiten**
  - Downstream/Upstream Reachability (Routing-Policy, DNS, Timeout)

## 6. Sofortmaßnahmen (Workarounds)
- **Traffic drosseln/abschalten**: temporäre Rate Limits oder Deny in `routing.yaml`
- **Safe Mode** aktivieren: `HAUSKI_SAFE_MODE=true` (nur wenn definiert & sinnvoll)
- **Limits anheben**: Latenzbudget temporär erhöhen (nur wenn notwendig, dokumentieren!)
- **Feature-Flags** deaktivieren, die akute Last/Fehler pushen

## 7. Rollback / Rollforward
1. **Rollback**
   - Letztes bekannt gutes Release/Commit ausrollen
   - Bestätigen: `/ready`, `/health`, p95-Latenz (ms) innerhalb Budget
2. **Rollforward**
   - Fix-Branch bauen/deployen
   - Canary/Smoke: `/health`, `/metrics` prüfen

**Wichtig:** Jede Änderung im Incident-Channel notieren (Wer/Was/Wann/Warum)

## 8. Abschluss & Kommunikation
- **Incident schließen**, wenn Metriken stabil (mind. 30 min im grünen Bereich)
- **Postmortem** (spätestens 48 h):
  - Timeline, Root Cause, Impact, Maßnahmen (kurz/mittel/lang)
  - Ticket-Links, PRs, Follow-ups

## 9. Artefakte / Hilfsmittel
- `/metrics` Prometheus-Format
- Logs/Tracing (rotierend via `tracing-appender`)
- Policies unter `policies/*`
- Runbooks: [Troubleshooting](../runbooks/troubleshooting.md), [Upgradepfade](./upgrade.md)

## 10. Beispiel-Kommandos
```bash
# Health & Ready
curl -sf http://127.0.0.1:8080/health
curl -sf http://127.0.0.1:8080/ready

# Metriken kurz prüfen
curl -s http://127.0.0.1:8080/metrics | head -n 50
```

## 11. Checkliste
- [ ] Incident Commander bestimmt und Kommunikationskanal geöffnet
- [ ] Severity festgelegt und Eskalation informiert
- [ ] `/health` (ggf. `/ready`) und zentrale Metriken geprüft
- [ ] Workaround/Rollback dokumentiert und wirksam
- [ ] Postmortem-Termin angesetzt, Follow-ups erfasst
