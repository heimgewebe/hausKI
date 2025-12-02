use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde_json::Value;
use std::path::PathBuf;

fn db_path(custom: Option<PathBuf>) -> PathBuf {
    match custom {
        Some(p) => p,
        None => {
            let home = dirs::home_dir().unwrap_or_else(|| ".".into());
            let base: PathBuf = std::env::var("HAUSKI_DATA")
                .map(Into::into)
                .unwrap_or(home.join(".hauski"));
            base.join("state").join("hauski.db")
        }
    }
}

// Helper: Synchronous connection setup (to be called inside spawn_blocking)
fn conn(custom_path: Option<PathBuf>) -> rusqlite::Result<Connection> {
    let p = db_path(custom_path);
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

/// Internal helper: Stores snapshot JSON under `name` (upsert).
/// Async wrapper around blocking SQLite calls.
async fn save_snapshot_with_path(
    name: String,
    snapshot: Value,
    custom_path: Option<PathBuf>,
) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let c = conn(custom_path)?;
        let now = chrono::Utc::now().timestamp();
        let js = snapshot.to_string();
        c.execute(
            "INSERT INTO policy_param(name, snapshot_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(name) DO UPDATE SET snapshot_json=excluded.snapshot_json, updated_at=excluded.updated_at",
            params![name, js, now],
        )?;
        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("spawn_blocking join failed")??;
    Ok(())
}

/// Internal helper: Loads snapshot JSON if present.
/// Async wrapper around blocking SQLite calls.
async fn load_snapshot_with_path(
    name: String,
    custom_path: Option<PathBuf>,
) -> Result<Option<Value>> {
    let res = tokio::task::spawn_blocking(move || {
        let c = conn(custom_path)?;
        let mut stmt = c.prepare("SELECT snapshot_json FROM policy_param WHERE name=?1")?;
        let mut rows = stmt.query(params![&name])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            let value: Value = serde_json::from_str(&s)
                .with_context(|| format!("failed to deserialize stored JSON snapshot '{}'", name))?;
            Ok::<Option<Value>, anyhow::Error>(Some(value))
        } else {
            Ok(None)
        }
    })
    .await
    .context("spawn_blocking join failed")??;
    Ok(res)
}

/// Stores snapshot JSON under `name` (upsert).
/// Async wrapper around blocking SQLite calls.
pub async fn save_snapshot(name: String, snapshot: Value) -> Result<()> {
    save_snapshot_with_path(name, snapshot, None).await
}

/// Loads snapshot JSON if present.
/// Async wrapper around blocking SQLite calls.
pub async fn load_snapshot(name: String) -> Result<Option<Value>> {
    load_snapshot_with_path(name, None).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_save_and_load_snapshot() {
        // Use a temporary directory to isolate test from shared state
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let test_db_path = temp_dir.path().join("test.db");

        let name = format!("test_key_{}", chrono::Utc::now().timestamp_millis());
        let data = json!({"foo": "bar", "baz": 123});

        save_snapshot_with_path(name.clone(), data.clone(), Some(test_db_path.clone()))
            .await
            .expect("save failed");

        let loaded = load_snapshot_with_path(name.clone(), Some(test_db_path.clone()))
            .await
            .expect("load failed");
        assert_eq!(loaded, Some(data));

        let missing = load_snapshot_with_path(
            "non_existent_key_9999".to_string(),
            Some(test_db_path.clone()),
        )
        .await
        .expect("load missing failed");
        assert!(missing.is_none());
    }
}
