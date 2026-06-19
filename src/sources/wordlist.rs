//! Random-word challenges from embedded wordlists (monkeytype's lists, bundled at compile time so
//! the game works as a single binary with no files present).

use std::collections::HashMap;

use rand::Rng;

const ENGLISH: &str = include_str!("../../assets/wordlists/english.txt");
const SPANISH: &str = include_str!("../../assets/wordlists/spanish.txt");

/// How many of the richest candidate words a practice drill samples from — enough variety without
/// diluting the focus on the slow letters.
const PRACTICE_POOL: usize = 80;

/// Case-fold one char (Unicode-aware, unlike `to_ascii_lowercase`) so an accented slow letter like
/// `É` matches the lowercase `é` in the wordlist.
fn lower_char(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// The word pool for a language name (falls back to English for unknown names).
pub fn words_for(language: &str) -> Vec<String> {
    let raw = match language.to_lowercase().as_str() {
        "spanish" | "español" | "espanol" | "es" => SPANISH,
        _ => ENGLISH,
    };
    raw.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// A space-separated passage of `need` random words from `language`'s wordlist.
pub fn random_passage(language: &str, need: usize, rng: &mut impl Rng) -> String {
    let words = words_for(language);
    super::select_words(&words, need, false, rng).join(" ")
}

/// A practice passage of `need` words drawn from those richest in `letters` (weighted toward the
/// first/slowest letter). Words are scored by the weighted count of their target letters; the
/// richest `PRACTICE_POOL` form the drill pool, sampled with replacement. Falls back to a random
/// passage when `letters` is empty or nothing in the wordlist matches.
pub fn practice_passage(
    language: &str,
    letters: &[char],
    need: usize,
    rng: &mut impl Rng,
) -> String {
    // Weight each target letter by its position (slowest first → highest weight).
    let mut weight: HashMap<char, usize> = HashMap::new();
    for (i, c) in letters.iter().enumerate() {
        let w = letters.len() - i;
        let entry = weight.entry(lower_char(*c)).or_insert(0);
        *entry = (*entry).max(w);
    }
    if weight.is_empty() {
        return random_passage(language, need, rng);
    }

    let pool = words_for(language);
    let score = |word: &str| -> usize {
        word.chars()
            .map(|c| weight.get(&lower_char(c)).copied().unwrap_or(0))
            .sum()
    };
    let mut scored: Vec<(usize, &String)> = pool
        .iter()
        .map(|w| (score(w), w))
        .filter(|(s, _)| *s > 0)
        .collect();
    if scored.is_empty() {
        return random_passage(language, need, rng);
    }
    // Richest first; break ties alphabetically for deterministic pools.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));
    let rich: Vec<String> = scored
        .into_iter()
        .take(PRACTICE_POOL)
        .map(|(_, w)| w.clone())
        .collect();
    super::select_words(&rich, need, false, rng).join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn embedded_wordlists_are_non_empty() {
        assert!(words_for("english").len() > 100);
        assert!(words_for("spanish").len() > 100);
    }

    #[test]
    fn unknown_language_falls_back_to_english() {
        assert_eq!(words_for("klingon"), words_for("english"));
    }

    #[test]
    fn random_passage_has_requested_word_count() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let passage = random_passage("english", 25, &mut rng);
        assert_eq!(passage.split(' ').count(), 25);
        // Every produced word is from the pool.
        let pool = words_for("english");
        assert!(passage.split(' ').all(|w| pool.iter().any(|p| p == w)));
    }

    #[test]
    fn practice_passage_prefers_words_with_target_letters() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(11);
        let passage = practice_passage("english", &['k', 'w'], 20, &mut rng);
        assert_eq!(passage.split(' ').count(), 20);
        // Every drilled word contains at least one target letter.
        assert!(
            passage
                .split(' ')
                .all(|w| w.contains('k') || w.contains('w')),
            "got: {passage}"
        );
    }

    #[test]
    fn practice_passage_falls_back_when_no_letters() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let passage = practice_passage("english", &[], 15, &mut rng);
        assert_eq!(passage.split(' ').count(), 15);
        let pool = words_for("english");
        assert!(passage.split(' ').all(|w| pool.iter().any(|p| p == w)));
    }

    #[test]
    fn practice_passage_has_requested_word_count() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(5);
        assert_eq!(
            practice_passage("english", &['e', 't'], 25, &mut rng)
                .split(' ')
                .count(),
            25
        );
    }

    #[test]
    fn practice_passage_matches_accented_letters_case_insensitively() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(9);
        // An uppercase accented slow letter (e.g. from a Spanish doc's sentence-initial capital)
        // must still match the lowercase accented words in the pool.
        let passage = practice_passage("spanish", &['É'], 8, &mut rng);
        assert_eq!(passage.split(' ').count(), 8);
        assert!(
            passage.split(' ').all(|w| w.contains('é')),
            "uppercase É must match lowercase é words, got: {passage}"
        );
    }
}
