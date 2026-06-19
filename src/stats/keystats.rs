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

/// Time spent producing one expected letter, summed across a session's keystrokes. The average
/// latency is `total_ms / samples`. Persisted into `char_stat` and aggregated across history to find
/// the slowest letters (which seed a practice drill).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharLatency {
    pub expected: char,
    /// Sum of clean inter-keystroke intervals attributed to this letter (ms).
    pub total_ms: u64,
    /// Number of intervals summed.
    pub samples: u32,
}

/// Inter-keystroke intervals longer than this (ms) are treated as think/AFK pauses and dropped, so
/// they don't inflate a letter's average latency.
const MAX_LATENCY_MS: u128 = 2000;

/// A target word the player struggled with, ranked worst-first. Still persisted per run as
/// `worst_word` (diagnostic data); the "retry worst words" drill that read it was superseded by the
/// slowest-letter practice drill, so no screen currently surfaces these rows.
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

/// Per-letter typing latency: the interval from the previous keystroke to this one, attributed to
/// the letter just typed. Only "clean digraph" intervals count — both keystrokes correct and
/// non-backspace, the current expected an alphabetic letter, and the previous expected a non-space
/// letter (so word-initial keystrokes, which bundle the inter-word pause, are excluded). Intervals
/// over `MAX_LATENCY_MS` (think/AFK pauses) are dropped. Sorted slowest-first (then alphabetical).
pub fn char_latencies(history: &[Keystroke]) -> Vec<CharLatency> {
    let mut map: HashMap<char, (u64, u32)> = HashMap::new();
    for i in 1..history.len() {
        let cur = &history[i];
        let prev = &history[i - 1];
        if cur.is_backspace || prev.is_backspace || !cur.correct || !prev.correct {
            continue;
        }
        let Some(expected) = cur.expected else {
            continue;
        };
        if !expected.is_alphabetic() {
            continue;
        }
        // The previous keystroke must be an alphabetic letter: this excludes word-initial keystrokes
        // (after a space), the synthetic space that ends a skipped word, and punctuation/digit
        // predecessors (in imported docs) whose interval shouldn't count as letter-to-letter typing.
        match prev.expected {
            Some(p) if p.is_alphabetic() => {}
            _ => continue,
        }
        let dt = cur.t_offset.saturating_sub(prev.t_offset).as_millis();
        if dt > MAX_LATENCY_MS {
            continue;
        }
        let entry = map.entry(expected).or_insert((0, 0));
        entry.0 += dt as u64;
        entry.1 += 1;
    }
    let mut out: Vec<CharLatency> = map
        .into_iter()
        .map(|(expected, (total_ms, samples))| CharLatency {
            expected,
            total_ms,
            samples,
        })
        .collect();
    out.sort_by(|a, b| {
        let avg = |c: &CharLatency| c.total_ms as f64 / c.samples.max(1) as f64;
        avg(b)
            .partial_cmp(&avg(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.expected.cmp(&b.expected))
    });
    out
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

    /// Type `chars` at the given absolute times (ms); `t_offset` ends up relative to the first.
    fn history_at(target: &str, chars: &[(char, u64)]) -> Vec<Keystroke> {
        let mut s = TypingSession::from_str(target, Mode::Words { count: 1 });
        for (c, ms) in chars {
            s.apply(Action::Type(*c), Duration::from_millis(*ms));
        }
        s.history().to_vec()
    }

    fn latency(latencies: &[CharLatency], c: char) -> Option<CharLatency> {
        latencies.iter().copied().find(|l| l.expected == c)
    }

    #[test]
    fn latency_attributes_interval_to_the_second_letter() {
        // "ab" typed at 0ms / 200ms: 'b' gets the 200ms interval; 'a' (first keystroke) gets none.
        let h = history_at("ab", &[('a', 0), ('b', 200)]);
        let l = char_latencies(&h);
        assert_eq!(
            latency(&l, 'b'),
            Some(CharLatency {
                expected: 'b',
                total_ms: 200,
                samples: 1
            })
        );
        assert_eq!(latency(&l, 'a'), None);
    }

    #[test]
    fn latency_skips_word_initial_letters() {
        // "ab cd": 'c' is word-initial (prev is the space) → excluded; 'b' and 'd' counted.
        let h = history_at(
            "ab cd",
            &[('a', 0), ('b', 100), (' ', 200), ('c', 300), ('d', 400)],
        );
        let l = char_latencies(&h);
        assert!(
            latency(&l, 'c').is_none(),
            "word-initial 'c' must be excluded"
        );
        assert_eq!(latency(&l, 'b').map(|x| x.samples), Some(1));
        assert_eq!(latency(&l, 'd').map(|x| x.samples), Some(1));
    }

    #[test]
    fn latency_ignores_incorrect_keystrokes() {
        // 'x' for 'b' is wrong (skips 'b'); 'c' has a wrong predecessor (skips 'c').
        let h = history_at("abc", &[('x', 0), ('b', 200), ('c', 400)]);
        let l = char_latencies(&h);
        assert!(
            latency(&l, 'b').is_none(),
            "prev keystroke wrong → 'b' not counted"
        );
        assert_eq!(latency(&l, 'c').map(|x| x.total_ms), Some(200));
    }

    #[test]
    fn latency_does_not_count_across_a_backspace() {
        // type 'a', backspace, retype 'a' at 200, 'b' at 400. The only clean digraph is a→b (400-200);
        // the backspace breaks adjacency so the interval is never summed across it.
        let mut s = TypingSession::from_str("ab", Mode::Words { count: 1 });
        s.apply(Action::Type('a'), Duration::from_millis(0));
        s.apply(Action::Backspace, Duration::from_millis(100));
        s.apply(Action::Type('a'), Duration::from_millis(200));
        s.apply(Action::Type('b'), Duration::from_millis(400));
        let l = char_latencies(s.history());
        assert_eq!(
            latency(&l, 'b'),
            Some(CharLatency {
                expected: 'b',
                total_ms: 200,
                samples: 1
            })
        );
        assert!(latency(&l, 'a').is_none());
    }

    #[test]
    fn latency_ignores_non_letter_predecessor() {
        // "a-b": the predecessor of 'b' is '-' (punctuation) — not a clean letter-to-letter digraph.
        let h = history_at("a-b", &[('a', 0), ('-', 100), ('b', 200)]);
        assert!(
            char_latencies(&h).is_empty(),
            "a punctuation/digit predecessor must not count toward the next letter"
        );
    }

    #[test]
    fn latency_caps_think_pauses() {
        // a 3s gap exceeds MAX_LATENCY_MS → dropped, no samples.
        let h = history_at("ab", &[('a', 0), ('b', 3000)]);
        assert!(char_latencies(&h).is_empty());
    }

    #[test]
    fn latency_sorts_slowest_first() {
        // "abcd": a→b fast (100), c→d slow (500). 'd' must sort before 'b'.
        let h = history_at(
            "ab cd",
            &[('a', 0), ('b', 100), (' ', 150), ('c', 200), ('d', 700)],
        );
        let l = char_latencies(&h);
        // 'c' excluded (word-initial); 'b' (100) and 'd' (500) present, 'd' first.
        assert_eq!(l.first().map(|x| x.expected), Some('d'));
        assert_eq!(latency(&l, 'd').map(|x| x.total_ms), Some(500));
        assert_eq!(latency(&l, 'b').map(|x| x.total_ms), Some(100));
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
