//! Writing a finished run to the database: one `test_run` row plus its `char_stat` and `worst_word`
//! children, in a single transaction.

use anyhow::{Context, Result};
use rusqlite::params;

use crate::storage::Store;

/// The local user every run is attributed to (seeded by migration 1). Multi-user is a later phase.
const LOCAL_USER_ID: i64 = 1;

/// One per-character aggregate row (`char_stat`).
#[derive(Debug, Clone)]
pub struct CharStatRow {
    pub expected_char: String,
    pub typed_total: i64,
    pub error_count: i64,
}

/// One mistyped-word row (`worst_word`).
#[derive(Debug, Clone)]
pub struct WorstWordRow {
    pub word: String,
    pub error_count: i64,
    pub word_wpm: Option<f64>,
    pub rank: i64,
}

/// A complete finished run, flattened into plain DB-shaped values (no engine/stats types). The app
/// layer builds this from a `Summary` + session at the moment a test finishes.
#[derive(Debug, Clone)]
pub struct RunRecord {
    pub mode: &'static str, // "time" | "words"
    pub target: i64,
    pub source: &'static str, // "random" | "pdf" | "docx" | "quote" | "retry"
    pub source_ref: Option<String>,
    pub language: Option<String>,
    pub wpm: f64,
    pub raw_wpm: f64,
    pub accuracy: f64,
    pub consistency: Option<f64>,
    pub chars_correct: i64,
    pub chars_incorrect: i64,
    pub chars_extra: i64,
    pub chars_missed: i64,
    pub elapsed_ms: i64,
    /// Wall-clock epoch ms when the test started (timeline key) and when the row was written.
    pub started_at: i64,
    pub created_at: i64,
    pub char_stats: Vec<CharStatRow>,
    pub worst_words: Vec<WorstWordRow>,
}

/// Insert a run and its children atomically. Returns the new `test_run.id`.
pub fn insert_run(store: &mut Store, rec: &RunRecord) -> Result<i64> {
    let tx = store.conn.transaction().context("begin transaction")?;

    tx.execute(
        "INSERT INTO test_run (
            user_id, mode, target, source, source_ref, language,
            wpm, raw_wpm, accuracy, consistency,
            chars_correct, chars_incorrect, chars_extra, chars_missed,
            elapsed_ms, started_at, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
        params![
            LOCAL_USER_ID,
            rec.mode,
            rec.target,
            rec.source,
            rec.source_ref,
            rec.language,
            rec.wpm,
            rec.raw_wpm,
            rec.accuracy,
            rec.consistency,
            rec.chars_correct,
            rec.chars_incorrect,
            rec.chars_extra,
            rec.chars_missed,
            rec.elapsed_ms,
            rec.started_at,
            rec.created_at,
        ],
    )
    .context("insert test_run")?;
    let run_id = tx.last_insert_rowid();

    for cs in &rec.char_stats {
        tx.execute(
            "INSERT INTO char_stat (run_id, expected_char, typed_total, error_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![run_id, cs.expected_char, cs.typed_total, cs.error_count],
        )
        .context("insert char_stat")?;
    }

    for ww in &rec.worst_words {
        tx.execute(
            "INSERT INTO worst_word (run_id, word, error_count, word_wpm, rank)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![run_id, ww.word, ww.error_count, ww.word_wpm, ww.rank],
        )
        .context("insert worst_word")?;
    }

    tx.commit().context("commit run")?;
    Ok(run_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RunRecord {
        RunRecord {
            mode: "time",
            target: 60,
            source: "random",
            source_ref: None,
            language: Some("english".into()),
            wpm: 92.0,
            raw_wpm: 95.0,
            accuracy: 98.0,
            consistency: Some(94.0),
            chars_correct: 230,
            chars_incorrect: 4,
            chars_extra: 0,
            chars_missed: 1,
            elapsed_ms: 60_000,
            started_at: 1_700_000_000_000,
            created_at: 1_700_000_060_000,
            char_stats: vec![CharStatRow {
                expected_char: "a".into(),
                typed_total: 30,
                error_count: 3,
            }],
            worst_words: vec![WorstWordRow {
                word: "example".into(),
                error_count: 2,
                word_wpm: Some(40.0),
                rank: 1,
            }],
        }
    }

    #[test]
    fn insert_run_round_trips() {
        let mut store = Store::open_in_memory().unwrap();
        let id = insert_run(&mut store, &sample()).unwrap();
        assert!(id > 0);

        let (wpm, acc, src): (f64, f64, String) = store
            .conn()
            .query_row(
                "SELECT wpm, accuracy, source FROM test_run WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!((wpm - 92.0).abs() < 1e-9);
        assert!((acc - 98.0).abs() < 1e-9);
        assert_eq!(src, "random");

        let stats: i64 = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM char_stat WHERE run_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stats, 1);
        let words: i64 = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM worst_word WHERE run_id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(words, 1);
    }

    #[test]
    fn deleting_a_run_cascades_to_children() {
        let mut store = Store::open_in_memory().unwrap();
        let id = insert_run(&mut store, &sample()).unwrap();
        store
            .conn()
            .execute("DELETE FROM test_run WHERE id = ?1", [id])
            .unwrap();
        let orphans: i64 = store
            .conn()
            .query_row("SELECT COUNT(*) FROM char_stat", [], |r| r.get(0))
            .unwrap();
        assert_eq!(orphans, 0, "char_stat rows cascade-deleted with the run");
    }
}
