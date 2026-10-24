use std::sync::{Arc, Mutex};
use crate::config::data_dir;

/// Live prompt shared between STT adapters and DictationService.
pub type SharedVocab = Arc<Mutex<String>>;

pub fn new_shared_vocab(initial: impl Into<String>) -> SharedVocab {
    Arc::new(Mutex::new(initial.into()))
}

/// Extract proper nouns and acronyms from a transcription and return new words only.
pub fn extract_words(text: &str, existing: &str) -> Vec<String> {
    let mut new_words: Vec<String> = Vec::new();
    let mut prev_is_boundary = true; // true at sentence start

    for word in text.split_whitespace() {
        // Strip leading/trailing punctuation
        let clean: String = word.chars()
            .filter(|c| c.is_alphabetic() || *c == '-' || *c == '\'')
            .collect();
        if clean.len() < 3 {
            if word.ends_with(['.', '!', '?']) { prev_is_boundary = true; }
            else if clean.len() > 1 { prev_is_boundary = false; } // single letters (I, a) don't reset boundary
            continue;
        }

        let is_mid_sentence_cap = !prev_is_boundary
            && clean.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        let is_acronym = clean.len() >= 2 && clean.chars().all(|c| c.is_uppercase());

        if (is_mid_sentence_cap || is_acronym)
            && !existing.contains(clean.as_str())
            && !new_words.contains(&clean)
        {
            new_words.push(clean);
        }

        prev_is_boundary = word.ends_with(['.', '!', '?']);
    }
    new_words
}

/// Append newly learned words to vocab.txt and return the updated full prompt.
pub fn persist_words(words: &[String]) -> Option<String> {
    if words.is_empty() { return None; }
    let path = data_dir().join("vocab.txt");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let new_line = words.join(", ");
    let header = if existing.contains("# auto-learned") { "" } else { "\n# auto-learned" };
    let updated = format!("{}{header}\n{new_line}\n", existing.trim_end());
    std::fs::write(&path, &updated).ok()?;
    // Return the flat list of all non-comment words for the prompt
    Some(
        updated.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_letter_word_does_not_reset_boundary() {
        // "I" is a single letter → prev_is_boundary stays true → "Love" is NOT captured
        let words = extract_words("I Love Paris", "");
        assert!(!words.contains(&"Love".to_string()), "'Love' after 'I' must not be captured");
        assert!(words.contains(&"Paris".to_string()), "'Paris' is a mid-sentence proper noun");
    }

    #[test]
    fn sentence_start_capital_not_captured() {
        let words = extract_words("Hello world today", "");
        assert!(words.is_empty(), "first word of sentence must not be captured");
    }

    #[test]
    fn acronym_always_captured() {
        let words = extract_words("we use GPU acceleration", "");
        assert!(words.contains(&"GPU".to_string()));
    }

    #[test]
    fn existing_word_not_duplicated() {
        let words = extract_words("I love Paris", "Paris");
        assert!(!words.contains(&"Paris".to_string()), "already in vocab");
    }
}
