//! Read queries that back `type-cli stats`. Each returns plain, UI-shaped structs (no engine/stats
//! types) so the rendering layer stays decoupled from SQL.

use anyhow::{Context, Result};

use crate::storage::Store;

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

/// Total number of stored *real* runs (drives the empty-state message). Practice drills
/// (`source = 'retry'`) don't count toward the user's stats.
pub fn run_count(store: &Store) -> Result<i64> {
    store
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM test_run WHERE source != 'retry'",
            [],
            |r| r.get(0),
        )
        .context("count runs")
}

/// Per-character aggregates across all runs, worst-first. Only characters typed at least
/// `min_sample` times are returned (so a single fat-finger doesn't dominate the heatmap).
pub fn key_aggregates(store: &Store, min_sample: i64) -> Result<Vec<KeyAgg>> {
    let mut stmt = store.conn().prepare(
        "SELECT expected_char, SUM(typed_total) AS tt, SUM(error_count) AS ec
         FROM char_stat
         JOIN test_run ON char_stat.run_id = test_run.id
         WHERE test_run.source != 'retry'
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

/// One letter ranked by how slowly it is typed (average per-keystroke latency).
#[derive(Debug, Clone, PartialEq)]
pub struct SlowLetter {
    pub ch: String,
    pub avg_latency_ms: f64,
    pub samples: i64,
}

/// The slowest letters across all real runs (highest average latency), worst-first. Only letters
/// with at least `min_sample` latency samples qualify, and practice drills are excluded. Seeds the
/// slow-letter practice drill.
pub fn slowest_letters(store: &Store, min_sample: i64, limit: usize) -> Result<Vec<SlowLetter>> {
    let mut stmt = store.conn().prepare(
        "SELECT expected_char,
                CAST(SUM(total_latency_ms) AS REAL) / SUM(latency_samples) AS avg,
                SUM(latency_samples) AS n
         FROM char_stat
         JOIN test_run ON char_stat.run_id = test_run.id
         WHERE test_run.source != 'retry'
         GROUP BY expected_char
         HAVING n >= ?1
         ORDER BY avg DESC, expected_char ASC
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map([min_sample, limit as i64], |r| {
            Ok(SlowLetter {
                ch: r.get(0)?,
                avg_latency_ms: r.get(1)?,
                samples: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()
        .context("read slowest letters")?;
    Ok(rows)
}

/// One WPM aggregate bucket for the history chart: the average is `sum_wpm / count`.
#[derive(Debug, Clone, PartialEq)]
pub struct BucketRow {
    /// Calendar key (e.g. `2026-06`) or, for session granularity, the run id.
    pub bucket: String,
    /// `"time"` or `"words"` — lets the chart filter by test type.
    pub mode: String,
    pub sum_wpm: f64,
    pub count: i64,
    /// Earliest `started_at` in the bucket (chronological sort key).
    pub first_started: i64,
}

/// WPM aggregated for the history chart, grouped per `(bucket, mode)` and ordered oldest-first.
/// With `strftime_fmt = Some(fmt)` runs are bucketed into local-time calendar periods (day/week/
/// month/year); with `None` each run is its own bucket ("session" granularity). Practice drills are
/// always excluded. Caller combines/filters the per-mode rows into the displayed series.
pub fn period_buckets(store: &Store, strftime_fmt: Option<&str>) -> Result<Vec<BucketRow>> {
    let read = |r: &rusqlite::Row| -> rusqlite::Result<BucketRow> {
        Ok(BucketRow {
            bucket: r.get(0)?,
            mode: r.get(1)?,
            sum_wpm: r.get(2)?,
            count: r.get(3)?,
            first_started: r.get(4)?,
        })
    };
    let rows = match strftime_fmt {
        Some(fmt) => {
            let mut stmt = store.conn().prepare(
                "SELECT strftime(?1, started_at/1000, 'unixepoch', 'localtime') AS bucket,
                        mode, SUM(wpm) AS sum_wpm, COUNT(*) AS n, MIN(started_at) AS first
                 FROM test_run
                 WHERE source != 'retry'
                 GROUP BY bucket, mode
                 ORDER BY first ASC",
            )?;
            // Bind to a local so the row iterator's borrow of `stmt` ends before `stmt` drops.
            let rows = stmt
                .query_map([fmt], read)?
                .collect::<rusqlite::Result<Vec<_>>>();
            rows
        }
        None => {
            let mut stmt = store.conn().prepare(
                "SELECT CAST(id AS TEXT) AS bucket, mode, wpm AS sum_wpm, 1 AS n,
                        started_at AS first
                 FROM test_run
                 WHERE source != 'retry'
                 ORDER BY started_at ASC, id ASC",
            )?;
            let rows = stmt
                .query_map([], read)?
                .collect::<rusqlite::Result<Vec<_>>>();
            rows
        }
    }
    .context("read period buckets")?;
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
                CharStatRow::new("e", 30, 6),
                CharStatRow::new("t", 25, 1),
                CharStatRow::new("z", 2, 2), // below min-sample
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

    /// A practice drill (excluded from analytics): `source = "retry"`, with a skewed char_stat.
    fn practice_run(started_at: i64, wpm: f64) -> RunRecord {
        let mut r = run(started_at, wpm);
        r.source = "retry";
        // Wildly skewed so any leak into analytics would be obvious.
        r.char_stats = vec![CharStatRow {
            expected_char: "e".into(),
            typed_total: 10_000,
            error_count: 10_000,
            total_latency_ms: 9_999_000,
            latency_samples: 10_000,
        }];
        r
    }

    /// A run with explicit per-letter latency: `stats` is `(char, total_latency_ms, latency_samples)`.
    fn latency_run(started_at: i64, source: &'static str, stats: &[(&str, i64, i64)]) -> RunRecord {
        let mut r = run(started_at, 80.0);
        r.source = source;
        r.char_stats = stats
            .iter()
            .map(|(c, total, n)| CharStatRow {
                expected_char: (*c).into(),
                typed_total: *n,
                error_count: 0,
                total_latency_ms: *total,
                latency_samples: *n,
            })
            .collect();
        r
    }

    #[test]
    fn run_count_excludes_practice_runs() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &run(100, 70.0)).unwrap();
        insert_run(&mut s, &practice_run(200, 50.0)).unwrap();
        assert_eq!(run_count(&s).unwrap(), 1, "practice drill must not count");
    }

    #[test]
    fn key_aggregates_exclude_practice_runs() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &run(100, 70.0)).unwrap();
        insert_run(&mut s, &practice_run(200, 50.0)).unwrap(); // skewed 'e' must not leak
        let aggs = key_aggregates(&s, 20).unwrap();
        let e = aggs.iter().find(|a| a.ch == "e").unwrap();
        assert_eq!(e.typed_total, 30, "only the real run's 'e' counts");
        assert_eq!(e.error_count, 6);
    }

    #[test]
    fn slowest_letters_orders_by_avg_latency_desc_and_gates_min_sample() {
        let mut s = Store::open_in_memory().unwrap();
        // 'q' avg 300ms over 20 samples; 'e' avg 100ms over 20; 'z' avg 999ms but only 2 samples.
        insert_run(
            &mut s,
            &latency_run(
                100,
                "random",
                &[("q", 6000, 20), ("e", 2000, 20), ("z", 1998, 2)],
            ),
        )
        .unwrap();
        let slow = slowest_letters(&s, 20, 10).unwrap();
        let chars: Vec<&str> = slow.iter().map(|l| l.ch.as_str()).collect();
        assert_eq!(
            chars,
            vec!["q", "e"],
            "z gated by min-sample; q slower than e"
        );
        assert!((slow[0].avg_latency_ms - 300.0).abs() < 1e-9);
    }

    #[test]
    fn slowest_letters_excludes_practice_runs() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &latency_run(100, "random", &[("e", 2000, 20)])).unwrap();
        insert_run(&mut s, &latency_run(200, "retry", &[("q", 99_000, 20)])).unwrap();
        let slow = slowest_letters(&s, 20, 10).unwrap();
        let chars: Vec<&str> = slow.iter().map(|l| l.ch.as_str()).collect();
        assert_eq!(
            chars,
            vec!["e"],
            "the practice run's slow 'q' must be excluded"
        );
    }

    #[test]
    fn period_buckets_group_by_day_and_exclude_practice() {
        let mut s = Store::open_in_memory().unwrap();
        // Two runs >2 days apart (TZ-robust: distinct local days in any timezone), at ~noon UTC.
        let day1 = 1_700_000_000_000; // arbitrary epoch ms
        let day2 = day1 + 3 * 86_400_000; // +3 days
        insert_run(&mut s, &run(day1, 70.0)).unwrap();
        insert_run(&mut s, &run(day2, 90.0)).unwrap();
        insert_run(&mut s, &practice_run(day2, 10.0)).unwrap(); // excluded
        let buckets = period_buckets(&s, Some("%Y-%m-%d")).unwrap();
        assert_eq!(buckets.len(), 2, "two distinct days, practice excluded");
        assert!(buckets[0].first_started < buckets[1].first_started); // oldest first
        assert_ne!(buckets[0].bucket, buckets[1].bucket);
        assert!(buckets.iter().all(|b| b.count == 1 && b.mode == "time"));
    }

    #[test]
    fn period_buckets_session_is_one_point_per_run() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &run(100, 70.0)).unwrap();
        insert_run(&mut s, &run(200, 90.0)).unwrap();
        insert_run(&mut s, &practice_run(300, 10.0)).unwrap();
        let buckets = period_buckets(&s, None).unwrap();
        assert_eq!(
            buckets.len(),
            2,
            "one point per real run, practice excluded"
        );
        assert_eq!(buckets[0].sum_wpm, 70.0);
        assert_eq!(buckets[1].sum_wpm, 90.0);
    }
}
