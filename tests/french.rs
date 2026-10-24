//! French end to end, without a model: accents, capitals, snippets, history, notifications in French.
//! Real understanding of spoken French (Whisper + LLM) is in tests/e2e_french.rs.
mod common;
use common::*;
use plume::adapters::{history::SqliteHistory, scratchpad::FileScratchpad};
use plume::application::{dictation::DictationService, snippets};
use plume::config::{OutputMode, UiLang};

#[test]
fn text_metrics_behave() {
    assert_eq!(wer("le chat dort", "le chat dort"), 0.0);
    assert!((wer("le chat dort ici", "le chien dort ici") - 0.25).abs() < 1e-6);
    assert_eq!(words("Il est à 10 % du but, c'est-à-dire proche."), ["il", "est", "à", "10%", "du", "but", "c", "est", "à", "dire", "proche"]);
    assert_eq!(plain_words("J'ai été à côté"), ["j", "ai", "ete", "a", "cote"]);
    // "J'étais" instead of "J'ai été" = 2 errors out of 17 words.
    let e = wer(FRENCH_SENTENCES[1].1, "J'étais à la plage hier, et il a fallu que je ramène les enfants avant la nuit.");
    assert!(e > 0.1 && e < 0.2, "{e}");
}

#[test]
fn accents_capitals_and_guillemets_survive_every_step() {
    let said = "Éric a dit : « À bientôt, l'été prochain ! » — Zoé, Œdipe et Noël.";
    let (db, pad) = (std::env::temp_dir().join(format!("plume-fr-{}.db", std::process::id())), std::env::temp_dir().join(format!("plume-fr-{}.txt", std::process::id())));
    let _ = (std::fs::remove_file(&db), std::fs::remove_file(&pad));
    let (stt, _) = FakeStt::new(said);
    let (typed, notes) = (Typed::default(), Notes::default());
    let mut svc = DictationService::new(FakeMic::one(voice()), stt, typed.clone(), None, Box::new(notes))
        .with_history(Box::new(SqliteHistory::open_at(&db).unwrap()))
        .with_scratchpad(Box::new(FileScratchpad::new(pad.clone())))
        .with_output(OutputMode::Both);
    dictate(&mut svc, 1);
    let stored: String = rusqlite::Connection::open(&db).unwrap().query_row("SELECT text FROM history", [], |r| r.get(0)).unwrap();
    let scratch = std::fs::read_to_string(&pad).unwrap();
    let _ = (std::fs::remove_file(&db), std::fs::remove_file(&pad));
    assert_eq!(typed.texts(), vec![said.to_string()]);
    assert_eq!(stored, said);
    assert!(scratch.contains(said));
}

#[test]
fn snippets_match_french_text_in_any_case_with_accents() {
    let rules = snippets::parse("mon adresse électronique => jean@example.com\nà bientôt => Cordialement,\\nJean\n");
    for (said, expected) in [
        ("MON ADRESSE ÉLECTRONIQUE est la suivante", "jean@example.com est la suivante"),
        ("Voici mon adresse électronique.", "Voici jean@example.com."),
        ("Merci et À BIENTÔT", "Merci et Cordialement,\nJean"),
        ("une adresse électroniques", "une adresse électroniques"),
    ] {
        assert_eq!(snippets::expand(said, &rules), expected);
    }
}

#[test]
fn french_notifications_for_the_user_who_chose_french() {
    for (name, mic, stt_says, expected) in [
        ("pas de micro", FakeMic::missing(), "Merci.", "Micro indisponible"),
        ("micro coupé", FakeMic::one(muted()), "Merci.", "Aucun son"),
        ("bruit + phrase inventée", FakeMic::one(white_noise(0.01)), "Sous-titrage Société Radio-Canada", "Rien entendu"),
    ] {
        let (stt, _) = FakeStt::new(stt_says);
        let (typed, notes) = (Typed::default(), Notes::default());
        let mut svc = DictationService::new(mic, stt, typed.clone(), None, Box::new(notes.clone())).with_ui_lang(UiLang::Fr);
        dictate(&mut svc, 1);
        assert!(typed.texts().is_empty(), "[{name}] rien ne doit être tapé");
        assert!(notes.all().contains(expected), "[{name}] attendu {expected:?}, reçu {:?}", notes.all());
    }
}

#[test]
fn a_long_french_dictation_is_corrected_in_chunks_without_losing_a_word() {
    use async_trait::async_trait;
    use plume::domain::ports::processor::TextProcessor;
    // Fake corrector that upper-cases: checks that the chunking glues the whole text back together.
    struct Upper;
    #[async_trait]
    impl TextProcessor for Upper {
        async fn process(&self, raw: &str) -> anyhow::Result<String> { Ok(raw.to_uppercase()) }
    }
    let sentence = "l'élève très pressé est allé à côté du château pour acheter du pain.";
    let said = [sentence; 12].join(" ");
    let (stt, _) = FakeStt::new(Box::leak(said.clone().into_boxed_str()));
    let (typed, notes) = (Typed::default(), Notes::default());
    let mut svc = DictationService::new(FakeMic::one(voice()), stt, typed.clone(), Some(Box::new(Upper)), Box::new(notes));
    dictate(&mut svc, 1);
    let out = typed.texts().remove(0);
    assert_eq!(plain_words(&out), plain_words(&said), "aucun mot perdu ni ajouté");
    assert_eq!(out, said.to_uppercase());
}
