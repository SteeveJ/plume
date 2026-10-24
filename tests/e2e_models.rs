//! Non-régression avec de vrais modèles (ignorés par défaut, ils sont gros) :
//!   PLUME_TEST_GGML=/chemin/ggml-tiny.bin PLUME_TEST_GGUF=/chemin/model.gguf [PLUME_TEST_VAD=/chemin/ggml-silero-v5.1.2.bin] \
//!     cargo test --release --test e2e_models -- --ignored
//! Ajouter `--features cuda` pour tester le GPU. Ce test attrape ce que la compilation ne voit pas :
//! whisper.cpp et llama.cpp ont chacun leur ggml, et un conflit de symboles ne plante qu'à l'exécution.
mod common;
use plume::adapters::{processor::LocalProcessor, stt::{new_shared_vocab, WhisperCpp}};
use plume::domain::model::AudioSamples;
use plume::domain::ports::{processor::TextProcessor, stt::SpeechToText};
use std::path::PathBuf;

fn jfk() -> AudioSamples {
    let mut r = hound::WavReader::open(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/jfk.wav")).unwrap();
    assert_eq!((r.spec().sample_rate, r.spec().channels), (16_000, 1));
    let samples = r.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
    AudioSamples::new(samples, 16_000, 1)
}

fn env_path(var: &str) -> PathBuf {
    PathBuf::from(std::env::var(var).unwrap_or_else(|_| panic!("définir {var}")))
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn whisper_and_llama_work_together_in_one_process() {
    let whisper = WhisperCpp::load(&env_path("PLUME_TEST_GGML"), "en", new_shared_vocab("")).unwrap();
    let llm = LocalProcessor::load(&env_path("PLUME_TEST_GGUF"), plume::config::style_prompt("clean", "en")).unwrap();

    let heard = whisper.transcribe(&jfk()).await.unwrap().text;
    let lower = heard.to_lowercase();
    assert!(lower.contains("fellow americans") && lower.contains("your country"), "transcription inattendue : {heard:?}");

    let silence = AudioSamples::new(vec![0.0; 16_000 * 3], 16_000, 1);
    // Sur du silence pur, Whisper peut inventer une phrase ("Thank you.") : le service de dictée l'écarte en amont
    // (contrôle du volume, voir tests/pipeline.rs). L'adaptateur, lui, ne doit jamais laisser passer de balise.
    let on_silence = whisper.transcribe(&silence).await.unwrap().text;
    assert!(!on_silence.contains('[') && !on_silence.contains('('), "balise non filtrée : {on_silence:?}");

    // Le LLM tourne après Whisper dans le même processus et ne recopie aucun artefact du prompt.
    let cleaned = llm.process(&heard.to_lowercase().replace([',', '.'], "")).await.unwrap();
    assert!(!cleaned.is_empty(), "une sortie vide ferait perdre la dictée");
    assert!(!cleaned.contains("nothink") && !cleaned.contains("<think>"), "{cleaned:?}");
    assert!(cleaned.to_lowercase().contains("your country"), "{cleaned:?}");

    // Et Whisper continue de fonctionner après le LLM.
    assert_eq!(whisper.transcribe(&jfk()).await.unwrap().text, heard);
}

/// Reproduit ce que le service de dictée fait avec la vraie sortie de Whisper : volume trop faible = Whisper non appelé,
/// sinon `is_hallucination`. Si une nouvelle version de Whisper invente une phrase absente de la liste, ce test la montre.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn real_whisper_noise_is_filtered_and_real_speech_is_kept() {
    use plume::application::hallucination::is_hallucination;
    let noises: Vec<(String, Vec<f32>)> = [0.006f32, 0.01, 0.02].into_iter().flat_map(|level| [
        (format!("bruit blanc {level}"), common::white_noise(level)),
        (format!("bourdonnement 50 Hz {level}"), common::hum(level)),
        (format!("claquements {level}"), common::key_clicks(level)),
    ]).collect();
    let speech = jfk();
    let speech_rms = common::rms(&speech.samples);
    assert!(speech_rms > plume::application::hallucination::QUIET_RMS, "la voix de référence doit être au-dessus du seuil ({speech_rms})");

    for lang in ["fr", "en", ""] {
        let mut whisper = WhisperCpp::load(&env_path("PLUME_TEST_GGML"), lang, new_shared_vocab("")).unwrap();
        let with_vad = std::env::var("PLUME_TEST_VAD").ok();
        if let Some(v) = &with_vad { whisper = whisper.with_vad(std::path::Path::new(v)); }
        for (name, samples) in &noises {
            let rms = common::rms(samples);
            let heard = whisper.transcribe(&AudioSamples::new(samples.clone(), 16_000, 1)).await.unwrap();
            let kept = !heard.text.is_empty() && !is_hallucination(&heard.text, rms, heard.speech_confirmed);
            assert!(!kept, "langue {lang:?}, {name} : {:?} atteindrait le clavier — l'ajouter à INVENTED si c'est une vraie hallucination", heard.text);
            if with_vad.is_some() {
                assert!(heard.text.is_empty() && !heard.speech_confirmed, "langue {lang:?}, {name} : le détecteur de voix aurait dû écarter ce bruit, Whisper a dit {:?}", heard.text);
            }
        }
        // Avec la langue forcée sur le français, Whisper traduit le discours anglais : on cherche donc le mot dans la bonne langue.
        let heard = whisper.transcribe(&speech).await.unwrap();
        let expected = if lang == "fr" { "pays" } else { "your country" };
        assert!(!is_hallucination(&heard.text, speech_rms, heard.speech_confirmed) && heard.text.to_lowercase().contains(expected), "langue {lang:?}, parole : {:?}", heard.text);
        assert_eq!(heard.speech_confirmed, with_vad.is_some(), "langue {lang:?} : la parole est confirmée avec le détecteur de voix, et seulement avec lui");
    }
}
