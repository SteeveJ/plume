/// cargo run --release --bin bench
/// Measures end-to-end latency per pipeline phase, including the text processor.
use std::time::{Duration, Instant};
use plume::{
    adapters::{
        stt::{WhisperCpp, new_shared_vocab, whisper_cpp::ggml_file},
        injector::ClipboardInjector,
        processor::LocalProcessor,
    },
    config::{AppConfig, data_dir, style_prompt},
    domain::model::AudioSamples,
    domain::ports::{stt::SpeechToText, processor::TextProcessor, injector::TextInjector},
};

fn hr() { println!("{}", "─".repeat(52)); }

fn print_row(phase: &str, d: Duration) {
    println!("  {:<38} {:>8.1} ms", phase, d.as_secs_f64() * 1000.0);
}

/// Sine wave at 400 Hz — gives Whisper something to decode (not pure silence).
fn make_audio(secs: f32) -> AudioSamples {
    let sr = 16_000u32;
    let n = (sr as f32 * secs) as usize;
    let samples: Vec<f32> = (0..n)
        .map(|i| (2.0 * std::f32::consts::PI * 400.0 * i as f32 / sr as f32).sin() * 0.3)
        .collect();
    AudioSamples { samples, channels: 1, sample_rate: sr }
}

/// Realistic transcription texts of increasing length.
fn sample_text(words: usize) -> String {
    let base = "Le projet avance bien. Nous avons finalisé la réunion avec l'équipe. \
                Les développeurs ont livré la première version. Il reste quelques \
                corrections à faire avant la mise en production. ";
    base.repeat((words / 30).max(1))[..words.min(base.len() * 10)].to_string()
}

#[tokio::main]
async fn main() {
    let cfg = AppConfig::load().expect("config");
    println!("\nPlume — latency benchmark");
    println!("  STT model  : {} ({})", cfg.stt_model, if cfg.stt_lang.is_empty() { "auto-detect" } else { &cfg.stt_lang });
    let proc_name = match &cfg.processor {
        plume::config::ProcessorKind::Local => "local", plume::config::ProcessorKind::OpenAi => "openai",
        plume::config::ProcessorKind::Anthropic => "anthropic", plume::config::ProcessorKind::Ollama => "ollama",
        plume::config::ProcessorKind::None => "none",
    };
    println!("  Processor  : {proc_name}");
    println!("  Style      : {}", if cfg.processor_style.is_empty() { "clean" } else { &cfg.processor_style });
    println!();

    // ── STT ──────────────────────────────────────────────────────────
    hr();
    println!("Phase 1 — Speech-to-Text (Whisper {}, GPU)", cfg.stt_model);
    hr();

    let vocab = new_shared_vocab(&cfg.stt_prompt);
    let t0 = Instant::now();
    let whisper = match WhisperCpp::load(&data_dir().join(ggml_file(&cfg.stt_model)), &cfg.stt_lang, vocab) {
        Ok(w) => w,
        Err(e) => { eprintln!("whisper not found: {e:#}"); return; }
    };
    print_row("Cold start (model load)", t0.elapsed());

    // Warm inference at various audio lengths
    for &secs in &[2.0f32, 5.0, 10.0, 20.0, 30.0] {
        let audio = make_audio(secs);
        let t = Instant::now();
        let result = whisper.transcribe(&audio).await.unwrap_or(plume::domain::model::Transcription { text: String::new() });
        let d = t.elapsed();
        print_row(&format!("STT warm — {secs:>4.0}s audio   → {:>3} chars", result.text.len()), d);
    }

    // ── Processor ────────────────────────────────────────────────────
    println!();
    hr();
    println!("Phase 2 — Text processor (Qwen3 0.6B local, CPU)");
    hr();

    let proc_result = match LocalProcessor::load(&cfg.model_path,
        style_prompt(&cfg.processor_style, &cfg.stt_lang))
    {
        Ok(p) => {
            for &words in &[20usize, 50, 100, 200] {
                let text = sample_text(words);
                let t = Instant::now();
                let out = p.process(&text).await.unwrap_or_default();
                let d = t.elapsed();
                print_row(
                    &format!("Processor — {:>3} words in → {:>3} out", words, out.split_whitespace().count()),
                    d
                );
            }
            true
        }
        Err(e) => {
            println!("  [skipped: model not found — {e}]");
            false
        }
    };

    // ── Inject ───────────────────────────────────────────────────────
    println!();
    hr();
    println!("Phase 3 — Inject (clipboard + uinput Ctrl+V)");
    hr();

    let inject_d = match ClipboardInjector::new() {
        Ok(inj) => {
            // Startup settle (already paid)
            let long = sample_text(50);
            let texts: &[&str] = &["Ok.", "Bonjour, le projet avance bien.", long.as_str()];
            let labels = ["short (3 chars)", "medium (31 chars)", "long (200+ chars)"];
            let mut last = Duration::ZERO;
            for (text, label) in texts.iter().zip(labels.iter()) {
                let t = Instant::now();
                let _ = inj.inject(text);
                last = t.elapsed();
                print_row(&format!("Inject — {label}"), last);
            }
            Some(last)
        }
        Err(e) => { println!("  [skipped: {e}]"); None }
    };

    // ── End-to-end summary ───────────────────────────────────────────
    println!();
    hr();
    println!("End-to-end estimate (warm STT + processor + inject)");
    hr();

    // Warm STT for 5s audio
    let stt_5s = {
        let t = Instant::now();
        let _ = whisper.transcribe(&make_audio(5.0)).await;
        t.elapsed()
    };

    // Processor for 50-word text (typical output of 5s speech)
    let proc_50 = if proc_result {
        if let Ok(p) = LocalProcessor::load(&cfg.model_path,
            style_prompt(&cfg.processor_style, &cfg.stt_lang))
        {
            let t = Instant::now();
            let _ = p.process(&sample_text(50)).await;
            Some(t.elapsed())
        } else { None }
    } else { None };

    print_row("STT (5s audio, warm)", stt_5s);
    if let Some(d) = proc_50 { print_row("Processor (50 words)", d); }
    if let Some(d) = inject_d { print_row("Inject", d); }

    let total = stt_5s
        + proc_50.unwrap_or(Duration::ZERO)
        + inject_d.unwrap_or(Duration::ZERO);
    println!();
    print_row("TOTAL estimated", total);
    hr();
}
