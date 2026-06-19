//! Read queries that back `type-cli stats`. Each returns plain, UI-shaped structs (no engine/stats
//! types) so the rendering layer stays decoupled from SQL.

use anyhow::{Context, Result};

use crate::storage::Store;

/// One finished run reduced to a timeline point (for the history chart).
#[derive(Debug, Clone, PartialEq)]
pub struct RunPoint {
    pub started_at: i64,
    pub wpm: f64,
    pub accuracy: f64,
}

/// Aggregated attempts/errors for one expected character, across all runs.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyAgg {
    pub ch: String,
    pub typed_total: i64,
    pub error_count: i64,
}

impl KeyAgg {
    /// Error rate in `[0, 1]` (0 when never typed).
    pub fn error_rate(&self) -> f64 {
        if self.typed_total == 0 {
            0.0
        } else {
            self.error_count as f64 / self.typed_total as f64
        }
    }
}

/// Total number of stored runs (drives the empty-state message).
pub fn run_count(store: &Store) -> Result<i64> {
    store
        .conn()
        .query_row("SELECT COUNT(*) FROM test_run", [], |r| r.get(0))
        .context("count runs")
}

/// The most recent `limit` runs as timeline points, oldest-first (left-to-right on the chart).
pub fn recent_runs(store: &Store, limit: usize) -> Result<Vec<RunPoint>> {
    // `id` breaks ties so the chart order is deterministic when two runs share `started_at`.
    let mut stmt = store.conn().prepare(
        "SELECT started_at, wpm, accuracy FROM (
             SELECT id, started_at, wpm, accuracy FROM test_run
             ORDER BY started_at DESC, id DESC LIMIT ?1
         ) ORDER BY started_at ASC, id ASC",
    )?;
    let rows = stmt
        .query_map([limit as i64], |r| {
            Ok(RunPoint {
                started_at: r.get(0)?,
                wpm: r.get(1)?,
                accuracy: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("read recent runs")?;
    Ok(rows)
}

/// Per-character aggregates across all runs, worst-first. Only characters typed at least
/// `min_sample` times are returned (so a single fat-finger doesn't dominate the heatmap).
pub fn key_aggregates(store: &Store, min_sample: i64) -> Result<Vec<KeyAgg>> {
    let mut stmt = store.conn().prepare(
        "SELECT expected_char, SUM(typed_total) AS tt, SUM(error_count) AS ec
         FROM char_stat
         GROUP BY expected_char
         HAVING tt >= ?1
         ORDER BY ec DESC, expected_char ASC",
    )?;
    let rows = stmt
        .query_map([min_sample], |r| {
            Ok(KeyAgg {
                ch: r.get(0)?,
                typed_total: r.get(1)?,
                error_count: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("read key aggregates")?;
    Ok(rows)
}

/// Worst words of the most recent run, by rank, capped at `limit`. Seeds "retry worst words".
pub fn most_recent_worst_words(store: &Store, limit: usize) -> Result<Vec<String>> {
    let mut stmt = store.conn().prepare(
        "SELECT word FROM worst_word
         WHERE run_id = (SELECT id FROM test_run ORDER BY started_at DESC, id DESC LIMIT 1)
         ORDER BY rank ASC LIMIT ?1",
    )?;
    let rows = stmt
        .query_map([limit as i64], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("read worst words")?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{insert_run, CharStatRow, RunRecord, WorstWordRow};

    fn run(started_at: i64, wpm: f64) -> RunRecord {
        RunRecord {
            mode: "time",
            target: 60,
            source: "random",
            source_ref: None,
            language: Some("english".into()),
            wpm,
            raw_wpm: wpm + 3.0,
            accuracy: 97.0,
            consistency: Some(90.0),
            chars_correct: 200,
            chars_incorrect: 5,
            chars_extra: 0,
            chars_missed: 0,
            elapsed_ms: 60_000,
            started_at,
            created_at: started_at + 60_000,
            char_stats: vec![
                CharStatRow {
                    expected_char: "e".into(),
                    typed_total: 30,
                    error_count: 6,
                },
                CharStatRow {
                    expected_char: "t".into(),
                    typed_total: 25,
                    error_count: 1,
                },
                CharStatRow {
                    expected_char: "z".into(),
                    typed_total: 2,
                    error_count: 2,
                }, // below min-sample
            ],
            worst_words: vec![WorstWordRow {
                word: "the".into(),
                error_count: 3,
                word_wpm: Some(40.0),
                rank: 1,
            }],
        }
    }

    #[test]
    fn recent_runs_are_oldest_first_and_capped() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &run(300, 90.0)).unwrap();
        insert_run(&mut s, &run(100, 70.0)).unwrap();
        insert_run(&mut s, &run(200, 80.0)).unwrap();

        let pts = recent_runs(&s, 2).unwrap();
        assert_eq!(pts.len(), 2); // capped
        assert_eq!(pts[0].started_at, 200); // oldest of the two newest, first
        assert_eq!(pts[1].started_at, 300);
        assert_eq!(run_count(&s).unwrap(), 3);
    }

    #[test]
    fn key_aggregates_gate_min_sample_and_rank_worst_first() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &run(100, 70.0)).unwrap();
        insert_run(&mut s, &run(200, 80.0)).unwrap();

        let aggs = key_aggregates(&s, 20).unwrap();
        // 'z' (typed 4 total across 2 runs) is below the min-sample gate of 20.
        assert!(aggs.iter().all(|a| a.ch != "z"));
        // 'e' has the most errors → first.
        assert_eq!(aggs[0].ch, "e");
        assert_eq!(aggs[0].error_count, 12); // 6 per run × 2
        assert!((aggs[0].error_rate() - 0.2).abs() < 1e-9); // 12 / 60
    }

    #[test]
    fn worst_words_come_from_the_latest_run() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &run(100, 70.0)).unwrap();
        insert_run(&mut s, &run(999, 80.0)).unwrap(); // latest
        let words = most_recent_worst_words(&s, 10).unwrap();
        assert_eq!(words, vec!["the"]);
    }

    #[test]
    fn latest_run_breaks_started_at_ties_by_id() {
        let mut s = Store::open_in_memory().unwrap();
        let mut first = run(500, 70.0);
        first.worst_words = vec![WorstWordRow {
            word: "first".into(),
            error_count: 1,
            word_wpm: None,
            rank: 1,
        }];
        let mut second = run(500, 80.0); // same started_at, inserted later → higher id
        second.worst_words = vec![WorstWordRow {
            word: "second".into(),
            error_count: 1,
            word_wpm: None,
            rank: 1,
        }];
        insert_run(&mut s, &first).unwrap();
        insert_run(&mut s, &second).unwrap();
        let words = most_recent_worst_words(&s, 10).unwrap();
        assert_eq!(words, vec!["second"], "tie broken by id → later run wins");
    }
}
