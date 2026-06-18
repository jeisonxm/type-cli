//! Random-word challenges from embedded wordlists (monkeytype's lists, bundled at compile time so
//! the game works as a single binary with no files present).

use rand::Rng;

const ENGLISH: &str = include_str!("../../assets/wordlists/english.txt");
const SPANISH: &str = include_str!("../../assets/wordlists/spanish.txt");

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
}
