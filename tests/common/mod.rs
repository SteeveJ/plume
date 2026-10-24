//! Faux micro, faux clavier et faux Whisper partagés par les tests d'intégration, plus des signaux audio reproductibles.
#![allow(dead_code)]
use anyhow::Result;
use async_trait::async_trait;
use plume::application::dictation::DictationService;
use plume::domain::model::{AudioSamples, HotkeyEvent, Transcription};
use plume::domain::ports::{audio::AudioRecorder, injector::TextInjector, notifier::Notifier, stt::SpeechToText};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};

/// Micro simulé : `fail` = aucun micro ; sinon chaque `stop()` rend l'enregistrement suivant de la file.
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

/// Whisper simulé qui répond toujours `says`, en comptant les appels.
pub struct FakeStt { pub says: &'static str, pub calls: Arc<AtomicUsize> }
impl FakeStt {
    pub fn new(says: &'static str) -> (Self, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        (Self { says, calls: calls.clone() }, calls)
    }
}
#[async_trait]
impl SpeechToText for FakeStt {
    async fn transcribe(&self, _: &AudioSamples) -> Result<Transcription> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(Transcription { text: self.says.into() })
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

/// Appuie puis relâche la touche `presses` fois et attend la fin du traitement.
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

/// Micro coupé ou débranché : zéros.
pub fn muted() -> Vec<f32> { vec![0.0; 16_000 * SECONDS] }
/// Bruit de fond blanc au volume RMS demandé.
pub fn white_noise(target_rms: f32) -> Vec<f32> {
    let mut seed = 7;
    scaled((0..16_000 * SECONDS).map(|_| lcg(&mut seed)).collect(), target_rms)
}
/// Bourdonnement secteur 50 Hz + harmonique.
pub fn hum(target_rms: f32) -> Vec<f32> {
    let tau = std::f32::consts::TAU;
    scaled((0..16_000 * SECONDS).map(|i| (i as f32 * 50.0 * tau / 16_000.0).sin() + 0.3 * (i as f32 * 150.0 * tau / 16_000.0).sin()).collect(), target_rms)
}
/// Claquements de touches espacés.
pub fn key_clicks(target_rms: f32) -> Vec<f32> {
    let (mut seed, mut v) = (11, vec![0.0f32; 16_000 * SECONDS]);
    for c in 0..8 { for i in 0..200 { v[4_000 + c * 7_000 + i] = lcg(&mut seed) * (1.0 - i as f32 / 200.0); } }
    scaled(v, target_rms)
}
/// Un son de volume « voix » (RMS ≈ 0,21), assez fort pour passer tous les garde-fous.
pub fn voice() -> Vec<f32> {
    (0..16_000 * SECONDS).map(|i| (i as f32 * 400.0 * std::f32::consts::TAU / 16_000.0).sin() * 0.3).collect()
}

// ---- comparaison de textes (tests de compréhension en français) ----

fn strip_accent(c: char) -> char {
    match c {
        'à' | 'â' | 'ä' => 'a', 'é' | 'è' | 'ê' | 'ë' => 'e', 'î' | 'ï' => 'i', 'ô' | 'ö' => 'o',
        'ù' | 'û' | 'ü' => 'u', 'ç' => 'c', 'œ' => 'o', _ => c,
    }
}

/// Mots en minuscules sans ponctuation ; « 10 % » = « 10% » ; apostrophes et traits d'union séparent les mots.
pub fn words(text: &str) -> Vec<String> {
    let lower = text.to_lowercase().replace(" %", "%");
    let spaced: String = lower.chars().map(|c| if c.is_alphanumeric() || c == '%' { c } else { ' ' }).collect();
    spaced.split_whitespace().map(String::from).collect()
}

/// Même chose sans accents : sert à vérifier que le LLM ne change que la forme (accents, majuscules, ponctuation), jamais les mots.
pub fn plain_words(text: &str) -> Vec<String> {
    words(text).into_iter().map(|w| w.chars().map(strip_accent).collect()).collect()
}

/// Taux d'erreur de mots (distance de Levenshtein sur les mots / longueur de la référence).
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

/// Texte de référence de chaque phrase dictée dans `tests/fixtures/fr/` : tel qu'un utilisateur veut le lire
/// (chiffres, pas « quinze »), puisque Whisper écrit « 15 mars à 14h30 ».
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
