//! Per-key error tallies → "most-failed key", and worst-word extraction.
//!
//! Pure aggregation over a session's keystroke history / typed buffer. The values produced here are
//! what Phase 2 persists (`char_stat`, `worst_word`) and what the stats screen visualizes. Wired
//! into the UI in Phase 2; implemented + tested now because it is cheap and pure.

use std::collections::HashMap;

use crate::engine::session::Keystroke;

/// Attempts and errors for one expected character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharTally {
    pub expected: char,
    pub typed_total: u32,
    pub error_count: u32,
}

/// Aggregate keystrokes by the character that was expected, sorted worst-first
/// (most errors, then alphabetical for stable ordering).
pub fn char_tallies(history: &[Keystroke]) -> Vec<CharTally> {
    let mut map: HashMap<char, (u32, u32)> = HashMap::new();
    for k in history.iter().filter(|k| !k.is_backspace) {
        if let Some(expected) = k.expected {
            let entry = map.entry(expected).or_insert((0, 0));
            entry.0 += 1;
            if !k.correct {
                entry.1 += 1;
            }
        }
    }
    let mut tallies: Vec<CharTally> = map
        .into_iter()
        .map(|(expected, (typed_total, error_count))| CharTally {
            expected,
            typed_total,
            error_count,
        })
        .collect();
    tallies.sort_by(|a, b| {
        b.error_count
            .cmp(&a.error_count)
            .then(a.expected.cmp(&b.expected))
    });
    tallies
}

/// The single most-failed key (by absolute error count), if any errors were made.
pub fn most_failed_key(history: &[Keystroke]) -> Option<char> {
    char_tallies(history)
        .into_iter()
        .find(|t| t.error_count > 0)
        .map(|t| t.expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Action, Mode, TypingSession};
    use std::time::Duration;

    fn typed_history(target: &str, typed: &str) -> Vec<Keystroke> {
        let mut s = TypingSession::from_str(target, Mode::Words { count: 1 });
        for c in typed.chars() {
            s.apply(Action::Type(c), Duration::ZERO);
        }
        s.history().to_vec()
    }

    #[test]
    fn counts_errors_per_expected_char() {
        // target "aaa", typed "axa": position 1 expected 'a' typed 'x' = 1 error on 'a'.
        let h = typed_history("aaa", "axa");
        let tallies = char_tallies(&h);
        assert_eq!(tallies.len(), 1);
        assert_eq!(tallies[0].expected, 'a');
        assert_eq!(tallies[0].typed_total, 3);
        assert_eq!(tallies[0].error_count, 1);
    }

    #[test]
    fn most_failed_key_picks_the_worst() {
        // target "abab", typed "xbxb": both 'a' positions wrong, 'b' positions right.
        let h = typed_history("abab", "xbxb");
        assert_eq!(most_failed_key(&h), Some('a'));
    }

    #[test]
    fn no_errors_means_no_most_failed_key() {
        let h = typed_history("abc", "abc");
        assert_eq!(most_failed_key(&h), None);
    }
}
