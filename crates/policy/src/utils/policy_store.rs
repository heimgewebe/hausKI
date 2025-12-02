use anyhow::{Context, Result};
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

// Helper: Synchronous connection setup (to be called inside spawn_blocking)
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
/// Async-Wrapper um blockierende SQLite-Calls.
pub async fn save_snapshot(name: String, snapshot: Value) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let c = conn()?;
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

/// Lädt Snapshot JSON, falls vorhanden.
/// Async-Wrapper um blockierende SQLite-Calls.
pub async fn load_snapshot(name: String) -> Result<Option<Value>> {
    let res = tokio::task::spawn_blocking(move || {
        let c = conn()?;
        let mut stmt = c.prepare("SELECT snapshot_json FROM policy_param WHERE name=?1")?;
        let mut rows = stmt.query(params![name])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            Ok::<Option<Value>, anyhow::Error>(serde_json::from_str(&s).ok())
        } else {
            Ok(None)
        }
    })
    .await
    .context("spawn_blocking join failed")??;
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_save_and_load_snapshot() {
        // Use a random name to avoid collisions if running against a shared DB (though tests should ideally use isolated DBs)
        // Since we can't easily inject the DB path in this helper without refactoring `db_path()`,
        // we'll rely on the fact that this is a local dev environment test.
        // Better: refactor to allow injecting DB path, but for now strict adherence to existing logic with async wrapper.
        let name = format!("test_key_{}", chrono::Utc::now().timestamp_millis());
        let data = json!({"foo": "bar", "baz": 123});

        save_snapshot(name.clone(), data.clone()).await.expect("save failed");

        let loaded = load_snapshot(name.clone()).await.expect("load failed");
        assert_eq!(loaded, Some(data));

        let missing = load_snapshot("non_existent_key_9999".to_string()).await.expect("load missing failed");
        assert!(missing.is_none());
    }
}
