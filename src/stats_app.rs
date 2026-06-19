//! State for the `type-cli stats` screen: load the analytics once, then drive a read-only loop that
//! navigates the WPM history graph (period / filter / scroll) or hands back a "practice slow letters"
//! request.
//!
//! This is the impure shell (it reads the DB); the rendering in `ui::stats_view` is a pure function
//! of this state. Showing this screen is the *opt-in* exception to the stealth UI (ADR-0003): it
//! only appears when the user explicitly runs `type-cli stats`.

use std::collections::HashMap;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::storage::queries::{self, BucketRow, KeyAgg};
use crate::storage::Store;
use crate::ui::theme::Theme;

/// Heatmap min-sample gate.
const MIN_SAMPLE: i64 = 20;
/// Slow-letter min latency-sample gate (so one slow keypress doesn't crown a letter).
const MIN_LATENCY_SAMPLE: i64 = 20;
/// How many of the slowest letters a practice drill targets.
const TOP_SLOW_LETTERS: usize = 3;
/// How many points fit in the scrolling chart window.
const WINDOW: usize = 40;

/// Aggregation period for the WPM history chart (cycled with `o`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphPeriod {
    Session,
    Day,
    Week,
    Month,
    Year,
}

impl GraphPeriod {
    /// All periods, in cycle (and precompute) order.
    pub const ALL: [GraphPeriod; 5] = [
        GraphPeriod::Session,
        GraphPeriod::Day,
        GraphPeriod::Week,
        GraphPeriod::Month,
        GraphPeriod::Year,
    ];

    fn next(self) -> GraphPeriod {
        match self {
            GraphPeriod::Session => GraphPeriod::Day,
            GraphPeriod::Day => GraphPeriod::Week,
            GraphPeriod::Week => GraphPeriod::Month,
            GraphPeriod::Month => GraphPeriod::Year,
            GraphPeriod::Year => GraphPeriod::Session,
        }
    }

    /// SQLite `strftime` format for this period, or `None` for per-run "session" granularity.
    fn strftime_fmt(self) -> Option<&'static str> {
        match self {
            GraphPeriod::Session => None,
            GraphPeriod::Day => Some("%Y-%m-%d"),
            GraphPeriod::Week => Some("%Y-%W"),
            GraphPeriod::Month => Some("%Y-%m"),
            GraphPeriod::Year => Some("%Y"),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GraphPeriod::Session => "session",
            GraphPeriod::Day => "day",
            GraphPeriod::Week => "week",
            GraphPeriod::Month => "month",
            GraphPeriod::Year => "year",
        }
    }
}

/// Which test types to chart (cycled with `O`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphFilter {
    All,
    Time,
    Words,
}

impl GraphFilter {
    pub const ALL: [GraphFilter; 3] = [GraphFilter::All, GraphFilter::Time, GraphFilter::Words];

    fn next(self) -> GraphFilter {
        match self {
            GraphFilter::All => GraphFilter::Time,
            GraphFilter::Time => GraphFilter::Words,
            GraphFilter::Words => GraphFilter::All,
        }
    }

    /// Index into the `[all, time, words]` series triple.
    fn index(self) -> usize {
        match self {
            GraphFilter::All => 0,
            GraphFilter::Time => 1,
            GraphFilter::Words => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GraphFilter::All => "all",
            GraphFilter::Time => "time",
            GraphFilter::Words => "words",
        }
    }
}

/// What the user asked for when leaving the stats screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatsOutcome {
    /// Just exit.
    Quit,
    /// Start a practice drill targeting these (slowest-first) letters.
    Practice(Vec<char>),
}

/// Loaded analytics + the screen's interaction state.
pub struct StatsApp {
    pub theme: Theme,
    pub run_count: i64,
    /// Precomputed WPM series keyed by `(period, filter)`; y-values only (x is the window index).
    series: HashMap<(GraphPeriod, GraphFilter), Vec<f64>>,
    /// Per-character aggregates across real runs, worst-first (min-sample gated). Drives the heatmap.
    pub key_aggs: Vec<KeyAgg>,
    /// The user's slowest letters, slowest-first (seeds a practice drill).
    pub slow_letters: Vec<char>,
    pub graph_period: GraphPeriod,
    pub graph_filter: GraphFilter,
    /// Offset (from the oldest point) of the visible window's left edge.
    pub graph_scroll: usize,
    pub should_quit: bool,
    pub outcome: StatsOutcome,
}

impl StatsApp {
    /// Load every analytics query the screen needs, precomputing all `(period, filter)` series.
    pub fn load(store: &Store, theme: &Theme) -> Result<Self> {
        let run_count = queries::run_count(store)?;

        let mut series: HashMap<(GraphPeriod, GraphFilter), Vec<f64>> = HashMap::new();
        for period in GraphPeriod::ALL {
            let rows = queries::period_buckets(store, period.strftime_fmt())?;
            let folded = fold_buckets(&rows);
            for filter in GraphFilter::ALL {
                series.insert((period, filter), folded[filter.index()].clone());
            }
        }

        let key_aggs = queries::key_aggregates(store, MIN_SAMPLE)?;
        let slow_letters = queries::slowest_letters(store, MIN_LATENCY_SAMPLE, TOP_SLOW_LETTERS)?
            .into_iter()
            .filter_map(|l| l.ch.chars().next())
            .collect();

        let mut app = StatsApp {
            theme: theme.clone(),
            run_count,
            series,
            key_aggs,
            slow_letters,
            graph_period: GraphPeriod::Session,
            graph_filter: GraphFilter::All,
            graph_scroll: 0,
            should_quit: false,
            outcome: StatsOutcome::Quit,
        };
        app.graph_scroll = app.max_scroll(); // start anchored to the latest data
        Ok(app)
    }

    /// Whether a slow-letter practice drill is available.
    pub fn can_practice(&self) -> bool {
        !self.slow_letters.is_empty()
    }

    fn current_series(&self) -> &[f64] {
        self.series
            .get(&(self.graph_period, self.graph_filter))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Highest scroll offset for the current view (0 when all points fit the window).
    fn max_scroll(&self) -> usize {
        self.current_series().len().saturating_sub(WINDOW)
    }

    /// The visible window of the current series as `(x_index, wpm)`, x reindexed from 0.
    pub fn visible_series(&self) -> Vec<(f64, f64)> {
        let s = self.current_series();
        let start = self.graph_scroll.min(self.max_scroll());
        let end = (start + WINDOW).min(s.len());
        s[start..end]
            .iter()
            .enumerate()
            .map(|(i, &y)| (i as f64, y))
            .collect()
    }

    /// Short description of the current view for the chart title (e.g. `month · words · ◂2`).
    pub fn graph_title(&self) -> String {
        let from_latest = self
            .max_scroll()
            .saturating_sub(self.graph_scroll.min(self.max_scroll()));
        let scroll = if from_latest > 0 {
            format!(" · ◂{from_latest}")
        } else {
            String::new()
        };
        format!(
            "{} · {}{scroll}",
            self.graph_period.label(),
            self.graph_filter.label()
        )
    }

    /// Handle one key press. `q`/`Esc` quit; `r` requests a practice drill; `o`/`O` cycle the
    /// period/filter; ←/→ scroll the chart.
    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.outcome = StatsOutcome::Quit;
                self.should_quit = true;
            }
            KeyCode::Char('r') if self.can_practice() => {
                self.outcome = StatsOutcome::Practice(self.slow_letters.clone());
                self.should_quit = true;
            }
            // Match by codepoint case (robust across plain vs. kitty terminals); guard against Ctrl.
            KeyCode::Char('o') if !ctrl => {
                self.graph_period = self.graph_period.next();
                self.graph_scroll = self.max_scroll(); // re-anchor to latest for the new series
            }
            KeyCode::Char('O') if !ctrl => {
                self.graph_filter = self.graph_filter.next();
                self.graph_scroll = self.max_scroll();
            }
            KeyCode::Left => {
                self.graph_scroll = self.graph_scroll.saturating_sub(1);
            }
            KeyCode::Right => {
                self.graph_scroll = (self.graph_scroll + 1).min(self.max_scroll());
            }
            _ => {}
        }
    }
}

/// Fold per-`(bucket, mode)` rows into three chronological WPM series: `[all, time, words]`. Each
/// series averages over *all runs* in a bucket (not an average of per-mode averages); a bucket with
/// no runs for a given filter is omitted from that series.
fn fold_buckets(rows: &[BucketRow]) -> [Vec<f64>; 3] {
    #[derive(Default)]
    struct Acc {
        all_sum: f64,
        all_n: i64,
        t_sum: f64,
        t_n: i64,
        w_sum: f64,
        w_n: i64,
        first: i64,
        /// First-seen position in the (already SQL-ordered) rows — a deterministic tiebreaker so
        /// buckets sharing a `first` timestamp (e.g. two runs in the same ms, session granularity)
        /// keep the query's `started_at, id` order instead of HashMap iteration order.
        seq: usize,
    }
    let mut map: HashMap<&str, Acc> = HashMap::new();
    let mut next_seq = 0usize;
    for r in rows {
        let acc = map.entry(r.bucket.as_str()).or_insert_with(|| {
            let seq = next_seq;
            next_seq += 1;
            Acc {
                first: r.first_started,
                seq,
                ..Acc::default()
            }
        });
        acc.all_sum += r.sum_wpm;
        acc.all_n += r.count;
        acc.first = acc.first.min(r.first_started);
        match r.mode.as_str() {
            "time" => {
                acc.t_sum += r.sum_wpm;
                acc.t_n += r.count;
            }
            "words" => {
                acc.w_sum += r.sum_wpm;
                acc.w_n += r.count;
            }
            _ => {}
        }
    }
    let mut accs: Vec<Acc> = map.into_values().collect();
    accs.sort_by_key(|a| (a.first, a.seq));

    let mut all = Vec::new();
    let mut time = Vec::new();
    let mut words = Vec::new();
    for a in &accs {
        if a.all_n > 0 {
            all.push(a.all_sum / a.all_n as f64);
        }
        if a.t_n > 0 {
            time.push(a.t_sum / a.t_n as f64);
        }
        if a.w_n > 0 {
            words.push(a.w_sum / a.w_n as f64);
        }
    }
    [all, time, words]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{insert_run, CharStatRow, RunRecord};

    /// A bare run with no per-char stats. `mode` must be `"time"` or `"words"` (schema CHECK).
    fn base_run(started_at: i64, mode: &'static str, wpm: f64) -> RunRecord {
        RunRecord {
            mode,
            target: 60,
            source: "random",
            source_ref: None,
            language: Some("english".into()),
            wpm,
            raw_wpm: wpm + 3.0,
            accuracy: 96.0,
            consistency: Some(90.0),
            chars_correct: 200,
            chars_incorrect: 5,
            chars_extra: 0,
            chars_missed: 0,
            elapsed_ms: 60_000,
            started_at,
            created_at: started_at + 60_000,
            char_stats: vec![],
            worst_words: vec![],
        }
    }

    /// A run carrying one slow letter's latency.
    fn slow_letter_run(started_at: i64, ch: &str, total_ms: i64, samples: i64) -> RunRecord {
        let mut r = base_run(started_at, "time", 80.0);
        r.char_stats = vec![CharStatRow {
            expected_char: ch.into(),
            typed_total: samples,
            error_count: 0,
            total_latency_ms: total_ms,
            latency_samples: samples,
        }];
        r
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    /// Store seeded with `n` session runs (1 ms apart → same calendar day), wpm = 50 + i.
    fn seeded(n: i64) -> Store {
        let mut s = Store::open_in_memory().unwrap();
        for i in 0..n {
            insert_run(&mut s, &base_run(1000 + i, "time", 50.0 + i as f64)).unwrap();
        }
        s
    }

    #[test]
    fn all_filter_averages_all_runs_not_avg_of_mode_averages() {
        let mut s = Store::open_in_memory().unwrap();
        let t = 1_700_000_000_000; // same instant → one day bucket
        insert_run(&mut s, &base_run(t, "time", 60.0)).unwrap();
        insert_run(&mut s, &base_run(t, "time", 60.0)).unwrap();
        insert_run(&mut s, &base_run(t, "time", 60.0)).unwrap();
        insert_run(&mut s, &base_run(t, "words", 100.0)).unwrap();
        let app = StatsApp::load(&s, &Theme::fallback()).unwrap();

        let all = app
            .series
            .get(&(GraphPeriod::Day, GraphFilter::All))
            .unwrap();
        assert_eq!(all, &vec![70.0], "(60+60+60+100)/4 = 70, NOT (60+100)/2");
        assert_eq!(
            app.series
                .get(&(GraphPeriod::Day, GraphFilter::Time))
                .unwrap(),
            &vec![60.0]
        );
        assert_eq!(
            app.series
                .get(&(GraphPeriod::Day, GraphFilter::Words))
                .unwrap(),
            &vec![100.0]
        );
    }

    #[test]
    fn session_breaks_started_at_ties_deterministically() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &base_run(500, "time", 70.0)).unwrap(); // id 1
        insert_run(&mut s, &base_run(500, "time", 80.0)).unwrap(); // id 2, same started_at
        let app = StatsApp::load(&s, &Theme::fallback()).unwrap();
        let session_all = app
            .series
            .get(&(GraphPeriod::Session, GraphFilter::All))
            .unwrap();
        assert_eq!(
            session_all,
            &vec![70.0, 80.0],
            "tied started_at falls back to insertion (id) order, not HashMap order"
        );
    }

    #[test]
    fn o_cycles_period_and_resets_scroll_to_latest() {
        let s = seeded(WINDOW as i64 + 5); // 45 session points → session scroll can be > 0
        let mut app = StatsApp::load(&s, &Theme::fallback()).unwrap();
        assert_eq!(app.graph_period, GraphPeriod::Session);
        assert_eq!(app.graph_scroll, 5, "anchored to latest: 45 - 40");

        app.on_key(key('o'));
        assert_eq!(app.graph_period, GraphPeriod::Day);
        // All 45 runs share one calendar day → one point → window fits → scroll resets to 0.
        assert_eq!(app.graph_scroll, 0);
        assert_eq!(app.max_scroll(), 0);
    }

    #[test]
    fn shift_o_cycles_filter_with_or_without_shift_and_ignores_ctrl() {
        let s = seeded(3);
        let mut app = StatsApp::load(&s, &Theme::fallback()).unwrap();
        assert_eq!(app.graph_filter, GraphFilter::All);

        app.on_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::NONE));
        assert_eq!(app.graph_filter, GraphFilter::Time);
        app.on_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT));
        assert_eq!(app.graph_filter, GraphFilter::Words);
        app.on_key(KeyEvent::new(KeyCode::Char('O'), KeyModifiers::CONTROL));
        assert_eq!(
            app.graph_filter,
            GraphFilter::Words,
            "Ctrl+O is not a filter cycle"
        );
    }

    #[test]
    fn left_right_scroll_is_clamped() {
        let s = seeded(WINDOW as i64 + 5); // session max_scroll = 5
        let mut app = StatsApp::load(&s, &Theme::fallback()).unwrap();
        assert_eq!(app.graph_scroll, 5);

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE)); // already at max
        assert_eq!(app.graph_scroll, 5);
        for _ in 0..10 {
            app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        }
        assert_eq!(app.graph_scroll, 0, "clamped at the oldest edge");
        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.graph_scroll, 1);
    }

    #[test]
    fn visible_series_windows_and_reindexes() {
        let s = seeded(WINDOW as i64 + 5); // series = [50, 51, ..., 94], scroll = 5
        let app = StatsApp::load(&s, &Theme::fallback()).unwrap();
        let vis = app.visible_series();
        assert_eq!(vis.len(), WINDOW);
        assert_eq!(vis[0], (0.0, 55.0)); // s[5]
        assert_eq!(vis[WINDOW - 1], (39.0, 94.0)); // s[44]
    }

    #[test]
    fn r_requests_practice_with_slow_letters() {
        let mut s = Store::open_in_memory().unwrap();
        insert_run(&mut s, &slow_letter_run(100, "q", 6000, 20)).unwrap(); // avg 300ms, 20 samples
        let mut app = StatsApp::load(&s, &Theme::fallback()).unwrap();
        assert!(app.can_practice());
        assert_eq!(app.slow_letters, vec!['q']);

        app.on_key(key('r'));
        assert!(app.should_quit);
        assert_eq!(app.outcome, StatsOutcome::Practice(vec!['q']));
    }

    #[test]
    fn q_quits_and_practice_needs_slow_letters() {
        let s = Store::open_in_memory().unwrap();
        let mut app = StatsApp::load(&s, &Theme::fallback()).unwrap();
        assert!(!app.can_practice()); // empty DB
        app.on_key(key('r')); // ignored — nothing to practice
        assert!(!app.should_quit);
        app.on_key(key('q'));
        assert!(app.should_quit);
        assert_eq!(app.outcome, StatsOutcome::Quit);
    }
}
