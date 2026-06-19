//! Per-key error tallies → "most-failed key", and worst-word extraction.
//!
//! Pure aggregation over a session's keystroke history / typed buffer. The values produced here are
//! what Phase 2 persists (`char_stat`, `worst_word`) and what the stats screen visualizes. Wired
//! into the UI in Phase 2; implemented + tested now because it is cheap and pure.

use std::collections::HashMap;

use crate::engine::session::Keystroke;
use crate::engine::TypingSession;

/// Attempts and errors for one expected character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharTally {
    pub expected: char,
    pub typed_total: u32,
    pub error_count: u32,
}

/// A target word the player struggled with, ranked worst-first. Persisted as `worst_word` and used
/// to seed the "retry worst words" session in Phase 2.
#[derive(Debug, Clone, PartialEq)]
pub struct WorstWord {
    pub word: String,
    pub error_count: u32,
    /// WPM over the span of keystrokes inside the word; `None` when it can't be measured
    /// (fewer than two keystrokes, or zero elapsed between them).
    pub word_wpm: Option<f64>,
    /// 1-based position in the worst-first ordering.
    pub rank: u32,
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

/// Words the player mistyped, ranked worst-first (most errors, then alphabetical). Only words with
/// at least one error are returned. `word_wpm` is measured over the keystrokes that landed inside
/// the word's target span.
pub fn worst_words(session: &TypingSession) -> Vec<WorstWord> {
    let target = session.target();
    let history = session.history();

    // Split the target into word spans [start, end) of non-space runs.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < target.len() {
        if target[i] == ' ' {
            i += 1;
            continue;
        }
        let start = i;
        while i < target.len() && target[i] != ' ' {
            i += 1;
        }
        spans.push((start, i));
    }

    let mut out: Vec<WorstWord> = Vec::new();
    for (start, end) in spans {
        let strokes: Vec<&Keystroke> = history
            .iter()
            .filter(|k| !k.is_backspace && k.index >= start && k.index < end)
            .collect();
        let error_count = strokes.iter().filter(|k| !k.correct).count() as u32;
        if error_count == 0 {
            continue;
        }
        let word: String = target[start..end].iter().collect();
        let word_wpm = word_wpm_over(&strokes, word.chars().count());
        out.push(WorstWord {
            word,
            error_count,
            word_wpm,
            rank: 0,
        });
    }

    out.sort_by(|a, b| b.error_count.cmp(&a.error_count).then(a.word.cmp(&b.word)));
    for (idx, w) in out.iter_mut().enumerate() {
        w.rank = idx as u32 + 1;
    }
    out
}

/// WPM across a word's keystrokes: `(chars / 5) / minutes`, spanning first→last keystroke.
fn word_wpm_over(strokes: &[&Keystroke], char_len: usize) -> Option<f64> {
    if strokes.len() < 2 {
        return None;
    }
    let t0 = strokes.iter().map(|k| k.t_offset).min()?;
    let t1 = strokes.iter().map(|k| k.t_offset).max()?;
    let minutes = (t1 - t0).as_secs_f64() / 60.0;
    if minutes <= 0.0 {
        return None;
    }
    Some((char_len as f64 / 5.0) / minutes)
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

    fn typed_session(target: &str, typed: &str) -> TypingSession {
        let mut s = TypingSession::from_str(target, Mode::Words { count: 99 });
        for (i, c) in typed.chars().enumerate() {
            s.apply(Action::Type(c), Duration::from_secs(i as u64));
        }
        s
    }

    #[test]
    fn worst_words_ranks_only_mistyped_words() {
        // "cat dog sun": miss in "cat" (t→d) and "dog" (o→x); "sun" is clean.
        let s = typed_session("cat dog sun", "cad dxg sun");
        let w = worst_words(&s);
        let words: Vec<&str> = w.iter().map(|x| x.word.as_str()).collect();
        assert_eq!(words, vec!["cat", "dog"]); // "sun" excluded (no errors)
        assert_eq!(w[0].rank, 1);
        assert_eq!(w[1].rank, 2);
        assert!(w.iter().all(|x| x.error_count == 1));
    }

    #[test]
    fn worst_words_sorts_by_error_count_first() {
        // "aaa bb": "aaa" gets 2 errors, "bb" gets 1 → "aaa" ranks first despite alpha order.
        let s = typed_session("aaa bb", "axx bx");
        let w = worst_words(&s);
        assert_eq!(w[0].word, "aaa");
        assert_eq!(w[0].error_count, 2);
        assert_eq!(w[1].word, "bb");
        assert_eq!(w[1].error_count, 1);
    }

    #[test]
    fn worst_words_measures_word_wpm() {
        // 1 char/sec: "ab" spans t=0..1 (1s). word_wpm = (2/5)/(1/60) = 24.
        let s = typed_session("ab cd", "xb cd");
        let w = worst_words(&s);
        assert_eq!(w.len(), 1);
        let wpm = w[0].word_wpm.expect("two strokes → measurable");
        assert!((wpm - 24.0).abs() < 1e-6, "got {wpm}");
    }
}
