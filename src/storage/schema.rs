//! Database schema as inline migrations (`rusqlite_migration`, state in `PRAGMA user_version`).
//!
//! Migration 1 is the full Phase 2 schema designed in `docs/ARCHITECTURE.md`. Later phases add
//! migrations here; every added column is nullable/defaulted so earlier rows stay valid (no rewrites).

use rusqlite_migration::{Migrations, M};

/// All migrations, applied in order to bring a database up to the latest schema.
pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(MIGRATION_1)])
}

/// Migration 1: tables, indexes, and a seeded `local` user (id = 1).
const MIGRATION_1: &str = r#"
CREATE TABLE user (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL,
    remote_id TEXT UNIQUE,
    created_at INTEGER NOT NULL
);

CREATE TABLE preset (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    mode TEXT NOT NULL CHECK(mode IN ('time','words')),
    target INTEGER NOT NULL,
    is_builtin INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE test_run (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES user(id),
    preset_id INTEGER REFERENCES preset(id) ON DELETE SET NULL,
    mode TEXT NOT NULL CHECK(mode IN ('time','words')),
    target INTEGER NOT NULL,
    source TEXT NOT NULL CHECK(source IN ('random','pdf','docx','quote','retry')),
    source_ref TEXT,
    language TEXT,
    wpm REAL NOT NULL,
    raw_wpm REAL NOT NULL,
    accuracy REAL NOT NULL,
    consistency REAL,
    chars_correct INTEGER NOT NULL DEFAULT 0,
    chars_incorrect INTEGER NOT NULL DEFAULT 0,
    chars_extra INTEGER NOT NULL DEFAULT 0,
    chars_missed INTEGER NOT NULL DEFAULT 0,
    elapsed_ms INTEGER NOT NULL,
    ghost_of_run_id INTEGER REFERENCES test_run(id) ON DELETE SET NULL,
    remote_id TEXT UNIQUE,
    synced_at INTEGER,
    started_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE char_stat (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES test_run(id) ON DELETE CASCADE,
    expected_char TEXT NOT NULL,
    typed_total INTEGER NOT NULL DEFAULT 0,
    error_count INTEGER NOT NULL DEFAULT 0,
    UNIQUE(run_id, expected_char)
);

CREATE TABLE worst_word (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES test_run(id) ON DELETE CASCADE,
    word TEXT NOT NULL,
    error_count INTEGER NOT NULL DEFAULT 0,
    word_wpm REAL,
    rank INTEGER NOT NULL
);

CREATE TABLE keystroke_event (
    id INTEGER PRIMARY KEY,
    run_id INTEGER NOT NULL REFERENCES test_run(id) ON DELETE CASCADE,
    t_offset_ms INTEGER NOT NULL,
    char_index INTEGER NOT NULL,
    typed_char TEXT,
    is_correct INTEGER NOT NULL DEFAULT 0,
    is_backspace INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_run_user_time ON test_run(user_id, started_at);
CREATE INDEX idx_run_user_mode ON test_run(user_id, mode, target, started_at);
CREATE INDEX idx_charstat_run  ON char_stat(run_id);
CREATE INDEX idx_worstword_run ON worst_word(run_id, rank);
CREATE INDEX idx_run_unsynced  ON test_run(synced_at) WHERE synced_at IS NULL;

INSERT INTO user (id, username, created_at)
VALUES (1, 'local', CAST(strftime('%s','now') AS INTEGER) * 1000);
"#;
