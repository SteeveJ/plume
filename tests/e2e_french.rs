//! Understanding of French with real models (ignored by default):
//!   PLUME_TEST_GGML=/chemin/ggml-turbo.bin PLUME_TEST_GGUF=/chemin/model.gguf \
//!     cargo test --release --test e2e_french -- --ignored        (+ --features cuda)
//! The sentences come from tests/fixtures/fr/ (French synthetic voice, 16 kHz); see docs/en/development.md.
mod common;
use common::*;
use plume::adapters::{processor::LocalProcessor, stt::{new_shared_vocab, WhisperCpp}};
use plume::domain::ports::{processor::TextProcessor, stt::SpeechToText};
use std::path::PathBuf;

/// llama.cpp initializes only once per process: tests that load the LLM run one after the other.
static ONE_AT_A_TIME: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn env_path(var: &str) -> PathBuf {
    PathBuf::from(std::env::var(var).unwrap_or_else(|_| panic!("définir {var}")))
}

/// Keywords that carry each sentence's meaning (alternatives separated by `|`): "15" or "quinze",
/// because what matters is that the message is understood, not how a number is written.
const MEANING: &[(&str, &[&str])] = &[
    ("mail", &["marie", "rapport", "jeudi", "soir"]),
    ("accents", &["plage|page", "hier", "ramène", "enfants", "nuit"]),
    ("nombres", &["mars", "15|quinze", "14|quatorze", "3|trois"]),
    ("noms", &["déployer", "kubernetes", "jean-pierre|jean pierre", "sophie", "lundi"]),
    ("question", &["place", "deux personnes", "demain", "20|vingt"]),
    ("paragraphe", &["budget", "ventes", "baissé", "10|dix", "comprendre"]),
];

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn whisper_understands_french_speech_forced_and_auto_detected() {
    let _serial = ONE_AT_A_TIME.lock().await;
    for lang in ["fr", ""] {
        let whisper = WhisperCpp::load(&env_path("PLUME_TEST_GGML"), lang, new_shared_vocab("")).unwrap();
        let (mut errors, mut total) = (0.0f32, 0.0f32);
        for (name, reference) in FRENCH_SENTENCES {
            let said = whisper.transcribe(&french_audio(name)).await.unwrap().text;
            let lower = said.to_lowercase();
            // 1. The meaning is there, whatever the number format.
            let anchors = MEANING.iter().find(|(n, _)| n == name).unwrap().1;
            for group in anchors {
                assert!(group.split('|').any(|alt| lower.contains(alt)), "langue {lang:?}, {name} : {group:?} absent de {said:?}");
            }
            // 2. The text stays in French (no translation to English with automatic detection).
            assert!(!lower.contains(" the ") && !lower.contains(" and "), "langue {lang:?}, {name} traduit ? {said:?}");
            // 3. No sentence is badly missed.
            let e = wer(reference, &said);
            assert!(e <= 0.30, "langue {lang:?}, {name} : {:.0} % d'erreurs\n  attendu {reference:?}\n  obtenu  {said:?}", e * 100.0);
            errors += e * words(reference).len() as f32;
            total += words(reference).len() as f32;
        }
        let overall = errors / total;
        assert!(overall <= 0.08, "langue {lang:?} : {:.1} % d'erreurs de mots sur l'ensemble (limite 8 %)", overall * 100.0);
    }
}

/// The limit that existed before the voice detector: a very quiet "Merci." looks like the "Merci." Whisper hallucinates
/// on noise. With the detector (`PLUME_TEST_VAD`) speech is confirmed and the word is kept; noise is not.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn a_very_quiet_merci_is_kept_thanks_to_the_voice_detector() {
    use plume::application::hallucination::is_hallucination;
    let Ok(vad) = std::env::var("PLUME_TEST_VAD") else { eprintln!("PLUME_TEST_VAD non défini : test ignoré"); return };
    let whisper = WhisperCpp::load(&env_path("PLUME_TEST_GGML"), "fr", new_shared_vocab("")).unwrap().with_vad(std::path::Path::new(&vad));
    let merci = french_audio("merci");
    for level in [0.01f32, 0.02] {
        let k = level / rms(&merci.samples);
        let quiet = plume::domain::model::AudioSamples::new(merci.samples.iter().map(|x| x * k).collect(), 16_000, 1);
        let heard = whisper.transcribe(&quiet).await.unwrap();
        assert!(heard.speech_confirmed, "volume {level} : parole non détectée");
        assert!(heard.text.to_lowercase().contains("merci"), "volume {level} : {:?}", heard.text);
        assert!(!is_hallucination(&heard.text, level, heard.speech_confirmed), "volume {level} : {:?} serait écarté", heard.text);
    }
    let noise = whisper.transcribe(&plume::domain::model::AudioSamples::new(white_noise(0.01), 16_000, 1)).await.unwrap();
    assert!(noise.text.is_empty() && !noise.speech_confirmed, "le bruit ne doit rien produire : {:?}", noise.text);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn llm_fixes_the_form_of_french_dictations_without_changing_the_words() {
    let _serial = ONE_AT_A_TIME.lock().await;
    let llm = LocalProcessor::load(&env_path("PLUME_TEST_GGUF"), plume::config::style_prompt("clean", "fr")).unwrap();
    let (mut well_formed, mut accents_restored) = (0, 0);
    for (name, reference) in FRENCH_SENTENCES {
        // Raw text as an approximate STT would render it: lowercase, no punctuation, no accents or apostrophes.
        let degraded = plain_words(reference).join(" ");
        let out = llm.process(&degraded).await.unwrap();

        // The LLM only changes the form: same words (accents aside), nothing lost, nothing added.
        assert_eq!(plain_words(&out), plain_words(&degraded), "{name} : le sens a changé\n  entrée {degraded:?}\n  sortie {out:?}");
        assert!(!out.contains("nothink") && !out.contains("<think>"), "{name} : {out:?}");

        let starts_upper = out.chars().next().is_some_and(char::is_uppercase);
        let ends_punct = out.trim_end().ends_with(['.', '?', '!']);
        well_formed += usize::from(starts_upper && ends_punct);
        accents_restored += ["été", "à", "baissé", "déployer", "dernier"].iter().filter(|a| out.contains(*a) && !degraded.contains(*a)).count();
    }
    // A 0.6-billion-parameter model corrects about 4 sentences out of 6: we require half, not perfection.
    assert!(well_formed >= 3, "seulement {well_formed}/6 phrases avec majuscule et ponctuation finale");
    assert!(accents_restored >= 2, "seulement {accents_restored} accents restaurés");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn french_speech_through_whisper_then_llm_keeps_its_meaning() {
    let _serial = ONE_AT_A_TIME.lock().await;
    let whisper = WhisperCpp::load(&env_path("PLUME_TEST_GGML"), "fr", new_shared_vocab("")).unwrap();
    let llm = LocalProcessor::load(&env_path("PLUME_TEST_GGUF"), plume::config::style_prompt("clean", "fr")).unwrap();
    let mut fell_back = Vec::new();
    for (name, reference) in FRENCH_SENTENCES {
        let heard = whisper.transcribe(&french_audio(name)).await.unwrap().text;
        let mut cleaned = llm.process(&heard).await.unwrap();
        // Like the dictation service: an empty output (the small model's thinking looping on a long text,
        // never answering) keeps Whisper's text. Nothing is lost, but the correction did not happen.
        if cleaned.trim().is_empty() {
            fell_back.push(*name);
            cleaned = heard.clone();
        }
        assert_eq!(plain_words(&cleaned), plain_words(&heard), "{name} : le LLM a modifié le sens\n  Whisper {heard:?}\n  LLM     {cleaned:?}");
        assert!(wer(reference, &cleaned) <= 0.30, "{name} : de bout en bout\n  attendu {reference:?}\n  obtenu  {cleaned:?}");
    }
    assert!(fell_back.len() <= 2, "le LLM n'a répondu que pour {}/6 phrases (vides : {fell_back:?})", 6 - fell_back.len());
}
