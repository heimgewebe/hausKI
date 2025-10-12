use chrono::{Local, Utc};
use serde_json::Value;
use std::{
    fs::{create_dir_all, OpenOptions},
    io::Write,
    path::PathBuf,
};

pub fn write_event_line(kind: &str, payload: &Value) {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base: PathBuf = std::env::var("HAUSKI_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".hauski"));
    let dir = base.join("events");

    if let Err(err) = create_dir_all(&dir) {
        eprintln!("failed to create event directory: {err}");
        return;
    }

    let file = dir.join(format!("{}.jsonl", Local::now().format("%Y-%m")));
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

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(file, "{}", line);
    }
}
