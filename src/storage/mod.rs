//! SQLite persistence (Phase 2). The impure shell that owns the database connection.
//!
//! Kept entirely out of `engine`/`stats` (which stay pure): those compute the numbers, this writes
//! them. Like `config.rs`, opening the store is best-effort — callers treat a failure as "no
//! persistence this run", never a crash.

pub mod queries;
pub mod runs;
mod schema;

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;

pub use runs::{insert_run, CharStatRow, RunRecord, WorstWordRow};

/// A handle to the on-disk (or in-memory) database, already migrated to the latest schema.
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (creating if needed) the database at `path`, apply pragmas, and run migrations.
    /// Creates the parent directory if missing.
    pub fn open(path: &Path) -> Result<Store> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("creating data dir {}", dir.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("opening database {}", path.display()))?;
        Self::from_conn(conn)
    }

    /// An ephemeral in-memory store (used by tests).
    pub fn open_in_memory() -> Result<Store> {
        Self::from_conn(Connection::open_in_memory()?)
    }

    fn from_conn(conn: Connection) -> Result<Store> {
        // Durability/concurrency pragmas. WAL + NORMAL is the standard "fast and safe enough" combo
        // for a local app; foreign_keys must be enabled per-connection for ON DELETE CASCADE to fire.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )
        .context("applying pragmas")?;

        let mut store = Store { conn };
        schema::migrations()
            .to_latest(&mut store.conn)
            .context("running migrations")?;
        Ok(store)
    }

    /// Read-only access to the underlying connection (used by the stats queries in Phase 2).
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_run_from_empty_db() {
        let store = Store::open_in_memory().expect("open + migrate");
        // user_version reflects the one applied migration.
        let v: i64 = store
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);
        // The seeded local user exists.
        let n: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM user WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn migrations_are_idempotent_across_reopens() {
        let dir = std::env::temp_dir().join(format!("typecli-mig-{}", std::process::id()));
        let path = dir.join("type-cli.db");
        let _ = std::fs::remove_dir_all(&dir);

        // Open twice on the same file: the second open must not re-apply or error.
        let s1 = Store::open(&path).expect("first open");
        drop(s1);
        let s2 = Store::open(&path).expect("second open");
        let users: i64 = s2
            .conn()
            .query_row("SELECT COUNT(*) FROM user", [], |r| r.get(0))
            .unwrap();
        assert_eq!(users, 1, "the local user is seeded exactly once");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn upgrades_v1_database_to_v2_preserving_runs() {
        // Build a v1 database by hand (the pre-latency `char_stat`, marked user_version = 1).
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE user (id INTEGER PRIMARY KEY, username TEXT NOT NULL, created_at INTEGER NOT NULL);
             CREATE TABLE test_run (id INTEGER PRIMARY KEY, user_id INTEGER NOT NULL, mode TEXT NOT NULL,
                 target INTEGER NOT NULL, source TEXT NOT NULL, source_ref TEXT, language TEXT,
                 wpm REAL NOT NULL, raw_wpm REAL NOT NULL, accuracy REAL NOT NULL, consistency REAL,
                 chars_correct INTEGER NOT NULL DEFAULT 0, chars_incorrect INTEGER NOT NULL DEFAULT 0,
                 chars_extra INTEGER NOT NULL DEFAULT 0, chars_missed INTEGER NOT NULL DEFAULT 0,
                 elapsed_ms INTEGER NOT NULL, started_at INTEGER NOT NULL, created_at INTEGER NOT NULL);
             CREATE TABLE char_stat (id INTEGER PRIMARY KEY, run_id INTEGER NOT NULL,
                 expected_char TEXT NOT NULL, typed_total INTEGER NOT NULL DEFAULT 0,
                 error_count INTEGER NOT NULL DEFAULT 0);
             INSERT INTO user (id, username, created_at) VALUES (1,'local',0);
             INSERT INTO test_run (user_id, mode, target, source, wpm, raw_wpm, accuracy,
                 elapsed_ms, started_at, created_at)
                 VALUES (1,'time',60,'random',80,83,97,60000,1000,61000);
             INSERT INTO char_stat (run_id, expected_char, typed_total, error_count) VALUES (1,'e',30,5);
             PRAGMA user_version = 1;",
        )
        .unwrap();

        // Opening through the Store runs migration 2 (the additive latency columns).
        let store = Store::from_conn(conn).expect("upgrade v1 → v2");
        let v: i64 = store
            .conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 2);
        let runs: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM test_run", [], |r| r.get(0))
            .unwrap();
        assert_eq!(runs, 1, "existing runs survive the upgrade");
        let (tot, n): (i64, i64) = store
            .conn()
            .query_row(
                "SELECT total_latency_ms, latency_samples FROM char_stat WHERE run_id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((tot, n), (0, 0), "legacy rows default to zero latency");
    }
}
