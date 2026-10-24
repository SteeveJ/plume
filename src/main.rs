mod domain;
mod adapters;
mod application;
mod config;
mod tray;
mod settings_gtk;
mod sysmem;
mod restart;
mod hints;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::mpsc, thread};
use application::dictation::DictationService;
use adapters::{
    audio::CpalRecorder,
    context::AtspiContext,
    history::SqliteHistory,
    stt::{OpenAiWhisper, WhisperCpp, new_shared_vocab},
    injector::ClipboardInjector,
    hotkey::EvdevListener,
    notifier::NotifySend,
    scratchpad::FileScratchpad,
    processor::{OllamaProcessor, OpenAiProcessor, AnthropicProcessor, LocalProcessor},
};
use config::{APP_NAME, AppConfig, data_dir};
use domain::ports::processor::TextProcessor;
use config::style_prompt;
use domain::ports::hotkey::HotkeyListener;
use tray::WisprTray;

/// Query HuggingFace API to find the best matching GGUF file and its SHA-256.
fn sys_notify(msg: &str) {
    let _ = notify_rust::Notification::new()
        .summary(APP_NAME)
        .body(msg)
        .id(42099)
        .show();
}

async fn hf_resolve(repo: &str, pattern: &str) -> Result<(String, Option<String>)> {
    let api = format!("https://huggingface.co/api/models/{repo}");
    let resp: serde_json::Value = reqwest::get(&api).await?.error_for_status()?.json().await?;
    let siblings = resp["siblings"].as_array()
        .context("HF API: no siblings field")?;
    // prefer exact Q4_K_M, fallback to any Q4
    let file = siblings.iter()
        .filter_map(|s| s["rfilename"].as_str())
        .filter(|n| n.ends_with(".gguf") && n.contains(pattern))
        .min_by_key(|n| n.len())  // shortest name = least quantization suffix noise
        .context(format!("no .gguf file matching '{pattern}' in {repo}"))?;
    let url = format!("https://huggingface.co/{repo}/resolve/main/{file}");
    let sha256 = siblings.iter()
        .find(|s| s["rfilename"].as_str() == Some(file))
        .and_then(|s| s["lfs"]["sha256"].as_str())
        .map(str::to_owned);
    tracing::info!("HF resolved: {file} (sha256: {})", sha256.as_deref().unwrap_or("unknown"));
    Ok((url, sha256))
}

async fn download_model(path: &std::path::Path, url: &str, expected_sha256: Option<&str>) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    if let Some(dir) = path.parent() { std::fs::create_dir_all(dir)?; }
    println!("Downloading {} → {}", url, path.display());
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .read_timeout(std::time::Duration::from_secs(60))
        .build()?;
    let mut resp = client.get(url).send().await?.error_for_status()?;
    if let (Some(len), Some(free)) = (resp.content_length(), sysmem::free_disk_mb(path)) {
        let need = len / 1_048_576 + 256;
        anyhow::ensure!(need <= free, "not enough disk space to download the model: need about {need} MB, only {free} MB free");
    }
    let tmp = path.with_extension("tmp");
    let result: Result<()> = async {
        let mut file = tokio::fs::File::create(&tmp).await?;
        let mut hasher = expected_sha256.map(|_| Sha256::new());
        while let Some(chunk) = resp.chunk().await? {
            if let Some(h) = &mut hasher { sha2::Digest::update(h, &chunk); }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        if let (Some(h), Some(expected)) = (hasher, expected_sha256) {
            let actual = hex::encode(h.finalize());
            anyhow::ensure!(actual == expected, "SHA-256 mismatch: got {actual}, expected {expected}");
            println!("SHA-256 verified.");
        }
        Ok(())
    }.await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    tokio::fs::rename(&tmp, path).await?;
    println!("Done.");
    Ok(())
}

/// Charge Whisper en local : télécharge le modèle au besoin (en prévenant l'utilisateur) et refuse s'il ne tient pas en RAM.
async fn load_local_whisper(cfg: &AppConfig, vocab: adapters::stt::SharedVocab) -> Result<WhisperCpp> {
    let file = adapters::stt::whisper_cpp::ggml_file(&cfg.stt_model);
    let path = data_dir().join(&file);
    if !path.exists() {
        sys_notify(&format!("Downloading the speech model '{}' (one time, can take a few minutes)…", cfg.stt_model));
        download_model(&path, &adapters::stt::whisper_cpp::ggml_url(&file), None).await
            .with_context(|| format!("could not download the speech model '{}'", cfg.stt_model))?;
        sys_notify("Speech model downloaded");
    }
    let size_mb = std::fs::metadata(&path)?.len() / 1_048_576;
    let available = sysmem::available_mb();
    anyhow::ensure!(
        sysmem::model_fits(size_mb, 19, available),
        "the speech model '{}' needs about {} MB of free RAM but only {} MB are available — choose a smaller STT_MODEL (for example base or small-q8_0) in Settings, or free some memory",
        cfg.stt_model, sysmem::required_mb(size_mb, 19), available.unwrap_or(0)
    );
    let whisper = WhisperCpp::load(&path, &cfg.stt_lang, vocab)?;

    // Détecteur de voix (moins de 1 Mo) : s'il manque, Plume continue avec ses filtres de volume.
    let vad = data_dir().join(adapters::stt::whisper_cpp::VAD_FILE);
    if !vad.exists() {
        if let Err(e) = download_model(&vad, &adapters::stt::whisper_cpp::vad_url(), None).await {
            tracing::warn!("voice detector not downloaded ({e:#}), continuing without it");
        }
    }
    Ok(if vad.exists() { whisper.with_vad(&vad) } else { whisper })
}

fn install_autostart() -> Result<()> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let exe = std::env::current_exe()?.canonicalize()?;

    // copy binary to ~/.local/bin/
    let bin_dir = PathBuf::from(&home).join(".local/bin");
    std::fs::create_dir_all(&bin_dir)?;
    let bin_path = bin_dir.join(APP_NAME);
    std::fs::copy(&exe, &bin_path)?;
    println!("Installed: {}", bin_path.display());

    // icon (embedded at compile time → works from any location)
    let icon_dir = PathBuf::from(&home).join(".local/share/icons/hicolor/256x256/apps");
    std::fs::create_dir_all(&icon_dir)?;
    let icon_path = icon_dir.join(format!("{APP_NAME}.png"));
    std::fs::write(&icon_path, include_bytes!("../assets/plume-idle.png"))?;
    println!("Installed: {}", icon_path.display());
    let icon_rec_path = icon_dir.join(format!("{APP_NAME}-recording.png"));
    std::fs::write(&icon_rec_path, include_bytes!("../assets/plume-recording.png"))?;
    println!("Installed: {}", icon_rec_path.display());
    // Refresh icon theme cache — skip gracefully if tool is absent (minimal installs).
    let icon_base = PathBuf::from(&home).join(".local/share/icons/hicolor");
    // `spawn` échoue simplement si l'outil est absent.
    let _ = std::process::Command::new("gtk-update-icon-cache")
        .args(["-f", "-t", icon_base.to_str().unwrap_or("")]).spawn();
    let _ = std::fs::File::open(&icon_base).and_then(|f| f.set_modified(std::time::SystemTime::now()));

    // .desktop entry
    let apps_dir = PathBuf::from(&home).join(".local/share/applications");
    std::fs::create_dir_all(&apps_dir)?;
    let desktop = format!(
        "[Desktop Entry]\nName=Plume\nComment=Voice dictation daemon\n\
         Exec={bin}\nIcon={app}\nType=Application\nCategories=Utility;\n\
         StartupNotify=false\n",
        app = APP_NAME,
        bin = bin_path.display()
    );
    std::fs::write(apps_dir.join(format!("{APP_NAME}.desktop")), &desktop)?;
    println!("Installed: {}", apps_dir.join(format!("{APP_NAME}.desktop")).display());

    // XDG autostart — started by GNOME session (has all user groups incl. input)
    let autostart_dir = PathBuf::from(&home).join(".config/autostart");
    std::fs::create_dir_all(&autostart_dir)?;
    let autostart = format!(
        "[Desktop Entry]\nName=Plume\nComment=Voice dictation daemon\n\
         Exec={bin}\nIcon={app}\nType=Application\n\
         X-GNOME-Autostart-enabled=true\nStartupNotify=false\nHidden=false\n",
        app = APP_NAME, bin = bin_path.display()
    );
    std::fs::write(autostart_dir.join(format!("{APP_NAME}.desktop")), &autostart)?;
    println!("Installed: {}", autostart_dir.join(format!("{APP_NAME}.desktop")).display());

    // Also write systemd unit (for users who prefer it, requires re-login for input group)
    let systemd_dir = PathBuf::from(&home).join(".config/systemd/user");
    std::fs::create_dir_all(&systemd_dir)?;
    let unit = format!(
        "[Unit]\nDescription=Plume dictation daemon\n\n\
         [Service]\nExecStart={bin}\nRestart=on-failure\n\n\
         [Install]\nWantedBy=default.target\n",
        bin = bin_path.display()
    );
    std::fs::write(systemd_dir.join(format!("{APP_NAME}.service")), &unit)?;
    println!("Installed: {}", systemd_dir.join(format!("{APP_NAME}.service")).display());

    println!("\nAutostart via GNOME session (re-login to activate).");
    println!("Or to run now: {}", bin_path.display());
    Ok(())
}

async fn ollama_available(_model: &str) -> bool {
    reqwest::get("http://localhost:11434/api/version").await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().any(|a| a == "--restart") {
        return restart::restart_daemon();
    }
    let result = run().await;
    if let Err(e) = &result {
        // Lancé depuis l'autostart, il n'y a pas de terminal : la notification est le seul retour à l'utilisateur.
        let msg = format!("{e:#}");
        let hint = hints::startup_hint(&msg).map(|h| format!("\n{h}")).unwrap_or_default();
        sys_notify(&format!("Plume could not start: {msg}{hint}"));
    }
    result
}

async fn run() -> Result<()> {
    if std::env::args().any(|a| a == "--install") {
        return install_autostart();
    }
    if std::env::args().any(|a| a == "--settings") {
        let cfg = AppConfig::load()?;
        settings_gtk::run_settings_window(cfg);
        return Ok(());
    }
    if std::env::args().any(|a| a == "--download-model") {
        let cfg = AppConfig::load()?;
        let (url, sha256) = match cfg.model_url {
            Some(ref u) => (u.clone(), cfg.model_sha256.clone()),
            None => hf_resolve(&cfg.model_repo, &cfg.model_pattern).await?,
        };
        return download_model(&cfg.model_path, &url, sha256.as_deref()).await;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,arboard=error,winit=error".parse().unwrap())
        )
        .init();

    config::ensure_data_dir();
    let cfg = AppConfig::load()?;
    if let Ok(conf) = std::fs::read_to_string(config::conf_path()) {
        if let Some(w) = config::conf_warnings(&conf).first() {
            sys_notify(&format!("{APP_NAME}.conf: {w}"));
        }
    }

    let processor_label = match &cfg.processor {
        config::ProcessorKind::Local     => "Local (Qwen)".into(),
        config::ProcessorKind::Ollama    => cfg.ollama_model.clone(),
        config::ProcessorKind::OpenAi    => "OpenAI".into(),
        config::ProcessorKind::Anthropic => "Anthropic".into(),
        config::ProcessorKind::None      => "None".into(),
    };
    let mode_toggle = matches!(cfg.mode, config::HotkeyMode::Toggle);

    // system tray — get handle before spawning (spawn() returns ())
    let tray_service = ksni::TrayService::new(WisprTray {
        recording: false,
        mode_toggle,
        processor_label,
    });
    let tray_handle = tray_service.handle();
    tray_service.spawn();

    // channel: DictationService → tray update thread
    let (state_tx, state_rx) = mpsc::sync_channel::<bool>(8);
    let tray_updater = tray_handle.clone();
    thread::spawn(move || {
        for recording in state_rx {
            tray_updater.update(|t: &mut WisprTray| t.recording = recording);
        }
    });

    let vocab = new_shared_vocab(&cfg.stt_prompt);

    let local_whisper = load_local_whisper(&cfg, vocab.clone()).await;
    let stt: Box<dyn domain::ports::stt::SpeechToText> = match local_whisper {
        Ok(w) => {
            tracing::info!("using local whisper.cpp ({})", cfg.stt_model);
            Box::new(w)
        }
        Err(e) => {
            tracing::warn!("local whisper unavailable ({e:#}), trying OpenAI");
            let key = cfg.openai_key.clone().with_context(|| format!(
                "OpenAI API key not found and the local speech model is unavailable ({e:#}). \
                 Set OPENAI_API_KEY or add OPENAI_API_KEY=sk-... to ~/.config/{APP_NAME}.conf"
            ))?;
            tracing::info!("using OpenAI Whisper API");
            Box::new(OpenAiWhisper::new(key, &cfg.stt_lang, vocab.clone()))
        }
    };

    let prompt = style_prompt(&cfg.processor_style, &cfg.stt_lang);
    let needs_rewrite = !matches!(cfg.processor_style.as_str(), "clean" | "");

    let processor: Option<Box<dyn TextProcessor>> = match &cfg.processor {
        config::ProcessorKind::Local if needs_rewrite => {
            // Local 0.6B can't reliably reformulate — escalate to OpenAI or Anthropic if available.
            if let Some(k) = &cfg.openai_key {
                tracing::info!("style '{}': escalating to OpenAI for reformulation", cfg.processor_style);
                Some(Box::new(OpenAiProcessor::new(k, &cfg.openai_processor_model, &prompt)))
            } else if let Some(k) = &cfg.anthropic_key {
                tracing::info!("style '{}': escalating to Anthropic for reformulation", cfg.processor_style);
                Some(Box::new(AnthropicProcessor::new(k, &cfg.anthropic_model, &prompt)))
            } else {
                tracing::warn!("style '{}' needs OpenAI/Anthropic key — falling back to clean correction", cfg.processor_style);
                sys_notify("⚠ Add OPENAI_API_KEY or ANTHROPIC_API_KEY for professional/soften/assertive/humor modes");
                // no key → run clean correction only (0.6B can't reformulate reliably)
                let clean_prompt = config::style_prompt("clean", &cfg.stt_lang);
                if cfg.model_path.exists() {
                    LocalProcessor::load(&cfg.model_path, &clean_prompt).ok().map(|p| Box::new(p) as Box<dyn TextProcessor>)
                } else { None }
            }
        }
        config::ProcessorKind::Local => {
            if !cfg.model_path.exists() {
                tracing::info!("model not found, resolving from HuggingFace…");
                sys_notify("Downloading model (~390 MB)…");
                let (url, sha256) = match cfg.model_url.clone() {
                    Some(u) => (u, cfg.model_sha256.clone()),
                    None => hf_resolve(&cfg.model_repo, &cfg.model_pattern).await
                        .unwrap_or_else(|e| {
                            tracing::warn!("HF resolve failed: {e} — using default URL");
                            ("https://huggingface.co/unsloth/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q4_K_M.gguf".to_owned(), None)
                        }),
                };
                if let Err(e) = download_model(&cfg.model_path, &url, sha256.as_deref()).await {
                    tracing::warn!("model download failed: {e} — processor disabled");
                    None
                } else {
                    match LocalProcessor::load(&cfg.model_path, &prompt) {
                        Ok(p) => { tracing::info!("processor: local llama ready"); Some(Box::new(p)) }
                        Err(e) => { tracing::warn!("model load failed: {e}"); None }
                    }
                }
            } else {
                match LocalProcessor::load(&cfg.model_path, &prompt) {
                    Ok(p) => { tracing::info!("processor: local llama ({:?})", cfg.model_path); Some(Box::new(p)) }
                    Err(e) => { tracing::warn!("local model load failed: {e}"); None }
                }
            }
        }
        config::ProcessorKind::None => {
            tracing::info!("processor: disabled");
            None
        }
        config::ProcessorKind::OpenAi => {
            match &cfg.openai_key {
                Some(k) => {
                    tracing::info!("processor: OpenAI ({})", cfg.openai_processor_model);
                    Some(Box::new(OpenAiProcessor::new(k, &cfg.openai_processor_model, &prompt)))
                }
                None => { tracing::warn!("PROCESSOR=openai but no OPENAI_API_KEY — skipping"); None }
            }
        }
        config::ProcessorKind::Anthropic => {
            match &cfg.anthropic_key {
                Some(k) => {
                    tracing::info!("processor: Anthropic ({})", cfg.anthropic_model);
                    Some(Box::new(AnthropicProcessor::new(k, &cfg.anthropic_model, &prompt)))
                }
                None => { tracing::warn!("PROCESSOR=anthropic but no ANTHROPIC_API_KEY — skipping"); None }
            }
        }
        config::ProcessorKind::Ollama => {
            if ollama_available(&cfg.ollama_model).await {
                tracing::info!("processor: Ollama ({})", cfg.ollama_model);
                Some(Box::new(OllamaProcessor::new(&cfg.ollama_model, &prompt)))
            } else {
                tracing::warn!("Ollama ({}) not reachable — processor disabled", cfg.ollama_model);
                None
            }
        }
    };

    let history = SqliteHistory::open()
        .map(|h| Box::new(h) as Box<dyn domain::ports::history::HistoryStore>)
        .map_err(|e| tracing::warn!("history unavailable: {e}"))
        .ok();

    let atspi_ctx = AtspiContext::spawn().await;

    let recorder = CpalRecorder::new().context("init audio recorder")?;
    let injector = ClipboardInjector::new().context("init text injector")?;

    let (tx, rx) = mpsc::channel();
    let listener = EvdevListener { key: cfg.hotkey, mode: cfg.mode };
    thread::spawn(move || {
        if let Err(e) = listener.listen(tx) {
            tracing::error!("hotkey listener: {e}");
        }
    });

    tracing::info!("ready — {cfg_hotkey:?} ({cfg_mode} mode)", cfg_hotkey = cfg.hotkey, cfg_mode = cfg.mode);
    let key = format!("{:?}", cfg.hotkey).trim_start_matches("KEY_").to_owned();
    sys_notify(&match (cfg.mode, cfg.ui_lang) {
        (config::HotkeyMode::Hold, config::UiLang::Fr)   => format!("Prêt — maintenez {key} et parlez"),
        (config::HotkeyMode::Hold, config::UiLang::En)   => format!("Ready — hold {key} and speak"),
        (config::HotkeyMode::Toggle, config::UiLang::Fr) => format!("Prêt — appuyez sur {key} pour démarrer/arrêter"),
        (config::HotkeyMode::Toggle, config::UiLang::En) => format!("Ready — press {key} to start/stop"),
    });

    let notifier = Box::new(NotifySend);
    let scratchpad = Box::new(FileScratchpad::new(data_dir().join("scratch.txt")));

    let mut service = DictationService::new(recorder, stt, injector, processor, notifier)
        .with_scratchpad(scratchpad)
        .with_state_tx(state_tx)
        .with_output(cfg.output)
        .with_ui_lang(cfg.ui_lang)
        .with_vocab(vocab)
        .with_content_guard(!needs_rewrite)
        .with_snippets(application::snippets::load());
    if let Some(h) = history { service = service.with_history(h); }
    if let Some(ctx) = atspi_ctx { service = service.with_context_reader(Box::new(ctx)); }

    service.run(rx).await
}
