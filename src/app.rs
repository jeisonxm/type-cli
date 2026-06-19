//! Application state machine: owns the current `TypingSession`, the clock baseline, and the
//! transitions between typing and results. The clock lives here (the impure shell), not in the
//! engine — `App` reads `Instant::now()` and passes a plain `Duration` down to the pure engine.

use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crossterm::event::KeyEvent;

use crate::config::AppConfig;
use crate::engine::{Action, Mode, TypingSession};
use crate::input;
use crate::sources::{self, wordlist, SourceKind};
use crate::stats::keystats::{char_latencies, char_tallies, worst_words};
use crate::stats::Summary;
use crate::storage::{insert_run, CharStatRow, RunRecord, Store, WorstWordRow};

/// Which screen the app is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppState {
    Typing,
    Results,
}

/// The whole running game.
pub struct App {
    pub config: AppConfig,
    pub mode: Mode,
    pub source: SourceKind,
    /// Normalized full text for document sources (used to re-window on restart).
    doc_text: Option<String>,
    pub session: TypingSession,
    pub state: AppState,
    /// Frozen results, computed once when the test finishes.
    pub summary: Option<Summary>,
    /// Whether the discreet timer is currently visible (toggled with Ctrl+T).
    pub show_timer: bool,
    /// Clock baseline for the current session.
    session_start: Instant,
    pub should_quit: bool,
    /// Open database handle, or `None` when persistence is unavailable (game still plays).
    store: Option<Store>,
}

impl App {
    pub fn new(
        config: AppConfig,
        mode: Mode,
        source: SourceKind,
        doc_text: Option<String>,
        show_timer: bool,
    ) -> Self {
        let session = Self::make_session(&source, doc_text.as_deref(), mode);
        // Best-effort: a missing/locked DB disables persistence but never blocks play.
        let store = match Store::open(&config.database_path()) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("type-cli: persistence disabled ({e})");
                None
            }
        };
        App {
            config,
            mode,
            source,
            doc_text,
            session,
            state: AppState::Typing,
            summary: None,
            show_timer,
            session_start: Instant::now(),
            should_quit: false,
            store,
        }
    }

    /// Build a fresh session's target text from the active source.
    fn make_session(source: &SourceKind, doc_text: Option<&str>, mode: Mode) -> TypingSession {
        let mut rng = rand::rng();
        let need = sources::needed_words(mode);
        let target: Vec<char> = match source {
            SourceKind::Random(lang) => wordlist::random_passage(lang, need, &mut rng)
                .chars()
                .collect(),
            SourceKind::Pdf(_) | SourceKind::Docx(_) => {
                sources::build_target(doc_text.unwrap_or_default(), mode, true, &mut rng)
            }
            // A practice drill is words rich in the player's slowest letters.
            SourceKind::SlowLetters { letters, language } => {
                wordlist::practice_passage(language, letters, need, &mut rng)
                    .chars()
                    .collect()
            }
        };
        TypingSession::new(target, mode)
    }

    /// Typing time elapsed since this session started (passed to the pure engine).
    pub fn elapsed(&self) -> Duration {
        self.session_start.elapsed()
    }

    /// Advance the clock (lets timed tests end without a keypress).
    pub fn on_tick(&mut self) {
        if self.state == AppState::Typing {
            let e = self.elapsed();
            self.session.tick(e);
            self.maybe_finish();
        }
    }

    /// Handle a key press.
    pub fn on_key(&mut self, key: KeyEvent) {
        let action = match input::map_key(key) {
            None => return,
            Some(input::Command::ToggleTimer) => {
                self.show_timer = !self.show_timer;
                return;
            }
            Some(input::Command::Engine(action)) => action,
        };
        match self.state {
            AppState::Typing => match action {
                Action::Quit => self.should_quit = true,
                Action::Restart => self.restart(),
                other => {
                    let e = self.elapsed();
                    self.session.apply(other, e);
                    self.maybe_finish();
                }
            },
            AppState::Results => match action {
                Action::Quit => self.should_quit = true,
                Action::Restart => self.restart(),
                Action::Type('q') => self.should_quit = true,
                _ => {}
            },
        }
    }

    /// Start a brand-new test (new random passage, or re-windowed document).
    pub fn restart(&mut self) {
        self.session = Self::make_session(&self.source, self.doc_text.as_deref(), self.mode);
        self.session_start = Instant::now();
        self.state = AppState::Typing;
        self.summary = None;
    }

    fn maybe_finish(&mut self) {
        if self.state == AppState::Typing && self.session.is_finished() {
            let summary = Summary::compute(&self.session, self.elapsed());
            self.summary = Some(summary);
            self.state = AppState::Results;
            self.persist_run(&summary);
        }
    }

    /// Write the just-finished run to the database (best-effort; a write error is logged, not fatal).
    fn persist_run(&mut self, summary: &Summary) {
        let Some(store) = self.store.as_mut() else {
            return;
        };

        let (mode, target) = match self.mode {
            Mode::Time { secs } => ("time", secs as i64),
            Mode::Words { count } => ("words", count as i64),
        };
        let (source, source_ref, language) = match &self.source {
            SourceKind::Random(lang) => ("random", None, Some(lang.clone())),
            SourceKind::Pdf(p) => ("pdf", Some(p.display().to_string()), None),
            SourceKind::Docx(p) => ("docx", Some(p.display().to_string()), None),
            // Practice drills are tagged `retry` (a non-analytics source) so they never pollute
            // stats/graphs; the slowest-letter aggregation also excludes them.
            SourceKind::SlowLetters { language, .. } => ("retry", None, Some(language.clone())),
        };

        // Per-letter latency, keyed by expected char, to enrich the tally rows below.
        let latencies: HashMap<char, (i64, i64)> = char_latencies(self.session.history())
            .into_iter()
            .map(|l| (l.expected, (l.total_ms as i64, l.samples as i64)))
            .collect();
        let char_stats = char_tallies(self.session.history())
            .into_iter()
            .map(|t| {
                let (total_latency_ms, latency_samples) =
                    latencies.get(&t.expected).copied().unwrap_or((0, 0));
                CharStatRow {
                    expected_char: t.expected.to_string(),
                    typed_total: t.typed_total as i64,
                    error_count: t.error_count as i64,
                    total_latency_ms,
                    latency_samples,
                }
            })
            .collect();
        let worst = worst_words(&self.session)
            .into_iter()
            .take(10)
            .map(|w| WorstWordRow {
                word: w.word,
                error_count: w.error_count as i64,
                word_wpm: w.word_wpm,
                rank: w.rank as i64,
            })
            .collect();

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let elapsed_ms = summary.elapsed.as_millis() as i64;

        let rec = RunRecord {
            mode,
            target,
            source,
            source_ref,
            language,
            wpm: summary.wpm,
            raw_wpm: summary.raw_wpm,
            accuracy: summary.accuracy,
            consistency: Some(summary.consistency),
            chars_correct: summary.correct_chars as i64,
            chars_incorrect: summary.incorrect_chars as i64,
            chars_extra: summary.extra_chars as i64,
            chars_missed: summary.missed_chars as i64,
            elapsed_ms,
            started_at: now_ms - elapsed_ms,
            created_at: now_ms,
            char_stats,
            worst_words: worst,
        };

        if let Err(e) = insert_run(store, &rec) {
            eprintln!("type-cli: could not save run ({e})");
        }
    }
}
