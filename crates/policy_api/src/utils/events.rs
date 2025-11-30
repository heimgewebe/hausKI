//! Event logging utilities for policy decisions.
//!
//! This module provides functionality to write structured event logs
//! in JSONL format for auditing and analysis purposes.

use chrono::{Local, Utc};
use serde_json::Value;
use std::{
    fs::{create_dir_all, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tracing::warn;

/// Writes an event line to the events log file.
///
/// Events are written to `$HAUSKI_DATA/events/YYYY-MM.jsonl` (or `~/.hauski/events/YYYY-MM.jsonl`
/// if `HAUSKI_DATA` is not set). Each event includes an ID, timestamp, node ID, kind, and payload.
///
/// # Arguments
///
/// * `kind` - The type of event being logged
/// * `payload` - JSON value containing event-specific data
///
/// # Panics
///
/// This function does not panic. Errors are logged as warnings and do not propagate.
pub fn write_event_line(kind: &str, payload: &Value) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base: PathBuf =
        std::env::var("HAUSKI_DATA").map_or_else(|_| home.join(".hauski"), PathBuf::from);
    let dir = base.join("events");

    if let Err(err) = create_dir_all(&dir) {
        warn!(error = %err, "failed to create event directory");
        return;
    }

    let file_path = dir.join(format!("{}.jsonl", Local::now().format("%Y-%m")));
    let id = ulid::Ulid::new().to_string();
    let ts = Utc::now().timestamp_millis();
    let node_id = hostname::get()
        .map(|v| v.to_string_lossy().into_owned())
        .unwrap_or_default();
    let line = serde_json::json!({
        "id": id,
        "node_id": node_id,
        "ts": ts,
        "kind": kind,
        "payload": payload,
    });

    match serde_json::to_string(&line) {
        Ok(json_line) => {
            if let Err(err) = append_line(&file_path, &json_line) {
                warn!(
                    path = %file_path.display(),
                    error = %err,
                    "failed to write event line"
                );
            }
        }
        Err(err) => {
            warn!(error = %err, "failed to serialize event payload");
        }
    }
}

fn append_line(path: &Path, line: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{line}")?;
    Ok(())
}
