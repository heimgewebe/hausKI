use std::{
    borrow::Cow,
    fmt,
    hash::Hash,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use once_cell::sync::OnceCell;
use prometheus_client::{
    encoding::{EncodeLabelSet, LabelSetEncoder},
    metrics::{counter::Counter, family::Family},
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;

// ---------- Metrik-Labels (bleiben wie in A1) ----------

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MemoryLabels<'a> {
    pub namespace: Cow<'a, str>,
    pub layer: Cow<'a, str>,
}
impl<'a> EncodeLabelSet for MemoryLabels<'a> {
    fn encode(&self, encoder: &mut LabelSetEncoder) -> fmt::Result {
        use prometheus_client::encoding::EncodeLabel;
        ("namespace", self.namespace.as_ref()).encode(encoder.encode_label())?;
        ("layer", self.layer.as_ref()).encode(encoder.encode_label())?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct EvictLabels<'a> {
    pub reason: Cow<'a, str>, // "expired" | "manual"
}
impl<'a> EncodeLabelSet for EvictLabels<'a> {
    fn encode(&self, encoder: &mut LabelSetEncoder) -> fmt::Result {
        use prometheus_client::encoding::EncodeLabel;
        ("reason", self.reason.as_ref()).encode(encoder.encode_label())?;
        Ok(())
    }
}

// ---------- Public API ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub key: String,
    pub value: Vec<u8>,
    pub ttl_sec: Option<i64>,
    pub pinned: bool,
    pub created_ts: DateTime<Utc>,
    pub updated_ts: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub pinned: u64,
    pub unpinned: u64,
    pub expired_evictions_total: u64,
}

#[derive(Clone, Debug)]
pub struct MemoryConfig {
    /// Optionaler Pfad zur DB-Datei. Default: $XDG_STATE_HOME/hauski/memory.db
    pub db_path: Option<PathBuf>,
    /// Janitor-Intervall in Sekunden (Default 60).
    pub janitor_interval_secs: u64,
}
impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            db_path: None,
            janitor_interval_secs: 60,
        }
    }
}

pub struct MemoryStore {
    pub(crate) db_path: PathBuf,
    // Metriken (werden in A3 an die Core-Registry gehängt)
    pub(crate) ops_total: Family<MemoryLabels<'static>, Counter>,
    pub(crate) evictions_total: Family<EvictLabels<'static>, Counter>,
    pub(crate) _janitor: JoinHandle<()>,
}

static GLOBAL: OnceCell<MemoryStore> = OnceCell::new();
static EXPIRED_EVICTIONS_TOTAL: AtomicU64 = AtomicU64::new(0);

pub fn expired_evictions_total() -> u64 {
    EXPIRED_EVICTIONS_TOTAL.load(Ordering::Relaxed)
}

pub fn init_default() -> Result<&'static MemoryStore> {
    init_with(MemoryConfig::default())
}

pub fn init_with(cfg: MemoryConfig) -> Result<&'static MemoryStore> {
    let base = dirs::state_dir().unwrap_or_else(|| {
        // Fallback in $HOME/.local/state
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".local/state")
    });
    let db_path = cfg.db_path.unwrap_or_else(|| base.join("hauski").join("memory.db"));
    std::fs::create_dir_all(db_path.parent().unwrap())
        .with_context(|| format!("create parent dir for {:?}", db_path))?;

    // ensure schema exists
    {
        let conn = Connection::open(&db_path)
            .with_context(|| format!("open sqlite at {:?}", db_path))?;
        conn.execute_batch(
            r#"
            PRAGMA journal_mode=WAL;
            CREATE TABLE IF NOT EXISTS memory_items(
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                ttl_sec INTEGER NULL,
                pinned INTEGER NOT NULL DEFAULT 0,
                created_ts TEXT NOT NULL,
                updated_ts TEXT NOT NULL
            );
            "#,
        )?;
    }

    // spawn janitor
    let interval = cfg.janitor_interval_secs.max(5);
    let jp = tokio::spawn(janitor_task(db_path.clone(), interval));

    let store = MemoryStore {
        db_path,
        ops_total: Family::default(),
        evictions_total: Family::default(),
        _janitor: jp,
    };
    Ok(GLOBAL.get_or_init(|| store))
}

pub fn global() -> &'static MemoryStore {
    GLOBAL.get().expect("hauski-memory not initialized; call init_default() early")
}

impl MemoryStore {
    pub fn set(&self, key: &str, value: &[u8], ttl_sec: Option<i64>, pinned: Option<bool>) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let pinned_i = if pinned.unwrap_or(false) { 1 } else { 0 };
        let conn = Connection::open(&self.db_path)?;

        // Bewahre created_ts, wenn vorhanden; sonst jetzt
        let created: Option<String> = conn
            .query_row(
                "SELECT created_ts FROM memory_items WHERE key=?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        let created_ts = created.unwrap_or_else(|| now.clone());

        conn.execute(
            r#"INSERT INTO memory_items(key,value,ttl_sec,pinned,created_ts,updated_ts)
                VALUES (?1,?2,?3,?4,?5,?6)
                ON CONFLICT(key) DO UPDATE SET
                    value=excluded.value,
                    ttl_sec=excluded.ttl_sec,
                    pinned=excluded.pinned,
                    updated_ts=excluded.updated_ts;"#,
            params![key, value, ttl_sec, pinned_i, created_ts, now],
        )?;

        let c = self.ops_total.get_or_create(&MemoryLabels{ namespace: Cow::Borrowed("default"), layer: Cow::Borrowed("short_term")});
        c.inc();
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<Item>> {
        let conn = Connection::open(&self.db_path)?;
        let row = conn
            .query_row(
                r#"SELECT key, value, ttl_sec, pinned, created_ts, updated_ts
                    FROM memory_items WHERE key=?1"#,
                params![key],
                |r| {
                    let pinned_i: i64 = r.get(3)?;
                    let created: String = r.get(4)?;
                    let updated: String = r.get(5)?;
                    Ok(Item {
                        key: r.get(0)?,
                        value: r.get(1)?,
                        ttl_sec: r.get(2)?,
                        pinned: pinned_i != 0,
                        created_ts: created.parse().unwrap_or_else(|e| {
                            tracing::warn!(error = ?e, "failed to parse created_ts");
                            Utc::now()
                        }),
                        updated_ts: updated.parse().unwrap_or_else(|e| {
                            tracing::warn!(error = ?e, "failed to parse updated_ts");
                            Utc::now()
                        }),
                    })
                },
            )
            .optional()?;

        let c = self.ops_total.get_or_create(&MemoryLabels{ namespace: Cow::Borrowed("default"), layer: Cow::Borrowed("short_term")});
        c.inc();
        Ok(row)
    }

    pub fn evict(&self, key: &str) -> Result<bool> {
        let conn = Connection::open(&self.db_path)?;
        let n = conn.execute("DELETE FROM memory_items WHERE key=?1", params![key])?;
        if n > 0 {
            let c = self.evictions_total.get_or_create(&EvictLabels{ reason: Cow::Borrowed("manual") });
            c.inc();
        }
        Ok(n > 0)
    }

    pub fn stats(&self) -> Result<Stats> {
        let conn = Connection::open(&self.db_path)?;
        let (pinned, unpinned) = conn.query_row(
            "SELECT
                COUNT(CASE WHEN pinned = 1 THEN 1 END),
                COUNT(CASE WHEN pinned = 0 THEN 1 END)
            FROM memory_items",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(Stats {
            pinned,
            unpinned,
            expired_evictions_total: expired_evictions_total(),
        })
    }
}

async fn janitor_task(db_path: PathBuf, every_secs: u64) {
    let d = Duration::from_secs(every_secs);
    loop {
        tokio::time::sleep(d).await;
        if let Ok(conn) = Connection::open(&db_path) {
            // Lösche abgelaufene TTLs, wenn nicht gepinnt
            let n = conn.execute(
                r#"DELETE FROM memory_items
                    WHERE pinned=0
                        AND ttl_sec IS NOT NULL
                        AND (strftime('%s','now') - strftime('%s', updated_ts)) > ttl_sec"#,
                [],
            );
            if let Ok(count) = n {
                if count > 0 {
                    EXPIRED_EVICTIONS_TOTAL.fetch_add(count as u64, Ordering::Relaxed);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Test-interne Hilfsfunktion, die einen isolierten Store für jeden Test erstellt.
    /// Gibt den Store und das TempDir zurück, um dessen Lebensdauer an den Test zu binden.
    fn test_store(janitor_interval_secs: u64) -> (MemoryStore, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("m.db");

        // Schema-Erstellung (wie in init_with)
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                r#"
                PRAGMA journal_mode=WAL;
                CREATE TABLE IF NOT EXISTS memory_items(
                    key TEXT PRIMARY KEY, value BLOB NOT NULL, ttl_sec INTEGER NULL,
                    pinned INTEGER NOT NULL DEFAULT 0, created_ts TEXT NOT NULL, updated_ts TEXT NOT NULL
                );"#,
            )
            .unwrap();
        }

        let jp = tokio::spawn(janitor_task(db_path.clone(), janitor_interval_secs));

        let store = MemoryStore {
            db_path,
            ops_total: Family::default(),
            evictions_total: Family::default(),
            _janitor: jp,
        };
        (store, tmp)
    }

    #[tokio::test]
    async fn set_get_evict_roundtrip() {
        let (store, _tmp) = test_store(60);
        store.set("k", "v".as_bytes(), Some(5), Some(false)).unwrap();
        let it = store.get("k").unwrap().unwrap();
        assert_eq!(it.key, "k");
        assert_eq!(it.value, b"v");
        assert!(store.evict("k").unwrap());
        assert!(store.get("k").unwrap().is_none());
    }

    #[tokio::test]
    async fn janitor_expires() {
        let (store, _tmp) = test_store(1);
        store.set("k", "v".as_bytes(), Some(1), Some(false)).unwrap();
        tokio::time::sleep(Duration::from_secs(3)).await;
        // allow janitor to run
        tokio::time::sleep(Duration::from_secs(2)).await;
        let got = store.get("k").unwrap();
        assert!(got.is_none(), "expected TTL expiry");
    }
}
