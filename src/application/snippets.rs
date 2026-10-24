use crate::config::data_dir;

pub struct Snippet {
    trigger: String,
    expansion: String,
}

/// Format : une règle par ligne `phrase => texte` ; `#` commente ; `\n` dans le texte = saut de ligne.
pub fn parse(src: &str) -> Vec<Snippet> {
    let mut v: Vec<Snippet> = src.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| {
            let (t, e) = l.split_once("=>")?;
            let trigger = t.trim().to_lowercase();
            (!trigger.is_empty()).then(|| Snippet { trigger, expansion: e.trim().replace("\\n", "\n") })
        })
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.trigger.len()));
    v
}

pub fn load() -> Vec<Snippet> {
    std::fs::read_to_string(data_dir().join("snippets.txt")).map(|s| parse(&s)).unwrap_or_default()
}

// shortcut: règles appliquées l'une après l'autre (une expansion peut en redéclencher une autre) ; to_lowercase qui change la longueur => texte inchangé.
pub fn expand(text: &str, snippets: &[Snippet]) -> String {
    let mut out = text.to_string();
    for s in snippets {
        let lower = out.to_lowercase();
        if lower.len() != out.len() { break; }
        let (mut res, mut last, mut from) = (String::with_capacity(out.len()), 0, 0);
        while let Some(i) = lower[from..].find(&s.trigger) {
            let (start, end) = (from + i, from + i + s.trigger.len());
            let word_start = lower[..start].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
            let word_end = lower[end..].chars().next().is_none_or(|c| !c.is_alphanumeric());
            if word_start && word_end {
                res.push_str(&out[last..start]);
                res.push_str(&s.expansion);
                last = end;
            }
            from = end;
        }
        res.push_str(&out[last..]);
        out = res;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_case_insensitive_whole_phrase_keeping_punctuation() {
        let s = parse("# commentaire\nmon email => jean@example.com\n");
        assert_eq!(expand("Envoie à Mon Email.", &s), "Envoie à jean@example.com.");
    }

    #[test]
    fn does_not_expand_inside_a_word() {
        let s = parse("sig => Cordialement");
        assert_eq!(expand("signature et sig", &s), "signature et Cordialement");
    }

    #[test]
    fn longest_trigger_first_and_newline_escape() {
        let s = parse("mon adresse => A\nmon adresse pro => B\\nC");
        assert_eq!(expand("mon adresse pro", &s), "B\nC");
    }

    #[test]
    fn ignores_malformed_and_empty_trigger_lines() {
        assert!(parse("pas de flèche\n => vide\n").is_empty());
    }
}
