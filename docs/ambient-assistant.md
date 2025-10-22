# Ambient Assistant Hooks (OS-Kontext)

hausKI orchestriert kontextbasierte Aktionen.

## Neue Playbooks
- `hold_notifications(mode=deepwork)`
- `release_notifications()`
- `summarize_alert_to_daily_digest(alert_id)`

## Beispiel-Fluss
1. mitschreiber sendet `os.context.state` (focus=true, app=code).
2. heimlern setzt Modus `deep_work`.
3. Breaking News kommen über aussensensor → hausKI fragt heimlern → “hold”.
4. hausKI plant `summarize_alert_to_daily_digest()` (semantAH).

## UI-Hinweise (lokal)
- dezente Banner/Toasts (“Ähnliches in Projekt-XYZ gefunden”)
- “Snooze” / “Don’t show during Deep Work”.
