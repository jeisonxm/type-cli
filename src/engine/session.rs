//! `TypingSession` — the live state of one typing test and the rules that advance it.
//!
//! Pure: it never reads the clock. Callers pass `elapsed` (monotonic time since the session was
//! created); the session records the moment of the first keystroke and measures everything from
//! there (monkeytype starts timing on the first keypress, not when the screen opens).

use std::time::Duration;

use crate::engine::action::Action;
use crate::engine::presets::Mode;

/// Per-character render state, derived (not stored) for the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharState {
    /// Not yet reached.
    Untyped,
    /// Typed and matches the target.
    Correct,
    /// Typed but wrong, or skipped (missed) by a mid-word space.
    Incorrect,
    /// The cursor sits here (the next character to type).
    Caret,
}

/// What the player actually typed at a target position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub typed: char,
    pub correct: bool,
}

/// One recorded keystroke. Feeds metrics, Phase 2 per-key stats, and Phase 3 ghost replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keystroke {
    /// Time since the first keystroke (so the first is always `0`).
    pub t_offset: Duration,
    /// Target index this keystroke acted on.
    pub index: usize,
    /// The character typed; `None` for a backspace.
    pub typed: Option<char>,
    /// The character expected at `index` when the keystroke happened.
    pub expected: Option<char>,
    /// Whether `typed == expected`.
    pub correct: bool,
    /// Whether this was a backspace (excluded from accuracy/WPM denominators).
    pub is_backspace: bool,
}

/// The live state of one test.
pub struct TypingSession {
    target: Vec<char>,
    typed: Vec<Option<Slot>>,
    cursor: usize,
    mode: Mode,
    /// `elapsed` value captured at the first keystroke; `None` until then.
    start_at: Option<Duration>,
    finished: bool,
    history: Vec<Keystroke>,
}

impl TypingSession {
    /// Create a session for `target` text in `mode`. `target` should already be normalized
    /// (NFC, single spaces) by the `sources` layer.
    pub fn new(target: Vec<char>, mode: Mode) -> Self {
        let len = target.len();
        Self {
            target,
            typed: vec![None; len],
            cursor: 0,
            mode,
            start_at: None,
            finished: false,
            history: Vec::new(),
        }
    }

    /// Convenience constructor from a `&str`.
    pub fn from_str(target: &str, mode: Mode) -> Self {
        Self::new(target.chars().collect(), mode)
    }

    // --- driving the session ------------------------------------------------

    /// Apply a player action at time `now` (elapsed since session creation).
    pub fn apply(&mut self, action: Action, now: Duration) {
        if self.finished {
            return;
        }
        match action {
            Action::Type(c) => self.type_char(c, now),
            Action::Backspace => self.backspace_one(now),
            Action::DeleteWord => self.delete_word(now),
            // Restart/Quit are lifecycle actions handled by the app layer.
            Action::Restart | Action::Quit => {}
        }
        self.check_finished(now);
    }

    /// Advance time without a keystroke (lets timed tests end on the clock).
    pub fn tick(&mut self, now: Duration) {
        if !self.finished {
            self.check_finished(now);
        }
    }

    fn type_char(&mut self, c: char, now: Duration) {
        if self.cursor >= self.target.len() {
            return;
        }
        if self.start_at.is_none() {
            self.start_at = Some(now);
        }
        let t = self.t_offset(now);
        let expected = self.target[self.cursor];

        // Space mid-word = skip to the next word, marking the remainder of the current word missed.
        if c == ' ' && expected != ' ' {
            let space_index = self.next_space_from(self.cursor);
            // Record the space press as completing the word boundary (lenient: not counted wrong).
            self.history.push(Keystroke {
                t_offset: t,
                index: space_index.unwrap_or(self.cursor),
                typed: Some(' '),
                expected: space_index.map(|i| self.target[i]),
                correct: space_index.is_some(),
                is_backspace: false,
            });
            match space_index {
                Some(i) => {
                    // Leave [cursor, i) as missed (None → rendered Incorrect), accept the space.
                    self.typed[i] = Some(Slot {
                        typed: ' ',
                        correct: true,
                    });
                    self.cursor = i + 1;
                }
                None => {
                    // No further space: skip to the end (test ends).
                    self.cursor = self.target.len();
                }
            }
            return;
        }

        let correct = c == expected;
        self.typed[self.cursor] = Some(Slot { typed: c, correct });
        self.history.push(Keystroke {
            t_offset: t,
            index: self.cursor,
            typed: Some(c),
            expected: Some(expected),
            correct,
            is_backspace: false,
        });
        self.cursor += 1;
    }

    fn backspace_one(&mut self, now: Duration) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let expected = self.target.get(self.cursor).copied();
        self.typed[self.cursor] = None;
        self.history.push(Keystroke {
            t_offset: self.t_offset(now),
            index: self.cursor,
            typed: None,
            expected,
            correct: false,
            is_backspace: true,
        });
    }

    fn delete_word(&mut self, now: Duration) {
        // Eat trailing spaces, then the word.
        while self.cursor > 0 && self.target[self.cursor - 1] == ' ' {
            self.backspace_one(now);
        }
        while self.cursor > 0 && self.target[self.cursor - 1] != ' ' {
            self.backspace_one(now);
        }
    }

    fn check_finished(&mut self, now: Duration) {
        if !self.target.is_empty() && self.cursor >= self.target.len() {
            self.finished = true;
            return;
        }
        if let Mode::Time { secs } = self.mode {
            if self.start_at.is_some() && self.elapsed(now) >= Duration::from_secs(secs) {
                self.finished = true;
            }
        }
    }

    fn next_space_from(&self, from: usize) -> Option<usize> {
        (from..self.target.len()).find(|&i| self.target[i] == ' ')
    }

    fn t_offset(&self, now: Duration) -> Duration {
        match self.start_at {
            Some(s) => now.saturating_sub(s),
            None => Duration::ZERO,
        }
    }

    // --- read-only accessors (used by ui/ and stats/) -----------------------

    /// Render state of target position `i`.
    pub fn char_state(&self, i: usize) -> CharState {
        if i == self.cursor && !self.finished {
            return CharState::Caret;
        }
        match self.typed.get(i).copied().flatten() {
            Some(slot) => {
                if slot.correct {
                    CharState::Correct
                } else {
                    CharState::Incorrect
                }
            }
            None if i < self.cursor => CharState::Incorrect, // skipped / missed
            None => CharState::Untyped,
        }
    }

    pub fn target(&self) -> &[char] {
        &self.target
    }
    pub fn typed(&self) -> &[Option<Slot>] {
        &self.typed
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn history(&self) -> &[Keystroke] {
        &self.history
    }
    pub fn is_finished(&self) -> bool {
        self.finished
    }
    pub fn is_started(&self) -> bool {
        self.start_at.is_some()
    }

    /// Typing time elapsed (measured from the first keystroke).
    pub fn elapsed(&self, now: Duration) -> Duration {
        match self.start_at {
            Some(s) => now.saturating_sub(s),
            None => Duration::ZERO,
        }
    }

    /// Target positions typed correctly (drives net WPM).
    pub fn correct_chars(&self) -> usize {
        self.typed
            .iter()
            .filter(|s| matches!(s, Some(slot) if slot.correct))
            .count()
    }

    /// Target positions typed incorrectly (excludes skipped/missed `None`s).
    pub fn incorrect_chars(&self) -> usize {
        self.typed
            .iter()
            .filter(|s| matches!(s, Some(slot) if !slot.correct))
            .count()
    }

    /// All non-backspace keystrokes (drives raw WPM and the accuracy denominator).
    pub fn typed_keystrokes(&self) -> usize {
        self.history.iter().filter(|k| !k.is_backspace).count()
    }

    /// Correct non-backspace keystrokes (accuracy numerator).
    pub fn correct_keystrokes(&self) -> usize {
        self.history
            .iter()
            .filter(|k| !k.is_backspace && k.correct)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: fn(u64) -> Duration = Duration::from_secs;

    fn words(n: usize) -> Mode {
        Mode::Words { count: n }
    }

    #[test]
    fn types_correct_characters() {
        let mut s = TypingSession::from_str("cat", words(1));
        s.apply(Action::Type('c'), S(0));
        s.apply(Action::Type('a'), S(0));
        assert_eq!(s.char_state(0), CharState::Correct);
        assert_eq!(s.char_state(1), CharState::Correct);
        assert_eq!(s.char_state(2), CharState::Caret);
        assert_eq!(s.correct_chars(), 2);
        assert!(!s.is_finished());
    }

    #[test]
    fn wrong_character_is_incorrect_but_advances() {
        let mut s = TypingSession::from_str("cat", words(1));
        s.apply(Action::Type('x'), S(0));
        assert_eq!(s.char_state(0), CharState::Incorrect);
        assert_eq!(s.cursor(), 1);
        assert_eq!(s.correct_chars(), 0);
        assert_eq!(s.incorrect_chars(), 1);
    }

    #[test]
    fn backspace_clears_the_previous_slot() {
        let mut s = TypingSession::from_str("cat", words(1));
        s.apply(Action::Type('c'), S(0));
        s.apply(Action::Type('x'), S(0));
        s.apply(Action::Backspace, S(0));
        assert_eq!(s.cursor(), 1);
        assert_eq!(s.char_state(1), CharState::Caret);
        assert_eq!(s.char_state(0), CharState::Correct);
    }

    #[test]
    fn backspace_at_start_is_a_noop() {
        let mut s = TypingSession::from_str("cat", words(1));
        s.apply(Action::Backspace, S(0));
        assert_eq!(s.cursor(), 0);
    }

    #[test]
    fn delete_word_removes_back_to_boundary() {
        let mut s = TypingSession::from_str("the cat", words(2));
        for c in "the ca".chars() {
            s.apply(Action::Type(c), S(0));
        }
        assert_eq!(s.cursor(), 6);
        s.apply(Action::DeleteWord, S(0));
        assert_eq!(s.cursor(), 4); // back to start of "ca" word
        assert_eq!(s.char_state(4), CharState::Caret);
    }

    #[test]
    fn words_mode_finishes_when_all_typed() {
        let mut s = TypingSession::from_str("hi", words(1));
        s.apply(Action::Type('h'), S(0));
        assert!(!s.is_finished());
        s.apply(Action::Type('i'), S(0));
        assert!(s.is_finished());
    }

    #[test]
    fn finished_session_ignores_further_input() {
        let mut s = TypingSession::from_str("hi", words(1));
        s.apply(Action::Type('h'), S(0));
        s.apply(Action::Type('i'), S(0));
        let cursor = s.cursor();
        s.apply(Action::Type('x'), S(0));
        assert_eq!(s.cursor(), cursor);
    }

    #[test]
    fn timed_mode_finishes_on_tick_after_limit() {
        let mut s =
            TypingSession::from_str("a long buffer of words to type", Mode::Time { secs: 30 });
        s.apply(Action::Type('a'), S(0)); // starts the clock at t=0
        s.tick(S(29));
        assert!(!s.is_finished());
        s.tick(S(30));
        assert!(s.is_finished());
    }

    #[test]
    fn timer_starts_on_first_keystroke_not_creation() {
        let mut s = TypingSession::from_str("abcdef ghij", Mode::Time { secs: 10 });
        // No keystroke yet: elapsed is zero regardless of `now`.
        s.tick(S(100));
        assert!(!s.is_finished());
        assert_eq!(s.elapsed(S(100)), Duration::ZERO);
        // First keystroke at now=100 anchors the clock there.
        s.apply(Action::Type('a'), S(100));
        assert_eq!(s.elapsed(S(105)), S(5));
    }

    #[test]
    fn space_midword_skips_remaining_letters() {
        let mut s = TypingSession::from_str("cat dog", words(2));
        s.apply(Action::Type('c'), S(0));
        s.apply(Action::Type(' '), S(0)); // skip "at"
        assert_eq!(s.char_state(1), CharState::Incorrect); // 'a' missed
        assert_eq!(s.char_state(2), CharState::Incorrect); // 't' missed
        assert_eq!(s.char_state(3), CharState::Correct); // the space
        assert_eq!(s.cursor(), 4); // at start of "dog"
        assert_eq!(s.char_state(4), CharState::Caret);
    }

    #[test]
    fn records_history_offsets_relative_to_first_keystroke() {
        let mut s = TypingSession::from_str("ab", words(1));
        s.apply(Action::Type('a'), S(5));
        s.apply(Action::Type('b'), S(7));
        let h = s.history();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].t_offset, Duration::ZERO);
        assert_eq!(h[1].t_offset, S(2));
    }
}
