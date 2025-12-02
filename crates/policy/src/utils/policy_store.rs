use rusqlite::{params, Connection};
use serde_json::Value;
use std::path::PathBuf;

fn db_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| ".".into());
    let base: PathBuf = std::env::var("HAUSKI_DATA")
        .map(Into::into)
        .unwrap_or(home.join(".hauski"));
    base.join("state").join("hauski.db")
}

// TODO: rusqlite calls in this module are synchronous and may block async runtimes.
// Consider wrapping them in tokio::task::spawn_blocking or migrating to an async-native
// solution (e.g. tokio-rusqlite or sqlx) for high-concurrency paths.
fn conn() -> rusqlite::Result<Connection> {
    let p = db_path();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let c = Connection::open(p)?;
    c.execute_batch(
        r"
        CREATE TABLE IF NOT EXISTS policy_param (
          name TEXT PRIMARY KEY,
          snapshot_json TEXT NOT NULL,
          updated_at INTEGER NOT NULL
        );
    ",
    )?;
    Ok(c)
}

/// Speichert Snapshot JSON unter `name` (upsert).
pub fn save_snapshot(name: &str, snapshot: &Value) -> rusqlite::Result<()> {
    let c = conn()?;
    let now = chrono::Utc::now().timestamp();
    let js = snapshot.to_string();
    c.execute(
        "INSERT INTO policy_param(name, snapshot_json, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(name) DO UPDATE SET snapshot_json=excluded.snapshot_json, updated_at=excluded.updated_at",
        params![name, js, now],
    )?;
    Ok(())
}

/// Lädt Snapshot JSON, falls vorhanden.
pub fn load_snapshot(name: &str) -> rusqlite::Result<Option<Value>> {
    let c = conn()?;
    let mut stmt = c.prepare("SELECT snapshot_json FROM policy_param WHERE name=?1")?;
    let mut rows = stmt.query(params![name])?;
    if let Some(row) = rows.next()? {
        let s: String = row.get(0)?;
        Ok(serde_json::from_str(&s).ok())
    } else {
        Ok(None)
    }
}
