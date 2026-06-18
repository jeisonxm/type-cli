//! Application state machine: owns the current `TypingSession`, the clock baseline, and the
//! transitions between typing and results. The clock lives here (the impure shell), not in the
//! engine — `App` reads `Instant::now()` and passes a plain `Duration` down to the pure engine.

use std::time::{Duration, Instant};

use crossterm::event::KeyEvent;

use crate::config::AppConfig;
use crate::engine::{Action, Mode, TypingSession};
use crate::input;
use crate::sources::{self, wordlist, SourceKind};
use crate::stats::Summary;

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
            self.summary = Some(Summary::compute(&self.session, self.elapsed()));
            self.state = AppState::Results;
        }
    }
}
