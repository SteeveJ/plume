//! Fake microphone, keyboard and Whisper shared by the integration tests, plus reproducible audio signals.
#![allow(dead_code)]
use anyhow::Result;
use async_trait::async_trait;
use plume::application::dictation::DictationService;
use plume::domain::model::{AudioSamples, HotkeyEvent, Transcription};
use plume::domain::ports::{audio::AudioRecorder, injector::TextInjector, notifier::Notifier, stt::SpeechToText};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};

/// Simulated microphone: `fail` = no microphone; otherwise each `stop()` returns the next recording from the queue.
pub struct FakeMic { pub fail: bool, pub takes: VecDeque<Vec<f32>> }
impl FakeMic {
    pub fn one(samples: Vec<f32>) -> Self { Self { fail: false, takes: VecDeque::from([samples]) } }
    pub fn missing() -> Self { Self { fail: true, takes: VecDeque::new() } }
}
impl AudioRecorder for FakeMic {
    fn start(&mut self) -> Result<()> {
        if self.fail { anyhow::bail!("no microphone found"); }
        Ok(())
    }
    fn stop(&mut self) -> Result<AudioSamples> {
        Ok(AudioSamples::new(self.takes.pop_front().unwrap_or_default(), 16_000, 1))
    }
}

/// Simulated Whisper that always answers `says`, counting calls.
pub struct FakeStt { pub says: &'static str, pub calls: Arc<AtomicUsize>, pub confirmed: bool }
impl FakeStt {
    pub fn new(says: &'static str) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (Self { says, calls: calls.clone(), confirmed: false }, calls)
    }
    /// Like Whisper with the voice detector: it confirmed speech in the audio.
    pub fn confirmed(says: &'static str) -> (Self, Arc<AtomicUsize>) {
        let (mut s, calls) = Self::new(says);
        s.confirmed = true;
        (s, calls)
    }
}
#[async_trait]
impl SpeechToText for FakeStt {
    async fn transcribe(&self, _: &AudioSamples) -> Result<Transcription> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Transcription { text: self.says.into(), speech_confirmed: self.confirmed })
    }
}

#[derive(Clone, Default)]
pub struct Typed(pub Arc<Mutex<Vec<String>>>);
impl Typed { pub fn texts(&self) -> Vec<String> { self.0.lock().unwrap().clone() } }
impl TextInjector for Typed {
    fn inject(&self, text: &str) -> Result<()> { self.0.lock().unwrap().push(text.into()); Ok(()) }
}

#[derive(Clone, Default)]
pub struct Notes(pub Arc<Mutex<Vec<String>>>);
impl Notes { pub fn all(&self) -> String { self.0.lock().unwrap().join(" | ") } }
impl Notifier for Notes { fn notify(&self, msg: &str) { self.0.lock().unwrap().push(msg.into()); } }

/// Presses then releases the key `presses` times and waits for processing to finish.
pub fn dictate<S: SpeechToText>(svc: &mut DictationService<FakeMic, S, Typed>, presses: usize) {
    let (tx, rx) = mpsc::channel();
    for _ in 0..presses {
        tx.send(HotkeyEvent::RecordStart).unwrap();
        tx.send(HotkeyEvent::RecordStop).unwrap();
    }
    drop(tx);
    tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(svc.run(rx)).unwrap();
}

pub fn rms(v: &[f32]) -> f32 { (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt() }

fn lcg(seed: &mut u64) -> f32 {
    *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*seed >> 33) as f32 / (1u64 << 31) as f32) * 2.0 - 1.0
}

fn scaled(sig: Vec<f32>, target_rms: f32) -> Vec<f32> {
    let k = target_rms / rms(&sig).max(1e-9);
    sig.into_iter().map(|x| x * k).collect()
}

const SECONDS: usize = 4;

/// Muted or unplugged microphone: zeros.
pub fn muted() -> Vec<f32> { vec![0.0; 16_000 * SECONDS] }
/// White background noise at the requested RMS volume.
pub fn white_noise(target_rms: f32) -> Vec<f32> {
    let mut seed = 7;
    scaled((0..16_000 * SECONDS).map(|_| lcg(&mut seed)).collect(), target_rms)
}
/// Bourdonnement secteur 50 Hz + harmonique.
pub fn hum(target_rms: f32) -> Vec<f32> {
    let tau = std::f32::consts::TAU;
    scaled((0..16_000 * SECONDS).map(|i| (i as f32 * 50.0 * tau / 16_000.0).sin() + 0.3 * (i as f32 * 150.0 * tau / 16_000.0).sin()).collect(), target_rms)
}
/// Spaced key clicks.
pub fn key_clicks(target_rms: f32) -> Vec<f32> {
    let (mut seed, mut v) = (11, vec![0.0f32; 16_000 * SECONDS]);
    for c in 0..8 { for i in 0..200 { v[4_000 + c * 7_000 + i] = lcg(&mut seed) * (1.0 - i as f32 / 200.0); } }
    scaled(v, target_rms)
}
/// A "voice"-level sound (RMS ≈ 0.21), loud enough to pass every guard.
pub fn voice() -> Vec<f32> {
    (0..16_000 * SECONDS).map(|i| (i as f32 * 400.0 * std::f32::consts::TAU / 16_000.0).sin() * 0.3).collect()
}

// ---- text comparison (French comprehension tests) ----

fn strip_accent(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' => 'a', 'é' | 'è' | 'ê' | 'ë' => 'e', 'î' | 'ï' => 'i', 'ô' | 'ö' => 'o',
        'ù' | 'û' | 'ü' => 'u', 'ç' => 'c', 'œ' => 'o', _ => c,
    }
}

/// Lowercase words without punctuation; "10 %" = "10%"; apostrophes and hyphens separate words.
pub fn words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase().replace(" %", "%");
    let spaced: String = lower.chars().map(|c| if c.is_alphanumeric() || c == '%' { c } else { ' ' }).collect();
    spaced.split_whitespace().map(String::from).collect()
}

/// Same without accents: used to check that the LLM only changes the form (accents, capitals, punctuation), never the words.
pub fn plain_words(text: &str) -> Vec<String> {
    words(text).into_iter().map(|w| w.chars().map(strip_accent).collect()).collect()
}

/// Word error rate (Levenshtein distance on words / reference length).
pub fn wer(reference: &str, hypothesis: &str) -> f32 {
    let (r, h) = (words(reference), words(hypothesis));
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, hw) in h.iter().enumerate() {
            cur.push((prev[j] + usize::from(rw != hw)).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[h.len()] as f32 / r.len().max(1) as f32
}

/// Reference text of each sentence dictated in `tests/fixtures/fr/`: as a user wants to read it
/// (digits, not "quinze"), since Whisper writes "15 mars à 14h30".
pub const FRENCH_SENTENCES: &[(&str, &str)] = &[
    ("mail", "Bonjour Marie, peux-tu m'envoyer le rapport avant jeudi soir, s'il te plaît ?"),
    ("accents", "J'ai été à la plage hier, et il a fallu que je ramène les enfants avant la nuit."),
    ("nombres", "Le rendez-vous est fixé au 15 mars à 14h30, en salle 3."),
    ("noms", "Nous allons déployer la nouvelle version sur Kubernetes avec Jean-Pierre et Sophie lundi matin."),
    ("question", "Est-ce que vous avez de la place pour deux personnes demain soir vers 20h ?"),
    ("paragraphe", "Je pense que nous devrions nous voir ensemble lundi matin pour parler du budget, parce que les ventes ont baissé de 10% depuis le mois dernier, et il faut absolument comprendre pourquoi."),
];

pub fn french_audio(name: &str) -> plume::domain::model::AudioSamples {
    let path = format!("{}/tests/fixtures/fr/{name}.wav", env!("CARGO_MANIFEST_DIR"));
    let mut r = hound::WavReader::open(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    assert_eq!((r.spec().sample_rate, r.spec().channels), (16_000, 1));
    plume::domain::model::AudioSamples::new(r.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect(), 16_000, 1)
}

// ---- mode instruction ----

/// Simulated desktop selection.
pub struct Selected(pub Option<String>);
impl plume::domain::ports::selection::SelectionReader for Selected {
    fn selected_text(&self) -> Result<Option<String>> { Ok(self.0.clone()) }
}

/// Simulated online model: applies the instruction "mets en majuscules" to the text after `Text:`, and counts its calls.
pub struct FakeRewriter(pub Arc<AtomicUsize>);
#[async_trait]
impl plume::domain::ports::processor::TextProcessor for FakeRewriter {
    async fn process(&self, raw: &str) -> Result<String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(raw.split_once("Text:\n").map_or(String::new(), |(_, t)| t.to_uppercase()))
    }
}

/// Sends exactly these key events, then waits for processing to finish.
pub fn press<S: SpeechToText>(svc: &mut DictationService<FakeMic, S, Typed>, events: Vec<HotkeyEvent>) {
    let (tx, rx) = mpsc::channel();
    for e in events { tx.send(e).unwrap(); }
    drop(tx);
    tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap().block_on(svc.run(rx)).unwrap();
}
