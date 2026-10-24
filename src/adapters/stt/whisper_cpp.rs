use async_trait::async_trait;
use anyhow::{Context, Result};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState, WhisperVadContext,
    WhisperVadContextParams, WhisperVadParams,
};
use crate::domain::model::{AudioSamples, Transcription};
use crate::domain::ports::stt::SpeechToText;
use super::audio_util::to_mono_16k;
use super::shared_vocab::SharedVocab;

pub fn ggml_file(model: &str) -> String {
    let name = if model == "turbo" { "large-v3-turbo" } else { model };
    format!("ggml-{name}.bin")
}

pub fn ggml_url(file: &str) -> String {
    format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file}")
}

/// Détecteur de voix Silero (moins de 1 Mo) : distingue la parole du bruit avant Whisper (ADR 0014).
pub const VAD_FILE: &str = "ggml-silero-v5.1.2.bin";

pub fn vad_url() -> String {
    format!("https://huggingface.co/ggml-org/whisper-vad/resolve/main/{VAD_FILE}")
}

/// Marge gardée autour de la parole détectée : 0,25 s, pour ne pas couper le début ni la fin des mots.
const VAD_PAD: usize = 4_000;

/// Le modèle (+ tampons de calcul, ~0,6 à 0,9 Go pour `small`) est libéré après cette durée sans dictée.
const IDLE_UNLOAD: Duration = Duration::from_secs(300);

struct Loaded {
    state: WhisperState,
    vad: Option<WhisperVadContext>,
    last_used: Instant,
}

struct Inner {
    model_path: PathBuf,
    lang: String,
    prompt: SharedVocab,
    threads: i32,
    vad_path: Mutex<Option<PathBuf>>,
    loaded: Mutex<Option<Loaded>>,
}

impl Inner {
    fn ensure_loaded(&self, slot: &mut Option<Loaded>) -> Result<()> {
        if slot.is_some() { return Ok(()); }
        let path = self.model_path.to_str().context("chemin du modèle non UTF-8")?;
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .with_context(|| format!("load whisper model {}", self.model_path.display()))?;
        let state = ctx.create_state().context("whisper state")?;
        let vad = self.vad_path.lock().unwrap().as_deref().and_then(load_vad);
        *slot = Some(Loaded { state, vad, last_used: Instant::now() });
        Ok(())
    }
}

/// Sans détecteur de voix Plume reste utilisable : les filtres de volume et de phrases inventées prennent le relais.
fn load_vad(path: &Path) -> Option<WhisperVadContext> {
    let mut params = WhisperVadContextParams::new();
    params.set_use_gpu(false);
    WhisperVadContext::new(path.to_str()?, params)
        .map_err(|e| tracing::warn!("voice detector {} unavailable ({e}), continuing without it", path.display()))
        .ok()
}

/// Zone de parole (en centisecondes, telle que la rend le détecteur) élargie de `VAD_PAD` échantillons, bornée à l'audio.
fn padded_range(first_start_cs: f32, last_end_cs: f32, len: usize) -> Range<usize> {
    let to_samples = |cs: f32| (cs.max(0.0) * 160.0) as usize;
    to_samples(first_start_cs).saturating_sub(VAD_PAD).min(len)..(to_samples(last_end_cs) + VAD_PAD).min(len)
}

/// `None` = aucune parole dans l'audio.
fn detect_speech(vad: &mut WhisperVadContext, pcm: &[f32]) -> Result<Option<Range<usize>>> {
    let segments = vad.segments_from_samples(WhisperVadParams::new(), pcm).context("voice detection")?;
    let n = segments.num_segments();
    if n == 0 { return Ok(None); }
    let (first, last) = (segments.get_segment(0), segments.get_segment(n - 1));
    Ok(first.zip(last).map(|(f, l)| padded_range(f.start, l.end, pcm.len())))
}

/// Whisper en processus (whisper.cpp) : aucun Python ni serveur externe.
pub struct WhisperCpp {
    inner: Arc<Inner>,
}

impl WhisperCpp {
    /// Charge le modèle tout de suite (échec rapide : l'appelant peut basculer sur OpenAI), puis le décharge après `IDLE_UNLOAD` d'inactivité.
    pub fn load(model_path: &std::path::Path, lang: impl Into<String>, prompt: SharedVocab) -> Result<Self> {
        let inner = Arc::new(Inner {
            model_path: model_path.to_path_buf(),
            lang: lang.into(),
            prompt,
            threads: compute_threads(std::thread::available_parallelism().map_or(4, |n| n.get())),
            vad_path: Mutex::new(None),
            loaded: Mutex::new(None),
        });
        inner.ensure_loaded(&mut inner.loaded.lock().unwrap())?;

        let weak = Arc::downgrade(&inner);
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            let Some(inner) = weak.upgrade() else { break };
            let mut slot = inner.loaded.lock().unwrap();
            if slot.as_ref().is_some_and(|l| l.last_used.elapsed() >= IDLE_UNLOAD) {
                *slot = None;
                tracing::info!("whisper model unloaded (idle)");
            }
        });
        Ok(Self { inner })
    }

    /// Active le détecteur de voix : sans parole dans l'audio, Whisper n'est pas appelé (donc rien d'inventé) ;
    /// sinon seule la zone de parole lui est envoyée.
    pub fn with_vad(self, vad_model: &Path) -> Self {
        *self.inner.vad_path.lock().unwrap() = Some(vad_model.to_path_buf());
        if let Some(loaded) = self.inner.loaded.lock().unwrap().as_mut() {
            loaded.vad = load_vad(vad_model);
        }
        self
    }

    #[cfg(test)]
    fn unload(&self) {
        *self.inner.loaded.lock().unwrap() = None;
    }

    #[cfg(test)]
    fn is_loaded(&self) -> bool {
        self.inner.loaded.lock().unwrap().is_some()
    }
}

#[async_trait]
impl SpeechToText for WhisperCpp {
    fn warm(&self) {
        // Déjà chargé, ou occupé à transcrire : rien à faire (évite d'empiler des threads derrière le verrou).
        if self.inner.loaded.try_lock().map_or(true, |s| s.is_some()) { return; }
        let inner = self.inner.clone();
        std::thread::spawn(move || {
            let mut slot = inner.loaded.lock().unwrap();
            if let Err(e) = inner.ensure_loaded(&mut slot) { tracing::warn!("whisper warm-up failed: {e:#}"); }
            else if let Some(l) = slot.as_mut() { l.last_used = Instant::now(); }
        });
    }

    async fn transcribe(&self, audio: &AudioSamples) -> Result<Transcription> {
        let pcm = to_mono_16k(audio);
        let inner = self.inner.clone();
        let prompt = inner.prompt.lock().unwrap().clone();
        // Hors du thread async : la boucle de dictée reste libre de voir une nouvelle pression de touche pendant le calcul.
        tokio::task::spawn_blocking(move || inner.run(&pcm, &prompt)).await.context("whisper task")?
    }
}

impl Inner {
    fn run(&self, pcm: &[f32], prompt: &str) -> Result<Transcription> {
        let mut slot = self.loaded.lock().unwrap();
        self.ensure_loaded(&mut slot)?;
        let loaded = slot.as_mut().expect("loaded");

        let (pcm, speech_confirmed) = match loaded.vad.as_mut().map(|v| detect_speech(v, pcm)) {
            Some(Ok(Some(range))) => (&pcm[range], true),
            Some(Ok(None)) => {
                tracing::debug!("voice detector: no speech in the audio, skipping Whisper");
                loaded.last_used = Instant::now();
                return Ok(Transcription::default());
            }
            Some(Err(e)) => { tracing::warn!("voice detector failed, using the whole audio: {e:#}"); (pcm, false) }
            None => (pcm, false),
        };

        let mut p = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        // "auto" = détection de la langue ; sans lui whisper.cpp suppose l'anglais.
        p.set_language(Some(if self.lang.is_empty() { "auto" } else { &self.lang }));
        if !prompt.is_empty() { p.set_initial_prompt(prompt); }
        p.set_n_threads(self.threads);
        p.set_translate(false);
        p.set_no_context(true);
        p.set_print_progress(false);
        p.set_print_realtime(false);
        p.set_print_special(false);
        p.set_print_timestamps(false);
        let state = &mut loaded.state;
        state.full(p, pcm).context("whisper full")?;

        let (mut text, mut no_speech, mut n) = (String::new(), 0.0f32, 0u32);
        for seg in (0..state.full_n_segments()).filter_map(|i| state.get_segment(i)) {
            let t = seg.to_str_lossy().context("segment text")?;
            no_speech += seg.no_speech_probability();
            n += 1;
            if !is_non_speech_tag(&t) { text.push_str(&t); }
        }
        loaded.last_used = Instant::now();
        // Whisper marque le silence/bruit par un no_speech_prob élevé : on écarte ces hallucinations.
        if n > 0 && no_speech / n as f32 > 0.6 {
            tracing::debug!("whisper: no_speech_prob={:.2} — discarding as silence/noise", no_speech / n as f32);
            return Ok(Transcription::default());
        }
        Ok(Transcription { text: text.trim().to_string(), speech_confirmed })
    }
}

/// whisper.cpp écrit `[BLANK_AUDIO]`, `[MUSIC]`, `(silence)`… pour ce qui n'est pas de la parole : ces balises ne doivent jamais être tapées dans un document.
fn is_non_speech_tag(segment: &str) -> bool {
    let t = segment.trim();
    let wrapped = (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')'));
    t.is_empty() || wrapped || t.chars().all(|c| matches!(c, '♪' | '♫' | ' ' | '-'))
}

/// Threads de calcul : trois quarts des cœurs (2 au minimum, 8 au maximum) pour que le bureau reste fluide pendant une longue dictée.
pub fn compute_threads(cores: usize) -> i32 {
    (cores * 3 / 4).clamp(2, 8) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_names_map_to_ggml_files() {
        assert_eq!(ggml_file("small"), "ggml-small.bin");
        assert_eq!(ggml_file("small-q8_0"), "ggml-small-q8_0.bin");
        assert_eq!(ggml_file("turbo"), "ggml-large-v3-turbo.bin");
        assert_eq!(ggml_file("large-v2"), "ggml-large-v2.bin");
        assert!(ggml_url("ggml-small.bin").ends_with("/ggml-small.bin"));
    }

    #[test]
    fn non_speech_tags_are_dropped_but_speech_is_kept() {
        for tag in ["[BLANK_AUDIO]", " [ Music ] ", "(silence)", "♪♪", "  ", "[inaudible]"] {
            assert!(is_non_speech_tag(tag), "{tag:?}");
        }
        for speech in ["Hello world.", " Bonjour (enfin) tout le monde", "Le prix est [35] euros, ok"] {
            assert!(!is_non_speech_tag(speech), "{speech:?}");
        }
    }

    #[test]
    fn voice_detector_is_downloaded_from_the_silero_file() {
        assert!(vad_url().starts_with("https://huggingface.co/ggml-org/whisper-vad/"));
        assert!(vad_url().ends_with(VAD_FILE) && VAD_FILE.contains("silero"));
    }

    #[test]
    fn speech_zone_is_padded_and_never_leaves_the_audio() {
        // Parole de 1,0 s à 2,0 s (centisecondes 100 à 200) dans 5 s d'audio : 0,25 s de marge de chaque côté.
        assert_eq!(padded_range(100.0, 200.0, 80_000), 12_000..36_000);
        assert_eq!(padded_range(5.0, 380.0, 64_000), 0..64_000, "bornée au début et à la fin");
        assert_eq!(padded_range(-3.0, 10.0, 64_000), 0..5_600);
        assert!(padded_range(100.0, 200.0, 0).is_empty());
    }

    #[test]
    fn compute_threads_leaves_cores_for_the_desktop() {
        assert_eq!([1, 2, 4, 8, 16, 64].map(compute_threads), [2, 2, 3, 6, 8, 8]);
    }

    #[test]
    fn missing_model_is_an_error_not_a_panic() {
        let v = crate::adapters::stt::new_shared_vocab("");
        assert!(WhisperCpp::load(std::path::Path::new("/nonexistent/ggml.bin"), "fr", v).is_err());
    }

    fn rss_mb() -> u64 {
        let s = std::fs::read_to_string("/proc/self/status").unwrap();
        let kb: u64 = s.lines().find(|l| l.starts_with("VmRSS:")).unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
        kb / 1024
    }

    /// Test avec un vrai modèle : `PLUME_TEST_GGML=/chemin/ggml-tiny.bin cargo test --release -- --ignored whisper_memory`
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn whisper_memory_is_released_on_unload_and_restored_by_warm() {
        let model = std::env::var("PLUME_TEST_GGML").expect("set PLUME_TEST_GGML");
        let w = WhisperCpp::load(std::path::Path::new(&model), "en", crate::adapters::stt::new_shared_vocab("")).unwrap();
        let silence = AudioSamples::new(vec![0.0; 16_000 * 3], 16_000, 1);
        let heard = w.transcribe(&silence).await.unwrap().text; assert_eq!(heard, "", "le silence ne doit rien produire, entendu {heard:?}");
        let loaded = rss_mb();

        w.unload();
        let unloaded = rss_mb();
        assert!(!w.is_loaded());
        assert!(unloaded + 30 < loaded, "la RAM doit baisser après le déchargement ({loaded} Mo -> {unloaded} Mo)");

        w.warm();
        for _ in 0..100 { if w.is_loaded() { break; } std::thread::sleep(Duration::from_millis(100)); }
        assert!(w.is_loaded(), "warm() doit recharger le modèle");
        w.warm(); // déjà chargé : ne doit rien empiler ni bloquer
        assert_eq!(w.transcribe(&silence).await.unwrap().text, "");
    }
}
