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
    selection::SelectionReader,
    stt::SpeechToText,
};
use crate::adapters::stt::SharedVocab;
use super::hallucination;
use super::snippets::{self, Snippet};

const CHUNK_CHARS: usize = 350;

/// Beyond this the selection is not sent to the online model: cost, slowness and text sent to a third party.
const MAX_SELECTION_CHARS: usize = 20_000;

/// What Whisper understood, once the safeguards have passed.
struct Heard {
    text: String,
    duration_ms: u64,
}

/// Start of the text read, so the user can see what is about to be replaced.
fn preview(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut p: String = flat.chars().take(40).collect();
    if p.len() < flat.len() { p.push('…'); }
    p
}

/// Cause of the error as one short line, readable in a notification.
fn short_reason(e: &anyhow::Error) -> String {
    let full = format!("{e:#}");
    let mut s: String = full.chars().take(90).collect();
    if s.len() < full.len() { s.push('…'); }
    s
}

/// Cuts after a sentence end once ~350 characters are reached; no word is lost.
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

/// True if the output lost more than 10 % of the words: the "clean" prompt asks to preserve the exact words (ignored under 12 words).
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
    instruct: Option<(Box<dyn SelectionReader>, Box<dyn TextProcessor>)>,
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
            instruct: None,
        }
    }

    pub fn with_context_reader(mut self, r: Box<dyn ContextReader>) -> Self { self.context_reader = Some(r); self }
    pub fn with_history(mut self, h: Box<dyn HistoryStore>) -> Self { self.history = Some(h); self }
    pub fn with_scratchpad(mut self, s: Box<dyn ScratchpadWriter>) -> Self { self.scratchpad = Some(s); self }
    pub fn with_state_tx(mut self, tx: SyncSender<bool>) -> Self { self.state_tx = Some(tx); self }
    pub fn with_output(mut self, m: OutputMode) -> Self { self.output = m; self }
    pub fn with_ui_lang(mut self, l: UiLang) -> Self { self.ui_lang = l; self }
    /// Turn off for styles that rewrite (the text may then legitimately get shorter).
    pub fn with_content_guard(mut self, on: bool) -> Self { self.content_guard = on; self }
    /// Enables instruction mode: `selection` reads the selected text, `rewriter` (an online model) rewrites it.
    pub fn with_instruct(mut self, selection: Box<dyn SelectionReader>, rewriter: Box<dyn TextProcessor>) -> Self {
        self.instruct = Some((selection, rewriter));
        self
    }
    pub fn with_snippets(mut self, s: Vec<Snippet>) -> Self { self.snippets = s; self }
    pub fn with_vocab(mut self, v: SharedVocab) -> Self { self.vocab = Some(v); self }

    pub async fn run(&mut self, events: Receiver<HotkeyEvent>) -> Result<()> {
        // Bridge std sync channel → tokio async so we can select! during transcription.
        let (etx, mut erx) = tokio::sync::mpsc::unbounded_channel::<HotkeyEvent>();
        std::thread::spawn(move || { for e in events { let _ = etx.send(e); } });

        let mut recording = false;
        // `Some` while the user is dictating an instruction: the text that was selected at that moment.
        let mut instruct_selection: Option<String> = None;
        loop {
            let event = match erx.recv().await { Some(e) => e, None => break };
            match event {
                HotkeyEvent::RecordStart if !recording => {
                    recording = self.begin_recording();
                }
                HotkeyEvent::RecordStop if recording && instruct_selection.is_none() => {
                    recording = false;
                    self.set_state(false);
                    // Race transcription against a new RecordStart (cancel & restart).
                    // `biased`: the transcription is polled first, otherwise an already closed channel could cancel it before it starts.
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
                HotkeyEvent::InstructStart if !recording && self.instruct.is_some() => {
                    if let Some(selection) = self.read_selection() {
                        recording = self.begin_recording();
                        if recording {
                            self.notifier.notify(&format!("{} « {} » — {}", t("✎ Rewriting", "✎ Réécriture de", self.ui_lang),
                                preview(&selection), t("say what to do with it", "dites ce qu'il faut en faire", self.ui_lang)));
                            instruct_selection = Some(selection);
                        }
                    }
                }
                HotkeyEvent::InstructStop if recording && instruct_selection.is_some() => {
                    recording = false;
                    self.set_state(false);
                    let selection = instruct_selection.take().unwrap_or_default();
                    tokio::select! {
                        biased;
                        _ = self.handle_instruct_stop(selection) => {}
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

    /// Reads the desktop selection. `None` (with a notification) if nothing is selected or the text is too long.
    fn read_selection(&self) -> Option<String> {
        let (reader, _) = self.instruct.as_ref()?;
        match reader.selected_text() {
            Ok(Some(s)) if s.chars().count() <= MAX_SELECTION_CHARS => Some(s),
            Ok(Some(_)) => {
                self.notifier.notify(t("✗ Selection too long (20 000 characters at most)", "✗ Sélection trop longue (20 000 caractères au plus)", self.ui_lang));
                None
            }
            Ok(None) => {
                self.notifier.notify(t("✗ Select the text to change first", "✗ Sélectionnez d'abord le texte à modifier", self.ui_lang));
                None
            }
            Err(e) => {
                tracing::error!("cannot read the selection: {e:#}");
                let msg = format!("{} {}", t("✗ Cannot read the selection:", "✗ Lecture de la sélection impossible :", self.ui_lang), short_reason(&e));
                self.notifier.notify(&msg);
                None
            }
        }
    }

    /// Starts recording. A microphone failure (headset unplugged, microphone busy) must not stop the daemon: tell the user and wait for the next key press.
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

    /// Stops recording and returns what Whisper understood. Refusals (too short, muted microphone, nothing heard, failure) notify by themselves.
    async fn hear(&mut self) -> Option<Heard> {
        let audio = match self.recorder.stop() {
            Ok(a) => a,
            Err(e) => { tracing::error!("stop error: {e}"); return None; }
        };

        if audio.duration().as_millis() < 300 {
            tracing::warn!("audio too short, skipping");
            self.notifier.notify(t("✗ Too short", "✗ Trop court", self.ui_lang));
            return None;
        }

        // Muted or unplugged microphone: RMS almost zero. Without this guard Whisper invents text on silence.
        let rms = hallucination::rms(&audio.samples);
        if rms < 0.005 {
            tracing::warn!("audio too quiet (rms={rms:.4}), skipping — mic disconnected?");
            self.notifier.notify(t("✗ No sound — is the microphone muted?", "✗ Aucun son — le micro est-il coupé ?", self.ui_lang));
            return None;
        }

        self.notifier.notify(t("⏳ Transcribing…", "⏳ Transcription…", self.ui_lang));
        tracing::info!("transcribing ({:.1}s)…", audio.duration().as_secs_f64());

        let text = match self.stt.transcribe(&audio).await {
            Ok(heard) if !heard.text.is_empty() && !hallucination::is_hallucination(&heard.text, rms, heard.speech_confirmed) => heard.text,
            Ok(heard) => {
                tracing::warn!("nothing usable heard (rms={rms:.4}, whisper said {:?})", heard.text);
                self.notifier.notify(t("✗ Nothing heard — speak closer to the mic", "✗ Rien entendu — parlez plus près du micro", self.ui_lang));
                return None;
            }
            Err(e) => {
                tracing::error!("STT error: {e:#}");
                let msg = format!("{} {}", t("✗ Speech recognition failed:", "✗ Échec de la reconnaissance vocale :", self.ui_lang), short_reason(&e));
                self.notifier.notify(&msg);
                return None;
            }
        };
        Some(Heard { text, duration_ms: audio.duration().as_millis() as u64 })
    }

    async fn handle_record_stop(&mut self) {
        let Some(Heard { text: raw, duration_ms }) = self.hear().await else { return };

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
            if let Err(e) = h.save(&raw, &text, duration_ms) {
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

    /// Instruction spoken over a selected text: the online model rewrites the selection and the result replaces it (pasted over it).
    /// If the model fails or returns nothing, nothing is pasted: the user's text stays intact.
    async fn handle_instruct_stop(&mut self, selection: String) {
        let Some(heard) = self.hear().await else { return };
        let Some((_, rewriter)) = &self.instruct else { return };
        self.notifier.notify(t("⏳ Rewriting…", "⏳ Réécriture…", self.ui_lang));
        let result = rewriter.process(&format!("Instruction: {}\n\nText:\n{selection}", heard.text)).await;
        match result {
            Ok(out) if !out.trim().is_empty() => {
                if let Some(h) = &mut self.history {
                    if let Err(e) = h.save(&format!("[instruction] {}", heard.text), &out, heard.duration_ms) { tracing::warn!("history save error: {e}"); }
                }
                self.do_inject(&out);
                self.notifier.notify(t("✓ Text rewritten — Ctrl+Z to undo", "✓ Texte réécrit — Ctrl+Z pour annuler", self.ui_lang));
            }
            Ok(_) => self.notifier.notify(t("✗ The model returned nothing — your text is unchanged", "✗ Le modèle n'a rien rendu — votre texte est inchangé", self.ui_lang)),
            Err(e) => {
                tracing::error!("instruction failed: {e:#}");
                let msg = format!("{} {}", t("✗ Rewrite failed:", "✗ Réécriture impossible :", self.ui_lang), short_reason(&e));
                self.notifier.notify(&msg);
            }
        }
    }

    /// Corrects in chunks (a small LLM skips passages on long texts) and keeps a chunk's raw text if the correction loses content.
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

    // ---- mode instruction ----

    use crate::domain::ports::selection::MockSelectionReader;

    /// Instruction-mode test service: microphone and Whisper simulated, selection and online model provided by the caller.
    fn instruct_service(
        selection: MockSelectionReader,
        rewriter: MockTextProcessor,
        inj: MockTextInjector,
        notifier: MockNotifier,
        starts: usize,
    ) -> DictationService<MockAudioRecorder, MockSpeechToText, MockTextInjector> {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().times(starts).returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));
        let mut stt = MockSpeechToText::new();
        stt.expect_warm().return_const(());
        stt.expect_transcribe().returning(|_| Ok(Transcription { text: "mets-le au pluriel".into(), ..Default::default() }));
        DictationService::new(rec, stt, inj, None, Box::new(notifier))
            .with_instruct(Box::new(selection), Box::new(rewriter))
    }

    fn selecting(text: Option<&'static str>) -> MockSelectionReader {
        let mut sel = MockSelectionReader::new();
        sel.expect_selected_text().returning(move || Ok(text.map(String::from)));
        sel
    }

    fn quiet_notifier() -> MockNotifier {
        let mut n = MockNotifier::new();
        n.expect_notify().return_const(());
        n
    }

    #[test]
    fn instruction_rewrites_the_selection_with_the_spoken_order() {
        let mut rewriter = MockTextProcessor::new();
        rewriter.expect_process()
            .withf(|input| input.contains("Instruction: mets-le au pluriel") && input.contains("Text:\nLe chat dort."))
            .times(1)
            .returning(|_| Ok("Les chats dorment.".into()));
        let mut inj = MockTextInjector::new();
        inj.expect_inject().with(eq("Les chats dorment.")).times(1).returning(|_| Ok(()));
        let mut svc = instruct_service(selecting(Some("Le chat dort.")), rewriter, inj, quiet_notifier(), 1);
        // A stray release of the dictation key during the instruction must change nothing.
        run(&mut svc, vec![HotkeyEvent::InstructStart, HotkeyEvent::RecordStop, HotkeyEvent::InstructStop]);
    }

    #[test]
    fn nothing_selected_means_no_recording_and_a_clear_message() {
        let mut rewriter = MockTextProcessor::new();
        rewriter.expect_process().never();
        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();
        let mut notifier = MockNotifier::new();
        notifier.expect_notify().withf(|m| m.contains("Select the text")).times(1).return_const(());
        let mut svc = instruct_service(selecting(None), rewriter, inj, notifier, 0);
        run(&mut svc, vec![HotkeyEvent::InstructStart, HotkeyEvent::InstructStop]);
    }

    #[test]
    fn a_failing_model_never_touches_the_users_text() {
        let mut rewriter = MockTextProcessor::new();
        rewriter.expect_process().returning(|_| Err(anyhow::anyhow!("HTTP 401 invalid api key")));
        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();
        let mut notifier = MockNotifier::new();
        notifier.expect_notify().withf(|m| m.contains("Rewrite failed") && m.contains("invalid api key")).times(1).return_const(());
        notifier.expect_notify().return_const(());
        let mut svc = instruct_service(selecting(Some("Le chat dort.")), rewriter, inj, notifier, 1);
        run(&mut svc, vec![HotkeyEvent::InstructStart, HotkeyEvent::InstructStop]);
    }

    #[test]
    fn an_empty_answer_never_replaces_the_selection() {
        let mut rewriter = MockTextProcessor::new();
        rewriter.expect_process().returning(|_| Ok("  \n".into()));
        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();
        let mut notifier = MockNotifier::new();
        notifier.expect_notify().withf(|m| m.contains("returned nothing")).times(1).return_const(());
        notifier.expect_notify().return_const(());
        let mut svc = instruct_service(selecting(Some("Le chat dort.")), rewriter, inj, notifier, 1);
        run(&mut svc, vec![HotkeyEvent::InstructStart, HotkeyEvent::InstructStop]);
    }

    #[test]
    fn a_huge_selection_is_not_sent_to_a_third_party() {
        let huge: &'static str = Box::leak("mot ".repeat(6_000).into_boxed_str());
        let mut rewriter = MockTextProcessor::new();
        rewriter.expect_process().never();
        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();
        let mut notifier = MockNotifier::new();
        notifier.expect_notify().withf(|m| m.contains("too long")).times(1).return_const(());
        let mut svc = instruct_service(selecting(Some(huge)), rewriter, inj, notifier, 0);
        run(&mut svc, vec![HotkeyEvent::InstructStart, HotkeyEvent::InstructStop]);
    }

    #[test]
    fn without_an_online_model_the_instruction_key_does_nothing() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().never();
        let stt = MockSpeechToText::new();
        let mut inj = MockTextInjector::new();
        inj.expect_inject().never();
        let mut svc = DictationService::new(rec, stt, inj, None, Box::new(MockNotifier::new()));
        run(&mut svc, vec![HotkeyEvent::InstructStart, HotkeyEvent::InstructStop]);
    }

    #[test]
    fn preview_is_one_short_line() {
        assert_eq!(preview("Le chat\n  dort."), "Le chat dort.");
        let long = preview(&"abcdefghij ".repeat(10));
        assert_eq!(long.chars().count(), 41);
        assert!(long.ends_with('…'));
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
