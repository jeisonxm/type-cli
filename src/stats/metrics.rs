//! Monkeytype-style metrics, as pure functions of counts + elapsed time.
//!
//! Definitions:
//! - `minutes = elapsed / 60s`
//! - **Net WPM**  = `(correct_chars / 5) / minutes`
//! - **Raw WPM**  = `(typed_chars / 5) / minutes`
//! - **Accuracy** = `100 * correct_keystrokes / total_keystrokes`
//! - **Consistency** = `100 * (1 - stddev/mean)` over the per-second raw-WPM series, clamped [0,100].

use std::time::Duration;

use crate::engine::session::Keystroke;
use crate::engine::TypingSession;

/// Standard typing "word" length used by WPM.
const CHARS_PER_WORD: f64 = 5.0;

fn minutes(elapsed: Duration) -> f64 {
    elapsed.as_secs_f64() / 60.0
}

/// Net words-per-minute: only correctly typed characters count.
pub fn net_wpm(correct_chars: usize, elapsed: Duration) -> f64 {
    let m = minutes(elapsed);
    if m <= 0.0 {
        return 0.0;
    }
    (correct_chars as f64 / CHARS_PER_WORD) / m
}

/// Raw words-per-minute: every typed character counts (errors included).
pub fn raw_wpm(typed_chars: usize, elapsed: Duration) -> f64 {
    let m = minutes(elapsed);
    if m <= 0.0 {
        return 0.0;
    }
    (typed_chars as f64 / CHARS_PER_WORD) / m
}

/// Accuracy as a percentage of keystrokes that matched the expected character.
pub fn accuracy(correct_keystrokes: usize, total_keystrokes: usize) -> f64 {
    if total_keystrokes == 0 {
        return 100.0;
    }
    100.0 * correct_keystrokes as f64 / total_keystrokes as f64
}

/// Coefficient-of-variation consistency over a per-second raw-WPM series, as a 0–100 score.
pub fn consistency(per_second_raw: &[f64]) -> f64 {
    let n = per_second_raw.len();
    if n < 2 {
        return 100.0;
    }
    let mean = per_second_raw.iter().sum::<f64>() / n as f64;
    if mean <= 0.0 {
        return 0.0;
    }
    let var = per_second_raw
        .iter()
        .map(|x| (x - mean).powi(2))
        .sum::<f64>()
        / n as f64;
    let stddev = var.sqrt();
    (100.0 * (1.0 - stddev / mean)).clamp(0.0, 100.0)
}

/// Build the per-second raw-WPM series from a keystroke history (for charts + consistency).
/// Each second bucket holds `(chars_typed_that_second / 5) * 60` WPM.
pub fn per_second_raw_wpm(history: &[Keystroke]) -> Vec<f64> {
    let typed: Vec<u64> = history
        .iter()
        .filter(|k| !k.is_backspace)
        .map(|k| k.t_offset.as_secs())
        .collect();
    let Some(&max_sec) = typed.iter().max() else {
        return Vec::new();
    };
    let mut buckets = vec![0usize; max_sec as usize + 1];
    for sec in typed {
        buckets[sec as usize] += 1;
    }
    buckets
        .iter()
        .map(|&c| (c as f64 / CHARS_PER_WORD) * 60.0)
        .collect()
}

/// A complete, immutable snapshot of a session's results.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Summary {
    pub wpm: f64,
    pub raw_wpm: f64,
    pub accuracy: f64,
    pub consistency: f64,
    pub correct_chars: usize,
    pub incorrect_chars: usize,
    /// Target positions skipped (cursor passed, left untyped) by a mid-word space.
    pub missed_chars: usize,
    /// Characters typed beyond the target's end. Always `0` today (the engine refuses input past the
    /// last position); kept for the `test_run.chars_extra` column and monkeytype parity.
    pub extra_chars: usize,
    pub typed_chars: usize,
    pub elapsed: Duration,
}

impl Summary {
    /// Compute the summary for `session` at time `now`.
    pub fn compute(session: &TypingSession, now: Duration) -> Self {
        let elapsed = session.elapsed(now);
        let correct_chars = session.correct_chars();
        let typed_chars = session.typed_keystrokes();
        Summary {
            wpm: net_wpm(correct_chars, elapsed),
            raw_wpm: raw_wpm(typed_chars, elapsed),
            accuracy: accuracy(session.correct_keystrokes(), session.typed_keystrokes()),
            consistency: consistency(&per_second_raw_wpm(session.history())),
            correct_chars,
            incorrect_chars: session.incorrect_chars(),
            missed_chars: session.missed_chars(),
            extra_chars: 0,
            typed_chars,
            elapsed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{Action, Mode, TypingSession};

    #[test]
    fn net_wpm_matches_definition() {
        // 25 correct chars in 60s = (25/5)/1 minute = 5 wpm.
        assert!((net_wpm(25, Duration::from_secs(60)) - 5.0).abs() < 1e-9);
        // 100 chars in 30s = (100/5)/0.5 = 40 wpm.
        assert!((net_wpm(100, Duration::from_secs(30)) - 40.0).abs() < 1e-9);
    }

    #[test]
    fn raw_wpm_counts_all_typed() {
        assert!((raw_wpm(50, Duration::from_secs(60)) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn zero_elapsed_is_zero_wpm_not_infinity() {
        assert_eq!(net_wpm(10, Duration::ZERO), 0.0);
        assert_eq!(raw_wpm(10, Duration::ZERO), 0.0);
    }

    #[test]
    fn accuracy_percentage() {
        assert!((accuracy(9, 10) - 90.0).abs() < 1e-9);
        assert_eq!(accuracy(0, 0), 100.0); // nothing typed yet
    }

    #[test]
    fn consistency_is_100_for_constant_series_and_lower_for_varied() {
        assert!((consistency(&[60.0, 60.0, 60.0]) - 100.0).abs() < 1e-9);
        assert!(consistency(&[20.0, 100.0, 60.0]) < 100.0);
        assert_eq!(consistency(&[60.0]), 100.0); // insufficient samples
    }

    #[test]
    fn summary_compute_end_to_end() {
        // Type "hello" correctly; pretend 1 char/sec so elapsed=4s after 5 keystrokes at 0..4.
        let mut s = TypingSession::from_str("hello", Mode::Words { count: 1 });
        for (i, c) in "hello".chars().enumerate() {
            s.apply(Action::Type(c), Duration::from_secs(i as u64));
        }
        let sum = Summary::compute(&s, Duration::from_secs(4));
        assert_eq!(sum.correct_chars, 5);
        assert_eq!(sum.typed_chars, 5);
        assert!((sum.accuracy - 100.0).abs() < 1e-9);
        // elapsed measured from first keystroke (t=0) to now=4s.
        assert_eq!(sum.elapsed, Duration::from_secs(4));
        assert_eq!(sum.missed_chars, 0);
        assert_eq!(sum.extra_chars, 0);
    }

    #[test]
    fn summary_counts_missed_chars_from_space_skip() {
        // "cat dog": type 'c' then a space → skips "at" (2 missed), accepts the space.
        let mut s = TypingSession::from_str("cat dog", Mode::Words { count: 2 });
        s.apply(Action::Type('c'), Duration::ZERO);
        s.apply(Action::Type(' '), Duration::ZERO);
        let sum = Summary::compute(&s, Duration::from_secs(1));
        assert_eq!(sum.missed_chars, 2); // 'a' and 't' left untyped behind the cursor
    }
}
