//! Does a correction keep the user's words? Used to reject a rewrite of a dictation that changed them (ADR 0005).

/// Words of `text` in plain form: lowercase, accents removed, split on anything that is not a letter or a digit.
/// Punctuation, capitals, accents, apostrophes and hyphens therefore never count as a change.
pub fn plain_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() { current.push(strip_accent(c)); }
        else if !current.is_empty() { words.push(std::mem::take(&mut current)); }
    }
    if !current.is_empty() { words.push(current); }
    words
}

fn strip_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'œ' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        _ => c,
    }
}

/// True when `output` is `input` with only its form fixed: the same words in the same order.
/// A small model sometimes invents a word while "correcting" (`fallu` becomes `fallé`); typing that would be worse than not correcting.
pub fn is_faithful(input: &str, output: &str) -> bool {
    plain_words(input) == plain_words(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn form_changes_are_faithful() {
        assert!(is_faithful("bonjour marie peux tu m envoyer le rapport", "Bonjour, Marie, peux-tu m'envoyer le rapport."));
        assert!(is_faithful("j ai ete a la plage", "J'ai été à la plage."));
        assert!(is_faithful("est ce que vous avez de la place", "Est-ce que vous avez de la place ?"));
        assert!(is_faithful("hello world", "Hello, world."));
    }

    #[test]
    fn a_changed_added_removed_or_reordered_word_is_not() {
        assert!(!is_faithful("il a fallu que je ramene", "Il a fallé que je ramène."), "an invented word");
        assert!(!is_faithful("avant jeudi soir", "Avant le soir."), "a lost word");
        assert!(!is_faithful("peux tu m envoyer", "Peux-tu me l'envoyer ?"), "an added word");
        assert!(!is_faithful("un deux trois", "Trois, deux, un."), "a different order");
    }

    #[test]
    fn plain_words_ignore_case_accents_and_punctuation() {
        assert_eq!(plain_words("L'été, À CÔTÉ du château !"), ["l", "ete", "a", "cote", "du", "chateau"]);
        assert!(plain_words("... !").is_empty());
    }
}
