//! Challenge sources: turn a wordlist or an imported document into a normalized typing passage.
//!
//! The text pipeline is pure and unit-tested. Document extraction (PDF/DOCX) lives in the
//! submodules and feeds the same normalizer.

pub mod docx;
pub mod pdf;
pub mod wordlist;

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use rand::Rng;
use unicode_normalization::UnicodeNormalization;

use crate::engine::Mode;

/// Where a challenge's text came from (recorded with each run in Phase 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    /// Random words from an embedded wordlist (the named language).
    Random(String),
    /// A passage extracted from a PDF.
    Pdf(PathBuf),
    /// A passage extracted from a Word .docx.
    Docx(PathBuf),
    /// A practice drill of words rich in the player's slowest letters (slowest first).
    SlowLetters {
        letters: Vec<char>,
        language: String,
    },
}

/// Zero-width / invisible characters to drop during normalization.
const ZERO_WIDTH: &[char] = &['\u{200B}', '\u{200C}', '\u{200D}', '\u{FEFF}', '\u{00AD}'];

/// Normalize raw extracted text into a typing-friendly passage:
/// NFC → (optional) fold smart punctuation → strip control/zero-width → de-hyphenate line breaks →
/// collapse all whitespace to single spaces and trim.
pub fn normalize(raw: &str, fold_smart_punctuation: bool) -> String {
    // 1. NFC: compose accents so a typed "é" (one codepoint) matches the target.
    let nfc: String = raw.nfc().collect();

    // 2. + 3. fold smart punctuation (optional) and strip control / zero-width chars.
    let mut cleaned = String::with_capacity(nfc.len());
    for c in nfc.chars() {
        if ZERO_WIDTH.contains(&c) {
            continue;
        }
        if c.is_control() && c != '\n' && c != '\t' {
            continue;
        }
        if fold_smart_punctuation {
            match c {
                '\u{2018}' | '\u{2019}' | '\u{02BC}' => cleaned.push('\''),
                '\u{201C}' | '\u{201D}' => cleaned.push('"'),
                '\u{2013}' | '\u{2014}' => cleaned.push('-'),
                '\u{2026}' => cleaned.push_str("..."),
                '\u{00A0}' | '\u{2009}' | '\u{202F}' => cleaned.push(' '),
                _ => cleaned.push(c),
            }
        } else {
            cleaned.push(c);
        }
    }

    // 4. De-hyphenate words broken across lines: "exam-\nple" → "example".
    let chars: Vec<char> = cleaned.chars().collect();
    let mut dehyph = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '-'
            && i > 0
            && chars[i - 1].is_alphabetic()
            && i + 2 < chars.len()
            && chars[i + 1] == '\n'
            && chars[i + 2].is_alphabetic()
        {
            i += 2; // drop the hyphen and the newline
            continue;
        }
        dehyph.push(chars[i]);
        i += 1;
    }

    // 5. Collapse every run of whitespace to a single space and trim the ends.
    dehyph.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Fraction of non-whitespace characters that are letters. Low values flag garbled extraction
/// (tables, CID-font mojibake) or a scanned/image PDF with no real text layer.
pub fn alpha_ratio(text: &str) -> f64 {
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    let total = text.chars().filter(|c| !c.is_whitespace()).count();
    if total == 0 {
        0.0
    } else {
        letters as f64 / total as f64
    }
}

/// How many words a passage needs for a given mode (timed tests get a generous buffer).
pub fn needed_words(mode: Mode) -> usize {
    match mode {
        Mode::Words { count } => count,
        // ~4 words/sec ceiling for a very fast typist, floored so short timers still have headroom.
        Mode::Time { secs } => (secs as usize * 4).max(120),
    }
}

/// Select `need` words from `words`. `contiguous` picks a real passage (for documents); otherwise
/// words are chosen independently at random (for the wordlist). Cycles if there are too few words.
pub fn select_words(
    words: &[String],
    need: usize,
    contiguous: bool,
    rng: &mut impl Rng,
) -> Vec<String> {
    if words.is_empty() || need == 0 {
        return Vec::new();
    }
    if contiguous {
        if words.len() >= need {
            let start = rng.random_range(0..=(words.len() - need));
            words[start..start + need].to_vec()
        } else {
            (0..need).map(|i| words[i % words.len()].clone()).collect()
        }
    } else {
        (0..need)
            .map(|_| words[rng.random_range(0..words.len())].clone())
            .collect()
    }
}

/// Build the target character buffer for a session from already-normalized source text.
pub fn build_target(
    source_text: &str,
    mode: Mode,
    contiguous: bool,
    rng: &mut impl Rng,
) -> Vec<char> {
    let words: Vec<String> = source_text.split_whitespace().map(str::to_string).collect();
    select_words(&words, needed_words(mode), contiguous, rng)
        .join(" ")
        .chars()
        .collect()
}

/// Load and normalize a document, rejecting files with no usable text layer.
pub fn load_document(path: &Path, fold_smart_punctuation: bool) -> Result<String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase);
    let raw = match ext.as_deref() {
        Some("pdf") => pdf::extract(path)?,
        Some("docx") => docx::extract(path)?,
        _ => bail!(
            "unsupported file type (only .pdf and .docx are supported): {}",
            path.display()
        ),
    };
    let text = normalize(&raw, fold_smart_punctuation);
    if text.split_whitespace().count() < 5 || alpha_ratio(&text) < 0.6 {
        bail!(
            "no selectable text found in {} — a scanned/image PDF? (OCR is not supported)",
            path.display()
        );
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    fn rng() -> rand::rngs::StdRng {
        rand::rngs::StdRng::seed_from_u64(42)
    }

    #[test]
    fn collapses_whitespace_and_trims() {
        assert_eq!(normalize("  hello \n\t world  \n", false), "hello world");
    }

    #[test]
    fn folds_smart_punctuation_when_enabled() {
        let raw = "“curly” ‘quotes’ — dash… and\u{00A0}nbsp";
        assert_eq!(
            normalize(raw, true),
            "\"curly\" 'quotes' - dash... and nbsp"
        );
    }

    #[test]
    fn keeps_smart_punctuation_when_disabled() {
        let raw = "“curly”";
        assert_eq!(normalize(raw, false), "“curly”");
    }

    #[test]
    fn de_hyphenates_line_broken_words() {
        assert_eq!(normalize("exam-\nple text", false), "example text");
        // A real hyphenated compound (no newline) is preserved.
        assert_eq!(normalize("well-known fact", false), "well-known fact");
    }

    #[test]
    fn strips_control_and_zero_width_chars() {
        assert_eq!(normalize("a\u{200B}b\u{0007}c", false), "abc");
    }

    #[test]
    fn alpha_ratio_flags_garbled_text() {
        assert!(alpha_ratio("hello world") > 0.95);
        assert!(alpha_ratio("@#$%^&*()1234") < 0.6);
        assert_eq!(alpha_ratio("   "), 0.0);
    }

    #[test]
    fn needed_words_buffers_timed_tests() {
        assert_eq!(needed_words(Mode::Words { count: 100 }), 100);
        assert_eq!(needed_words(Mode::Time { secs: 60 }), 240);
        assert_eq!(needed_words(Mode::Time { secs: 5 }), 120); // floor
    }

    #[test]
    fn select_words_contiguous_picks_a_real_slice() {
        let words: Vec<String> = "alpha bravo charlie delta echo"
            .split(' ')
            .map(str::to_string)
            .collect();
        let picked = select_words(&words, 3, true, &mut rng());
        assert_eq!(picked.len(), 3);
        // It must be a contiguous slice of the original.
        let joined = words.join(" ");
        assert!(joined.contains(&picked.join(" ")));
    }

    #[test]
    fn select_words_cycles_when_too_few() {
        let words = vec!["a".to_string(), "b".to_string()];
        let picked = select_words(&words, 5, true, &mut rng());
        assert_eq!(picked, vec!["a", "b", "a", "b", "a"]);
    }

    #[test]
    fn build_target_word_count_matches_mode() {
        let text = "alpha bravo charlie delta echo foxtrot golf hotel india juliet";
        let target: String = build_target(text, Mode::Words { count: 4 }, true, &mut rng())
            .into_iter()
            .collect();
        assert_eq!(target.split(' ').count(), 4);
    }
}
