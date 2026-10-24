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

/// Silero voice detector (under 1 MB): tells speech from noise before Whisper (ADR 0014).
pub const VAD_FILE: &str = "ggml-silero-v5.1.2.bin";

pub fn vad_url() -> String {
    format!("https://huggingface.co/ggml-org/whisper-vad/resolve/main/{VAD_FILE}")
}

/// Margin kept around the detected speech: 0.25 s, so word starts and ends are not cut.
const VAD_PAD: usize = 4_000;

/// The model (+ compute buffers, ~0.6 to 0.9 GB for `small`) is freed after this long without dictation.
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

/// Without the voice detector Plume stays usable: the volume and invented-phrase filters take over.
fn load_vad(path: &Path) -> Option<WhisperVadContext> {
    let mut params = WhisperVadContextParams::new();
    params.set_use_gpu(false);
    WhisperVadContext::new(path.to_str()?, params)
        .map_err(|e| tracing::warn!("voice detector {} unavailable ({e}), continuing without it", path.display()))
        .ok()
}

/// Speech zone (in centiseconds, as the detector reports it) widened by `VAD_PAD` samples and clamped to the audio.
fn padded_range(first_start_cs: f32, last_end_cs: f32, len: usize) -> Range<usize> {
    let to_samples = |cs: f32| (cs.max(0.0) * 160.0) as usize;
    to_samples(first_start_cs).saturating_sub(VAD_PAD).min(len)..(to_samples(last_end_cs) + VAD_PAD).min(len)
}

/// `None` = no speech in the audio.
fn detect_speech(vad: &mut WhisperVadContext, pcm: &[f32]) -> Result<Option<Range<usize>>> {
    let segments = vad.segments_from_samples(WhisperVadParams::new(), pcm).context("voice detection")?;
    let n = segments.num_segments();
    if n == 0 { return Ok(None); }
    let (first, last) = (segments.get_segment(0), segments.get_segment(n - 1));
    Ok(first.zip(last).map(|(f, l)| padded_range(f.start, l.end, pcm.len())))
}

/// Whisper in-process (whisper.cpp): no Python and no external server.
pub struct WhisperCpp {
    inner: Arc<Inner>,
}

impl WhisperCpp {
    /// Loads the model right away (fail fast: the caller can fall back to OpenAI), then unloads it after `IDLE_UNLOAD` of inactivity.
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

    /// Enables the voice detector: with no speech in the audio Whisper is not called (so nothing is invented);
    /// otherwise only the speech zone is sent to it.
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
        // Already loaded, or busy transcribing: nothing to do (avoids piling threads behind the lock).
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
        // Off the async thread: the dictation loop stays free to see a new key press during the computation.
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
        // "auto" = language detection; without it whisper.cpp assumes English.
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
        // Whisper flags silence/noise with a high no_speech_prob: drop those hallucinations.
        if n > 0 && no_speech / n as f32 > 0.6 {
            tracing::debug!("whisper: no_speech_prob={:.2} — discarding as silence/noise", no_speech / n as f32);
            return Ok(Transcription::default());
        }
        Ok(Transcription { text: text.trim().to_string(), speech_confirmed })
    }
}

/// whisper.cpp writes `[BLANK_AUDIO]`, `[MUSIC]`, `(silence)`… for what is not speech: these tags must never be typed into a document.
fn is_non_speech_tag(segment: &str) -> bool {
    let t = segment.trim();
    let wrapped = (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')'));
    t.is_empty() || wrapped || t.chars().all(|c| matches!(c, '♪' | '♫' | ' ' | '-'))
}

/// Compute threads: three quarters of the cores (2 minimum, 8 maximum) so the desktop stays smooth during a long dictation.
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
        // Speech from 1.0 s to 2.0 s (centiseconds 100 to 200) in 5 s of audio: 0.25 s of margin on each side.
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

    /// Test with a real model: `PLUME_TEST_GGML=/path/ggml-tiny.bin cargo test --release -- --ignored whisper_memory`
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
        w.warm(); // already loaded: must neither pile up nor block
        assert_eq!(w.transcribe(&silence).await.unwrap().text, "");
    }
}
