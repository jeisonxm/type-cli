//! State for the `type-cli stats` screen: load the analytics once, then drive a tiny read-only loop
//! that can either quit or hand back a "retry the worst words" request.
//!
//! This is the impure shell (it reads the DB); the rendering in `ui::stats_view` is a pure function
//! of this state. Showing this screen is the *opt-in* exception to the stealth UI (ADR-0003): it
//! only appears when the user explicitly runs `type-cli stats`.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::storage::queries::{self, KeyAgg, RunPoint};
use crate::storage::Store;
use crate::ui::theme::Theme;

/// How many recent runs to chart, the heatmap min-sample gate, and how many worst words to offer.
const HISTORY_LIMIT: usize = 60;
const MIN_SAMPLE: i64 = 20;
const WORST_WORDS_LIMIT: usize = 12;

/// What the user asked for when leaving the stats screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsOutcome {
    /// Just exit.
    Quit,
    /// Start a typing drill from these words ("retry worst words").
    Retry(Vec<String>),
}

/// Loaded analytics + the screen's interaction state.
pub struct StatsApp {
    pub theme: Theme,
    pub run_count: i64,
    /// Recent runs, oldest-first (for the history chart).
    pub points: Vec<RunPoint>,
    /// Charting series `(x_index, wpm)`, mirrors `points`.
    pub wpm_series: Vec<(f64, f64)>,
    /// Per-character aggregates across all runs, worst-first (min-sample gated).
    pub key_aggs: Vec<KeyAgg>,
    /// Worst words of the most recent run (seeds a retry drill).
    pub worst_words: Vec<String>,
    pub should_quit: bool,
    pub outcome: StatsOutcome,
}

impl StatsApp {
    /// Load every analytics query the screen needs from `store`.
    pub fn load(store: &Store, theme: &Theme) -> Result<Self> {
        let run_count = queries::run_count(store)?;
        let points = queries::recent_runs(store, HISTORY_LIMIT)?;
        let wpm_series = points
            .iter()
            .enumerate()
            .map(|(i, p)| (i as f64, p.wpm))
            .collect();
        let key_aggs = queries::key_aggregates(store, MIN_SAMPLE)?;
        let worst_words = queries::most_recent_worst_words(store, WORST_WORDS_LIMIT)?;
        Ok(StatsApp {
            theme: theme.clone(),
            run_count,
            points,
            wpm_series,
            key_aggs,
            worst_words,
            should_quit: false,
            outcome: StatsOutcome::Quit,
        })
    }

    /// Whether a "retry worst words" drill is available (the latest run had mistyped words).
    pub fn can_retry(&self) -> bool {
        !self.worst_words.is_empty()
    }

    /// Handle one key press. `q`/`Esc` quits; `r` requests a retry drill when one is available.
    pub fn on_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.outcome = StatsOutcome::Quit;
                self.should_quit = true;
            }
            KeyCode::Char('r') if self.can_retry() => {
                self.outcome = StatsOutcome::Retry(self.worst_words.clone());
                self.should_quit = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{insert_run, CharStatRow, RunRecord, WorstWordRow};
    use crossterm::event::KeyModifiers;

    fn seed_run(store: &mut Store) {
        let rec = RunRecord {
            mode: "time",
            target: 60,
            source: "random",
            source_ref: None,
            language: Some("english".into()),
            wpm: 88.0,
            raw_wpm: 91.0,
            accuracy: 96.0,
            consistency: Some(90.0),
            chars_correct: 200,
            chars_incorrect: 5,
            chars_extra: 0,
            chars_missed: 0,
            elapsed_ms: 60_000,
            started_at: 1000,
            created_at: 61_000,
            char_stats: vec![CharStatRow {
                expected_char: "e".into(),
                typed_total: 40,
                error_count: 8,
            }],
            worst_words: vec![WorstWordRow {
                word: "their".into(),
                error_count: 2,
                word_wpm: Some(35.0),
                rank: 1,
            }],
        };
        insert_run(store, &rec).unwrap();
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn load_builds_series_and_offers_retry() {
        let mut store = Store::open_in_memory().unwrap();
        seed_run(&mut store);
        let app = StatsApp::load(&store, &Theme::fallback()).unwrap();
        assert_eq!(app.run_count, 1);
        assert_eq!(app.wpm_series, vec![(0.0, 88.0)]);
        assert_eq!(app.key_aggs[0].ch, "e");
        assert!(app.can_retry());
    }

    #[test]
    fn r_requests_retry_with_the_worst_words() {
        let mut store = Store::open_in_memory().unwrap();
        seed_run(&mut store);
        let mut app = StatsApp::load(&store, &Theme::fallback()).unwrap();
        app.on_key(key('r'));
        assert!(app.should_quit);
        assert_eq!(app.outcome, StatsOutcome::Retry(vec!["their".into()]));
    }

    #[test]
    fn q_quits_without_retry() {
        let store = Store::open_in_memory().unwrap();
        let mut app = StatsApp::load(&store, &Theme::fallback()).unwrap();
        assert!(!app.can_retry()); // empty DB
        app.on_key(key('r')); // ignored, nothing to retry
        assert!(!app.should_quit);
        app.on_key(key('q'));
        assert!(app.should_quit);
        assert_eq!(app.outcome, StatsOutcome::Quit);
    }
}
