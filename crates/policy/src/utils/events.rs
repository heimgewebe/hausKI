use chrono::{Datelike, SecondsFormat, Utc};
use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use tracing::warn;

fn events_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| ".".into());
    let base: PathBuf = std::env::var("HAUSKI_DATA")
        .map(Into::into)
        .unwrap_or(home.join(".hauski"));
    base.join("events")
}

pub fn write_event_line(event_type: &str, payload: &Value) {
    let now = Utc::now();
    let dir = events_dir();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        warn!(error = %err, "failed to create events directory");
        return;
    }

    let file_name = format!("{:04}-{:02}.jsonl", now.year(), now.month());
    let path = dir.join(file_name);

    let entry = json!({
        "ts": now.to_rfc3339_opts(SecondsFormat::Millis, true),
        "event": event_type,
        "payload": payload,
    });

    match serde_json::to_string(&entry) {
        Ok(line) => {
            if let Err(err) = append_line(&path, &line) {
                warn!(path = %path.display(), error = %err, "failed to write event line");
            }
        }
        Err(err) => warn!(error = %err, "failed to serialize event payload"),
    }
}

fn append_line(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", line)?;
    Ok(())
}
