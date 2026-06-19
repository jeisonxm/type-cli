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
        assert_eq!(v, 1);
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
}
