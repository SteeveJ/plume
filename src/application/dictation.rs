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
    history::HistoryStore,
    injector::TextInjector,
    notifier::Notifier,
    processor::TextProcessor,
    scratchpad::ScratchpadWriter,
    stt::SpeechToText,
};
use crate::adapters::stt::SharedVocab;

pub struct DictationService<R, S, I> {
    recorder: R,
    stt: S,
    injector: I,
    processor: Option<Box<dyn TextProcessor>>,
    history: Option<Box<dyn HistoryStore>>,
    notifier: Box<dyn Notifier>,
    scratchpad: Option<Box<dyn ScratchpadWriter>>,
    state_tx: Option<SyncSender<bool>>,
    output: OutputMode,
    ui_lang: UiLang,
    vocab: Option<SharedVocab>,
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
            notifier,
            history: None,
            scratchpad: None,
            state_tx: None,
            output: OutputMode::Inject,
            ui_lang: UiLang::En,
            vocab: None,
        }
    }

    pub fn with_history(mut self, h: Box<dyn HistoryStore>) -> Self { self.history = Some(h); self }
    pub fn with_scratchpad(mut self, s: Box<dyn ScratchpadWriter>) -> Self { self.scratchpad = Some(s); self }
    pub fn with_state_tx(mut self, tx: SyncSender<bool>) -> Self { self.state_tx = Some(tx); self }
    pub fn with_output(mut self, m: OutputMode) -> Self { self.output = m; self }
    pub fn with_ui_lang(mut self, l: UiLang) -> Self { self.ui_lang = l; self }
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
                    tracing::info!("recording…");
                    self.recorder.start()?;
                    recording = true;
                    self.set_state(true);
                    self.notifier.notify(t("🎤 Recording…", "🎤 Enregistrement…", self.ui_lang));
                }
                HotkeyEvent::RecordStop if recording => {
                    recording = false;
                    self.set_state(false);
                    // Race transcription against a new RecordStart (cancel & restart).
                    tokio::select! {
                        _ = self.handle_record_stop() => {}
                        next = erx.recv() => {
                            if let Some(HotkeyEvent::RecordStart) = next {
                                self.recorder.start()?;
                                recording = true;
                                self.set_state(true);
                                self.notifier.notify(t("🎤 Recording…", "🎤 Enregistrement…", self.ui_lang));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
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

        // Reject silent/disconnected mic: RMS below 0.01 means no real speech.
        let rms = {
            let sum: f32 = audio.samples.iter().map(|s| s * s).sum();
            (sum / audio.samples.len().max(1) as f32).sqrt()
        };
        if rms < 0.005 {
            tracing::warn!("audio too quiet (rms={rms:.4}), skipping — mic disconnected?");
            return;
        }

        self.notifier.notify(t("⏳ Transcribing…", "⏳ Transcription…", self.ui_lang));
        tracing::info!("transcribing ({:.1}s)…", audio.duration().as_secs_f64());

        let raw = match self.stt.transcribe(&audio).await {
            Ok(t) if !t.text.is_empty() => t.text,
            Ok(_) => { tracing::warn!("empty transcription"); self.notifier.notify(t("✗ Empty", "✗ Vide", self.ui_lang)); return; }
            Err(e) => { tracing::error!("STT error: {e}"); self.notifier.notify(t("✗ STT error", "✗ Erreur STT", self.ui_lang)); return; }
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

        let text = match &self.processor {
            Some(p) => match p.process(&raw).await {
                Ok(t) => t,
                Err(e) => { tracing::warn!("processor error: {e}"); raw.clone() }
            },
            None => raw.clone(),
        };

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
        stt: MockSpeechToText,
        inj: MockTextInjector,
        notifier: MockNotifier,
    ) -> DictationService<MockAudioRecorder, MockSpeechToText, MockTextInjector> {
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
        stt.expect_transcribe()
            .returning(|_| Ok(Transcription { text: "hello world".into() }));

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
    fn processor_cleans_text_before_inject() {
        let mut rec = MockAudioRecorder::new();
        rec.expect_start().returning(|| Ok(()));
        rec.expect_stop().returning(|| Ok(samples(1.0)));

        let mut stt = MockSpeechToText::new();
        stt.expect_transcribe()
            .returning(|_| Ok(Transcription { text: "raw text".into() }));

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
        stt.expect_transcribe()
            .returning(|_| Ok(Transcription { text: "raw".into() }));

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
        stt.expect_transcribe()
            .returning(|_| Ok(Transcription { text: "note this".into() }));

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
