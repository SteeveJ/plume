//! Whisper hallucination cases: no microphone, muted microphone, background noise — and the case where one really speaks.
//! Whisper `turbo` invents "Thank you.", subtitle credits or a lone "." on noise
//! (measured in tests/e2e_models.rs); none of that must reach the keyboard, but the same phrase said aloud must.
mod common;
use common::*;
use plume::domain::ports::stt::SpeechToText;
use plume::application::dictation::DictationService;

struct Case {
    name: &'static str,
    mic: FakeMic,
    whisper_says: &'static str,
    typed: &'static [&'static str],
    whisper_called: bool,
    notification: &'static str,
}

fn run_case(c: Case) {
    let (stt, calls) = FakeStt::new(c.whisper_says);
    let (typed, notes) = (Typed::default(), Notes::default());
    let mut svc = DictationService::new(c.mic, stt, typed.clone(), None, Box::new(notes.clone()));
    dictate(&mut svc, 1);
    let called = calls.load(std::sync::atomic::Ordering::SeqCst) > 0;
    assert_eq!(typed.texts(), c.typed, "[{}] texte tapé", c.name);
    assert_eq!(called, c.whisper_called, "[{}] Whisper appelé", c.name);
    assert!(notes.all().contains(c.notification), "[{}] notification attendue {:?}, reçu {:?}", c.name, c.notification, notes.all());
}

#[test]
fn no_microphone_does_not_crash_and_nothing_is_typed() {
    run_case(Case { name: "pas de micro", mic: FakeMic::missing(), whisper_says: "Thank you.",
        typed: &[], whisper_called: false, notification: "Microphone unavailable" });
}

#[test]
fn muted_microphone_never_reaches_whisper() {
    // Muted microphone: zeros. The volume guard avoids calling Whisper, which would answer "Thank you.".
    run_case(Case { name: "micro coupé", mic: FakeMic::one(muted()), whisper_says: "Thank you.",
        typed: &[], whisper_called: false, notification: "No sound" });
}

#[test]
fn hallucinations_on_background_noise_are_dropped() {
    // Noise above the silence threshold: Whisper is called and invents something; the result is dropped.
    let cases = [
        ("souffle + Thank you", white_noise(0.01), "Thank you."),
        ("bourdonnement + crédits FR", hum(0.01), "Sous-titrage Société Radio-Canada"),
        ("claquements + Thank you", key_clicks(0.02), "Thank you."),
        ("bruit + point seul", white_noise(0.01), "."),
        ("bruit + points de suspension", white_noise(0.02), "..."),
        ("bruit + Merci", white_noise(0.01), "Merci."),
    ];
    for (name, audio, says) in cases {
        run_case(Case { name, mic: FakeMic::one(audio), whisper_says: says, typed: &[], whisper_called: true, notification: "Nothing heard" });
    }
}

#[test]
fn the_same_phrases_are_kept_when_the_user_really_speaks() {
    for (name, says) in [("Thank you à voix haute", "Thank you."), ("Merci à voix haute", "Merci."), ("phrase normale", "Ouvre le rapport de jeudi.")] {
        run_case(Case { name, mic: FakeMic::one(voice()), whisper_says: says,
            typed: Box::leak(vec![says].into_boxed_slice()), whisper_called: true, notification: "Done" });
    }
}

#[test]
fn a_quiet_merci_is_typed_when_the_voice_detector_confirmed_speech() {
    // Same weak audio, same text as "noise + Merci" (dropped): only the voice detector's confirmation changes everything.
    let (stt, _) = FakeStt::confirmed("Merci.");
    let (typed, notes) = (Typed::default(), Notes::default());
    let mut svc = DictationService::new(FakeMic::one(white_noise(0.01)), stt, typed.clone(), None, Box::new(notes.clone()));
    dictate(&mut svc, 1);
    assert_eq!(typed.texts(), vec!["Merci.".to_string()], "{}", notes.all());
    // Punctuation alone stays dropped even when confirmed.
    let (stt, _) = FakeStt::confirmed(".");
    let (typed, _) = (Typed::default(), ());
    let mut svc = DictationService::new(FakeMic::one(white_noise(0.01)), stt, typed.clone(), None, Box::new(Notes::default()));
    dictate(&mut svc, 1);
    assert!(typed.texts().is_empty());
}

#[test]
fn quiet_but_real_speech_is_not_mistaken_for_noise() {
    // Quiet voice (RMS 0.01) but a real sentence: only the known invented phrases are filtered.
    run_case(Case { name: "voix basse", mic: FakeMic::one(white_noise(0.01)), whisper_says: "Ouvre le rapport de jeudi",
        typed: &["Ouvre le rapport de jeudi"], whisper_called: true, notification: "Done" });
}

#[test]
fn noise_then_speech_in_the_same_session_types_only_the_speech() {
    struct Scripted(std::sync::Mutex<Vec<&'static str>>, std::sync::Arc<std::sync::atomic::AtomicUsize>);
    #[async_trait::async_trait]
    impl SpeechToText for Scripted {
        async fn transcribe(&self, _: &plume::domain::model::AudioSamples) -> anyhow::Result<plume::domain::model::Transcription> {
            self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(plume::domain::model::Transcription { text: self.0.lock().unwrap().remove(0).into(), ..Default::default() })
        }
    }
    let mic = FakeMic { fail: false, takes: [white_noise(0.01), muted(), voice()].into() };
    let stt = Scripted(std::sync::Mutex::new(vec!["Thank you.", "Bonjour tout le monde."]), Default::default());
    let (typed, notes) = (Typed::default(), Notes::default());
    let mut svc = DictationService::new(mic, stt, typed.clone(), None, Box::new(notes.clone()));
    dictate(&mut svc, 3);
    assert_eq!(typed.texts(), vec!["Bonjour tout le monde.".to_string()], "{}", notes.all());
}
