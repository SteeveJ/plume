#[cfg(test)]
use mockall::predicate::*;

fn t<'a>(en: &'a str, fr: &'a str, lang: UiLang) -> &'a str {
    match lang { UiLang::Fr => fr, _ => en }
}
use anyhow::Result;
use arboard::Clipboard;
use std::sync::mpsc::{Receiver, SyncSender};
use crate::config::{OutputMode, UiLang};
use crate::domain::model::HotkeyEvent;
use crate::domain::ports::{
    audio::AudioRecorder,
    context::ContextReader,
    history::HistoryStore,
    injector::TextInjector,
    notifier::Notifier,
    processor::TextProcessor,
    scratchpad::ScratchpadWriter,
    stt::SpeechToText,
};
use crate::adapters::stt::SharedVocab;
use super::hallucination;
use super::snippets::{self, Snippet};

const CHUNK_CHARS: usize = 350;

/// Cause de l'erreur en une ligne courte, lisible dans une notification.
fn short_reason(e: &anyhow::Error) -> String {
    let full = format!("{e:#}");
    let mut s: String = full.chars().take(90).collect();
    if s.len() < full.len() { s.push('…'); }
    s
}

/// Coupe après une fin de phrase une fois ~350 caractères atteints ; aucun mot n'est perdu.
fn split_chunks(text: &str) -> Vec<&str> {
    let (mut chunks, mut start, mut prev) = (Vec::new(), 0, '\0');
    for (i, c) in text.char_indices() {
        if c.is_whitespace() && matches!(prev, '.' | '!' | '?') && i - start >= CHUNK_CHARS {
            chunks.push(text[start..i].trim());
            start = i;
        }
        prev = c;
    }
    let last = text[start..].trim();
    if !last.is_empty() || chunks.is_empty() { chunks.push(last); }
    chunks
}

/// Vrai si la sortie a perdu plus de 10 % des mots : le prompt "clean" demande de préserver les mots exacts (ignoré sous 12 mots).
fn lost_content(input: &str, output: &str) -> bool {
    let (a, b) = (input.split_whitespace().count(), output.split_whitespace().count());
    a >= 12 && b * 10 < a * 9
}

pub struct DictationService<R, S, I> {
    recorder: R,
    stt: S,
    injector: I,
    processor: Option<Box<dyn TextProcessor>>,
    context_reader: Option<Box<dyn ContextReader>>,
    history: Option<Box<dyn HistoryStore>>,
    notifier: Box<dyn Notifier>,
    scratchpad: Option<Box<dyn ScratchpadWriter>>,
    state_tx: Option<SyncSender<bool>>,
    output: OutputMode,
    ui_lang: UiLang,
    vocab: Option<SharedVocab>,
    snippets: Vec<Snippet>,
    content_guard: bool,
}

impl<R, S, I> DictationService<R, S, I>
where
    R: AudioRecorder,
    S: SpeechToText,
    I: TextInjector,
{
    pub fn new(
        recorder: R,
        stt: S,
        injector: I,
        processor: Option<Box<dyn TextProcessor>>,
        notifier: Box<dyn Notifier>,
    ) -> Self {
        Self {
            recorder, stt, injector, processor,
            context_reader: None,
            notifier,
            history: None,
            scratchpad: None,
            state_tx: None,
            output: OutputMode::Inject,
            ui_lang: UiLang::En,
            vocab: None,
            snippets: Vec::new(),
            content_guard: true,
        }
    }

    pub fn with_context_reader(mut self, r: Box<dyn ContextReader>) -> Self { self.context_reader = Some(r); self }
    pub fn with_history(mut self, h: Box<dyn HistoryStore>) -> Self { self.history = Some(h); self }
    pub fn with_scratchpad(mut self, s: Box<dyn ScratchpadWriter>) -> Self { self.scratchpad = Some(s); self }
    pub fn with_state_tx(mut self, tx: SyncSender<bool>) -> Self { self.state_tx = Some(tx); self }
    pub fn with_output(mut self, m: OutputMode) -> Self { self.output = m; self }
    pub fn with_ui_lang(mut self, l: UiLang) -> Self { self.ui_lang = l; self }
    /// Désactiver pour les styles qui reformulent (le texte peut alors raccourcir légitimement).
    pub fn with_content_guard(mut self, on: bool) -> Self { self.content_guard = on; self }
    pub fn with_snippets(mut self, s: Vec<Snippet>) -> Self { self.snippets = s; self }
    pub fn with_vocab(mut self, v: SharedVocab) -> Self { self.vocab = Some(v); self }

    pub async fn run(&mut self, events: Receiver<HotkeyEvent>) -> Result<()> {
        // Bridge std sync channel → tokio async so we can select! during transcription.
        let (etx, mut erx) = tokio::sync::mpsc::unbounded_channel::<HotkeyEvent>();
        std::thread::spawn(move || { for e in events { let _ = etx.send(e); } });

        let mut recording = false;
        loop {
            let event = match erx.recv().await { Some(e) => e, None => break };
            match event {
                HotkeyEvent::RecordStart if !recording => {
                    recording = self.begin_recording();
                }
                HotkeyEvent::RecordStop if recording => {
                    recording = false;
                    self.set_state(false);
                    // Race transcription against a new RecordStart (cancel & restart).
                    // `biased` : la transcription est sondée d'abord, sinon un canal déjà fermé pouvait l'annuler avant qu'elle démarre.
                    tokio::select! {
                        biased;
                        _ = self.handle_record_stop() => {}
                        next = erx.recv() => {
                            if let Some(HotkeyEvent::RecordStart) = next {
                                recording = self.begin_recording();
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Démarre l'enregistrement. Une panne de micro (casque débranché, micro occupé) ne doit pas arrêter le démon : on prévient et on attend la touche suivante.
    fn begin_recording(&mut self) -> bool {
        match self.recorder.start() {
            Ok(()) => {
                tracing::info!("recording…");
                self.stt.warm();
                self.set_state(true);
                self.notifier.notify(t("🎤 Recording…", "🎤 Enregistrement…", self.ui_lang));
                true
            }
            Err(e) => {
                tracing::error!("cannot start recording: {e:#}");
                let msg = format!("{} {}", t("✗ Microphone unavailable:", "✗ Micro indisponible :", self.ui_lang), short_reason(&e));
                self.notifier.notify(&msg);
                false
            }
        }
    }

    async fn handle_record_stop(&mut self) {
        let audio = match self.recorder.stop() {
            Ok(a) => a,
            Err(e) => { tracing::error!("stop error: {e}"); return; }
        };

        if audio.duration().as_millis() < 300 {
            tracing::warn!("audio too short, skipping");
            self.notifier.notify(t("✗ Too short", "✗ Trop court", self.ui_lang));
            return;
        }

        // Micro coupé ou débranché : RMS quasi nul. Sans ce garde-fou Whisper invente du texte sur le silence.
        let rms = hallucination::rms(&audio.samples);
        if rms < 0.005 {
            tracing::warn!("audio too quiet (rms={rms:.4}), skipping — mic disconnected?");
            self.notifier.notify(t("✗ No sound — is the microphone muted?", "✗ Aucun son — le micro est-il coupé ?", self.ui_lang));
            return;
        }

        self.notifier.notify(t("⏳ Transcribing…", "⏳ Transcription…", self.ui_lang));
        tracing::info!("transcribing ({:.1}s)…", audio.duration().as_secs_f64());

        let raw = match self.stt.transcribe(&audio).await {
            Ok(heard) if !heard.text.is_empty() && !hallucination::is_hallucination(&heard.text, rms, heard.speech_confirmed) => heard.text,
            Ok(heard) => {
                tracing::warn!("nothing usable heard (rms={rms:.4}, whisper said {:?})", heard.text);
                self.notifier.notify(t("✗ Nothing heard — speak closer to the mic", "✗ Rien entendu — parlez plus près du micro", self.ui_lang));
                return;
            }
            Err(e) => {
                tracing::error!("STT error: {e:#}");
                let msg = format!("{} {}", t("✗ Speech recognition failed:", "✗ Échec de la reconnaissance vocale :", self.ui_lang), short_reason(&e));
                self.notifier.notify(&msg);
                return;
            }
        };

        // Vocab learning and LLM processing are independent — run them in parallel.
        let vocab_task = self.vocab.as_ref().map(|vocab| {
            use crate::adapters::stt::shared_vocab::{extract_words, persist_words};
            let vocab = vocab.clone();
            let raw_for_vocab = raw.clone();
            let existing = vocab.lock().unwrap().clone();
            tokio::task::spawn_blocking(move || {
                let new_words = extract_words(&raw_for_vocab, &existing);
                if !new_words.is_empty() {
                    tracing::debug!("vocab learned: {:?}", new_words);
                    if let Some(updated) = persist_words(&new_words) {
                        *vocab.lock().unwrap() = updated;
                    }
                }
            })
        });

        let ctx = self.context_reader.as_ref().and_then(|cr| cr.get_context());
        let text = match &self.processor {
            Some(p) => self.run_processor(p.as_ref(), &raw, ctx.as_deref()).await,
            None => raw.clone(),
        };
        let text = snippets::expand(&text, &self.snippets);

        if let Some(h) = &mut self.history {
            if let Err(e) = h.save(&raw, &text, audio.duration().as_millis() as u64) {
                tracing::warn!("history save error: {e}");
            }
        }

        tracing::info!("output: {:?}", text);
        match self.output {
            OutputMode::Inject => self.do_inject(&text),
            OutputMode::Scratchpad => self.do_scratchpad(&text),
            OutputMode::Both => { self.do_scratchpad(&text); self.do_inject(&text); }
        }

        // Await vocab task before returning — ensures vocab is current for the next dictation.
        if let Some(task) = vocab_task { let _ = task.await; }

        self.notifier.notify(t("✓ Done", "✓ Fait", self.ui_lang));
    }

    /// Corrige par morceaux (un petit LLM saute des passages sur les longs textes) et garde le texte brut d'un morceau si la correction perd du contenu.
    async fn run_processor(&self, p: &dyn TextProcessor, raw: &str, ctx: Option<&str>) -> String {
        let chunks = if self.content_guard { split_chunks(raw) } else { vec![raw.trim()] };
        let mut out = Vec::with_capacity(chunks.len());
        for (i, chunk) in chunks.into_iter().enumerate() {
            let input = match ctx {
                Some(c) if i == 0 => format!(
                    "[Context before cursor — use for tone, do NOT include in output]:\n{c}\n\n[Dictated text to correct]:\n{chunk}"
                ),
                _ => chunk.to_string(),
            };
            match p.process(&input).await {
                Ok(t) if !t.trim().is_empty() && !(self.content_guard && lost_content(chunk, &t)) => out.push(t),
                Ok(_) => { tracing::warn!("processor returned empty or shortened text, keeping raw chunk"); out.push(chunk.to_string()); }
                Err(e) => { tracing::warn!("processor error: {e}"); out.push(chunk.to_string()); }
            }
        }
        out.join(" ")
    }

    fn do_inject(&self, text: &str) {
        if let Err(e) = self.injector.inject(text) {
            tracing::error!("inject error: {e}");
            self.notifier.notify(t("✗ Inject error", "✗ Erreur injection", self.ui_lang));
        }
    }

    fn do_scratchpad(&self, text: &str) {
        if let Some(s) = &self.scratchpad {
            s.append(text);
        }
        if let Ok(mut cb) = Clipboard::new() {
            let _ = cb.set_text(text.to_owned());
        }
    }

    fn set_state(&self, recording: bool) {
        if let Some(tx) = &self.state_tx {
            let _ = tx.try_send(recording);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use crate::domain::model::{AudioSamples, Transcription};
    use crate::domain::ports::{
        audio::MockAudioRecorder,
        stt::MockSpeechToText,
        injector::MockTextInjector,
        notifier::MockNotifier,
        processor::MockTextProcessor,
        scratchpad::MockScratchpadWriter,
    };

    // Sine wave at 400 Hz — RMS ≈ 0.21, well above the 0.005 silence threshold.
    fn samples(secs: f32) -> AudioSamples {
        let n = (secs * 16000.0) as usize;
        let sr = 16000u32;
        let data: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 400.0 * i as f32 / sr as f32).sin() * 0.3)
            .collect();
        AudioSamples::new(data, sr, 1)
    }

    fn silent_samples(secs: f32) -> AudioSamples {
        let n = (secs * 16000.0) as usize;
        AudioSamples::new(vec![0.0; n], 16000, 1)
    }

    fn make_service(
        rec: MockAudioRecorder,
        mut stt: MockSpeechToText,
        inj: MockTextInjector,
        notifier: MockNotifier,
    ) -> DictationService<MockAudioRecorder, MockSpeechToText, MockTextInjector> {
        stt.expect_warm().return_const(());
        DictationService::new(rec, stt, inj, None, Box::new(notifier))
    }

    fn run(
        svc: &mut DictationService<MockAudioRecorder, MockSpeechToText, MockTextInjector>,
        events: Vec<HotkeyEvent>,
    ) {
        let (tx, rx) = mpsc::channel();
        for e in events { tx.send(e).unwrap(); }
        drop(tx);
        tokio::runtime::Builder::new_current_thread()
            .build().unwrap()
            .block_on(svc.run(rx)).unwrap();
    }

    #[test]
    fn start_then_stop_injects_transcription() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));

        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe()
            .returning(|_| Ok(Transcription { text: "hello world".into(), ..Default::default() }));

        let mut inj = MockTextInjector::new();
        inj.expect_inject()
            .with(eq("hello world"))
            .returning(|_| Ok(()));

        let mut notifier = MockNotifier::new();
        notifier.expect_notify().returning(|_| ());

        let mut svc = make_service(rec, stt, inj, notifier);
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn microphone_failure_notifies_and_the_next_press_still_works() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().times(1).returning(|| Err(anyhow::anyhow!("no microphone found")));
        rec.expect_start().times(1).returning(|| Ok(()));
        rec.expect_stop().times(1).returning(|| Ok(samples(1.0)));
        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe().times(1).returning(|_| Ok(Transcription { text: "hello".into(), ..Default::default() }));
        let mut inj = MockTextInjector::new();
        inj.expect_inject().with(eq("hello")).times(1).returning(|_| Ok(()));
        let mut notifier = MockNotifier::new();
        notifier.expect_notify().withf(|m| m.contains("Microphone unavailable") && m.contains("no microphone found")).times(1).return_const(());
        notifier.expect_notify().return_const(());
        let mut svc = DictationService::new(rec, stt, inj, None, Box::new(notifier));
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn stt_failure_tells_the_user_why() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));
        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe().returning(|_| Err(anyhow::anyhow!("whisper model not loaded")));
        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();
        let mut notifier = MockNotifier::new();
        notifier.expect_notify().withf(|m| m.contains("recognition failed") && m.contains("whisper model not loaded")).times(1).return_const(());
        notifier.expect_notify().return_const(());
        let mut svc = DictationService::new(rec, stt, inj, None, Box::new(notifier));
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn short_reason_is_one_bounded_line() {
        let e = anyhow::anyhow!("root cause").context("while doing X");
        assert_eq!(short_reason(&e), "while doing X: root cause");
        let long = anyhow::anyhow!("é".repeat(200));
        assert_eq!(short_reason(&long).chars().count(), 91);
    }

    #[test]
    fn split_chunks_keeps_every_word() {
        let t = "Une phrase assez longue pour remplir. ".repeat(40);
        let chunks = split_chunks(&t);
        assert!(chunks.len() > 1);
        assert_eq!(chunks.join(" ").split_whitespace().count(), t.split_whitespace().count());
        assert_eq!(split_chunks("court."), vec!["court."]);
        assert_eq!(split_chunks(""), vec![""]);
    }

    #[test]
    fn split_chunks_never_cuts_inside_an_accented_letter() {
        let t = "Élève très pressé, il a été à côté du château. ".repeat(40);
        let chunks = split_chunks(&t);
        assert!(chunks.len() > 1);
        assert!(chunks.iter().all(|c| c.ends_with('.')), "chaque morceau finit une phrase : {chunks:?}");
        assert_eq!(chunks.join(" ").split_whitespace().count(), t.split_whitespace().count());
    }

    #[test]
    fn lost_content_ignores_short_texts_and_flags_big_drops() {
        assert!(!lost_content("euh je veux ça", "je veux ça"));
        let long = "mot ".repeat(30);
        assert!(lost_content(&long, &"mot ".repeat(20)));
        assert!(lost_content(&long, &"mot ".repeat(26)));
        assert!(!lost_content(&long, &"mot ".repeat(28)));
    }

    #[test]
    fn empty_processor_output_never_erases_the_dictation() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));
        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe().returning(|_| Ok(Transcription { text: "salut jean".into(), ..Default::default() }));
        let mut proc = MockTextProcessor::new();
        proc.expect_process().returning(|_| Ok("  ".into()));
        let mut inj = MockTextInjector::new();
        inj.expect_inject().with(eq("salut jean")).times(1).returning(|_| Ok(()));
        let mut notifier = MockNotifier::new();
        notifier.expect_notify().return_const(());
        let mut svc = DictationService::new(rec, stt, inj, Some(Box::new(proc)), Box::new(notifier));
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn processor_that_drops_content_falls_back_to_raw() {
        let raw = "un deux trois quatre cinq six sept huit neuf dix onze douze treize quatorze";
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));
        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe().returning(move |_| Ok(Transcription { text: raw.into(), ..Default::default() }));
        let mut proc = MockTextProcessor::new();
        proc.expect_process().returning(|_| Ok("Un deux.".into()));
        let mut inj = MockTextInjector::new();
        inj.expect_inject().with(eq(raw)).returning(|_| Ok(()));
        let mut notifier = MockNotifier::new();
        notifier.expect_notify().returning(|_| ());
        let mut svc = DictationService::new(rec, stt, inj, Some(Box::new(proc)), Box::new(notifier));
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn processor_cleans_text_before_inject() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));

        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe()
            .returning(|_| Ok(Transcription { text: "raw text".into(), ..Default::default() }));

        let mut proc = MockTextProcessor::new();
        proc.expect_process()
            .with(eq("raw text"))
            .returning(|_| Ok("Cleaned text.".into()));

        let mut inj = MockTextInjector::new();
        inj.expect_inject()
            .with(eq("Cleaned text."))
            .returning(|_| Ok(()));

        let mut notifier = MockNotifier::new();
        notifier.expect_notify().returning(|_| ());

        let mut svc = DictationService::new(rec, stt, inj, Some(Box::new(proc)), Box::new(notifier));
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn short_audio_notifies_and_skips_inject() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(0.1))); // too short

        let mut inj = MockTextInjector::new();
        inj.expect_inject().never(); // must NOT be called

        let mut notifier = MockNotifier::new();
        notifier.expect_notify()
            .with(eq("✗ Too short"))
            .returning(|_| ());
        notifier.expect_notify()
            .with(eq("🎤 Recording…"))
            .returning(|_| ());

        let stt = MockSpeechToText::new();
        let mut svc = make_service(rec, stt, inj, notifier);
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn stt_error_notifies_and_skips_inject() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));

        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe()
            .returning(|_| Err(anyhow::anyhow!("STT failed")));

        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();

        let mut notifier = MockNotifier::new();
        notifier.expect_notify().returning(|_| ());

        let mut svc = make_service(rec, stt, inj, notifier);
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn processor_error_falls_back_to_raw() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));

        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe()
            .returning(|_| Ok(Transcription { text: "raw".into(), ..Default::default() }));

        let mut proc = MockTextProcessor::new();
        proc.expect_process()
            .returning(|_| Err(anyhow::anyhow!("proc failed")));

        let mut inj = MockTextInjector::new();
        inj.expect_inject()
            .with(eq("raw")) // falls back to raw
            .returning(|_| Ok(()));

        let mut notifier = MockNotifier::new();
        notifier.expect_notify().returning(|_| ());

        let mut svc = DictationService::new(rec, stt, inj, Some(Box::new(proc)), Box::new(notifier));
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn silent_audio_skips_stt_and_inject() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(silent_samples(1.0)));

        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe().never(); // must NOT be called

        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();

        let mut notifier = MockNotifier::new();
        notifier.expect_notify().returning(|_| ());

        let mut svc = make_service(rec, stt, inj, notifier);
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }

    #[test]
    fn scratchpad_mode_writes_and_skips_inject() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));

        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe()
            .returning(|_| Ok(Transcription { text: "note this".into(), ..Default::default() }));

        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();

        let mut scratch = MockScratchpadWriter::new();
        scratch.expect_append()
            .with(eq("note this"))
            .returning(|_| ());

        let mut notifier = MockNotifier::new();
        notifier.expect_notify().returning(|_| ());

        let mut svc = DictationService::new(rec, stt, inj, None, Box::new(notifier))
            .with_scratchpad(Box::new(scratch))
            .with_output(crate::config::OutputMode::Scratchpad);
        run(&mut svc, vec![HotkeyEvent::RecordStart, HotkeyEvent::RecordStop]);
    }
}
