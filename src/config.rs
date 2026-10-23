use anyhow::{Context, Result};
use evdev::Key;
use std::path::PathBuf;

pub const APP_NAME: &str = "plume";

const DEFAULT_CONF: &str = "\
# Plume — voice dictation daemon
# Edit this file, then restart plume (tray → Reload or pkill plume).

# --- Hotkey ---
# Key to hold (or press) to record. Default: right Ctrl.
# Values: KEY_RIGHTCTRL, KEY_RIGHTALT, KEY_CAPSLOCK, KEY_PAUSE, KEY_F1..F12, etc.
HOTKEY=KEY_RIGHTCTRL

# hold = record while key is held | toggle = press once to start, again to stop
MODE=hold

# --- Speech-to-text ---
# Language you speak. Leave empty for auto-detection.
# Examples: fr, en, es, de
STT_LANG=

# Whisper model size: tiny (fast/rough) | base | small (recommended) | medium | large-v2
# small is the best balance for non-English languages. large-v2 is the most accurate but slow.
STT_MODEL=small

# Vocabulary hints for Whisper (proper nouns, names, technical terms).
# Whisper will bias toward these words. Example: Yves, ChatGPT, Kubernetes
STT_PROMPT=

# --- Text processor ---
# Processor used to clean/rewrite the transcription.
# Values: local (Qwen3, ~390 MB, downloaded on first run) | openai | anthropic | ollama | none
PROCESSOR=local

# Style applied by the processor.
# Values: clean (fix errors only) | professional | diplomatic | soften | assertive | humor
PROCESSOR_STYLE=clean

# OpenAI (used when PROCESSOR=openai or STT falls back to Whisper API)
#OPENAI_API_KEY=sk-...
#OPENAI_PROCESSOR_MODEL=gpt-4o-mini

# Anthropic (used when PROCESSOR=anthropic)
#ANTHROPIC_API_KEY=sk-ant-...
#ANTHROPIC_MODEL=claude-haiku-4-5

# Ollama (used when PROCESSOR=ollama, must be running on localhost:11434)
#OLLAMA_MODEL=qwen3:0.6b

# --- Output ---
# Where to send the transcribed text.
# Values: inject (paste in focused window) | scratchpad (append to file + clipboard) | both
OUTPUT=inject

# --- App language ---
# Language for notifications (en or fr)
APP_LANG=en
";

pub fn data_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/share").join(APP_NAME)
}

pub fn default_model_path() -> PathBuf {
    data_dir().join("model.gguf")
}

#[derive(Clone, Copy)]
pub enum HotkeyMode { Hold, Toggle }

impl std::fmt::Display for HotkeyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { HotkeyMode::Hold => "hold", HotkeyMode::Toggle => "toggle" })
    }
}

#[derive(Clone, Copy)]
pub enum OutputMode { Inject, Scratchpad, Both }

#[derive(Clone, Copy, Default)]
pub enum UiLang { #[default] En, Fr }

#[derive(Clone)]
pub enum ProcessorKind {
    Local,
    Ollama,
    OpenAi,
    Anthropic,
    None,
}

const FR_GRAMMAR: &str = "\
Règles d'orthographe et grammaire françaises à appliquer : \
accents obligatoires (é è ê ë à â ù û ô î ï ç œ æ) ; \
accord en genre et en nombre (adjectifs, participes passés) ; \
ponctuation typographique française (espace insécable avant : ; ! ? » et après «) ; \
majuscules aux noms propres et en début de phrase ; \
apostrophe typographique (') et non droite (') ; \
trait d'union dans les nombres composés (vingt-deux, quatre-vingts) et les mots composés.";

pub fn style_prompt(style: &str, lang: &str) -> String {
    let base = match style {
        "soften" | "diplomatic" =>
            "Reformulate this message in a softer, diplomatic tone. Keep the original meaning but remove any harsh or aggressive language. Return ONLY the reformulated text.",
        "professional" =>
            "Reformulate this message in a professional tone suitable for work. Replace informal, vulgar or colloquial language with professional equivalents. Keep the original meaning. Return ONLY the reformulated text.",
        "assertive" =>
            "Reformulate this message in a clear, direct and assertive tone. Keep the original meaning. Return ONLY the reformulated text.",
        "humor" =>
            "Reformulate this message with a touch of humor while keeping the original meaning. Return ONLY the reformulated text.",
        _ =>
            "Fix punctuation, capitalisation and obvious speech-to-text errors. Preserve the user's exact words and style. Return ONLY the corrected text, nothing else.",
    };
    if lang == "fr" {
        format!("{base} {FR_GRAMMAR}")
    } else {
        base.to_owned()
    }
}

pub struct AppConfig {
    pub openai_key: Option<String>,
    pub anthropic_key: Option<String>,
    pub hotkey: Key,
    pub mode: HotkeyMode,
    pub model_path: PathBuf,
    pub model_url: Option<String>,   // None = auto-discover from HF
    pub model_sha256: Option<String>,
    pub model_repo: String,
    pub model_pattern: String,
    pub ollama_model: String,
    pub openai_processor_model: String,
    pub anthropic_model: String,
    pub processor: ProcessorKind,
    pub output: OutputMode,
    pub stt_lang: String,
    pub stt_model: String,
    pub stt_prompt: String,
    pub ui_lang: UiLang,
    pub processor_style: String,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let home = std::env::var("HOME").context("HOME not set")?;
        let path = PathBuf::from(&home).join(".config").join(format!("{APP_NAME}.conf"));

        let mut hotkey_str = "KEY_RIGHTCTRL".to_owned();
        let mut mode = HotkeyMode::Hold;
        let mut openai_key: Option<String> = None;
        let mut anthropic_key: Option<String> = None;
        let mut model_path = default_model_path();
        let mut ollama_model = "qwen3:0.6b".to_owned();
        let mut openai_processor_model = "gpt-4o-mini".to_owned();
        let mut anthropic_model = "claude-haiku-4-5".to_owned();
        let mut processor = ProcessorKind::Local;
        let mut output = OutputMode::Inject;
        let mut model_url: Option<String> = None;
        let mut model_sha256: Option<String> = None;
        let mut model_repo = "unsloth/Qwen3-0.6B-GGUF".to_owned();
        let mut model_pattern = "Q4_K_M".to_owned();
        let mut stt_lang = String::new();
        let mut stt_model = "small".to_owned();
        let mut stt_prompt = String::new();
        let mut ui_lang = UiLang::En;
        let mut processor_style = String::new();

        if !path.exists() {
            let _ = std::fs::create_dir_all(path.parent().unwrap());
            let _ = std::fs::write(&path, DEFAULT_CONF);
        }

        if let Ok(contents) = std::fs::read_to_string(&path) {
            for raw in contents.lines() {
                let line = raw.trim();
                if line.starts_with('#') { continue; }
                let Some((k, v)) = line.split_once('=') else { continue; };
                match k.trim() {
                    "OPENAI_API_KEY"          => openai_key = Some(v.trim().to_owned()),
                    "ANTHROPIC_API_KEY"        => anthropic_key = Some(v.trim().to_owned()),
                    "HOTKEY"                   => hotkey_str = v.trim().to_owned(),
                    "MODE"                     => mode = if v.trim() == "toggle" { HotkeyMode::Toggle } else { HotkeyMode::Hold },
                    "OLLAMA_MODEL"             => ollama_model = v.trim().to_owned(),
                    "OPENAI_PROCESSOR_MODEL"   => openai_processor_model = v.trim().to_owned(),
                    "ANTHROPIC_MODEL"          => anthropic_model = v.trim().to_owned(),
                    "MODEL_PATH"               => model_path = PathBuf::from(v.trim()),
                    "MODEL_URL"                => model_url = Some(v.trim().to_owned()),
                    "MODEL_SHA256"             => model_sha256 = Some(v.trim().to_owned()),
                    "MODEL_REPO"               => model_repo = v.trim().to_owned(),
                    "MODEL_PATTERN"            => model_pattern = v.trim().to_owned(),
                    "PROCESSOR"                => processor = match v.trim() {
                        "ollama"    => ProcessorKind::Ollama,
                        "openai"    => ProcessorKind::OpenAi,
                        "anthropic" => ProcessorKind::Anthropic,
                        "none"      => ProcessorKind::None,
                        _           => ProcessorKind::Local,
                    },
                    "OUTPUT"                   => output = match v.trim() {
                        "scratchpad" => OutputMode::Scratchpad,
                        "both"       => OutputMode::Both,
                        _            => OutputMode::Inject,
                    },
                    "STT_LANG"         => stt_lang = v.trim().to_owned(),
                    "STT_MODEL"        => stt_model = v.trim().to_owned(),
                    "STT_PROMPT"       => stt_prompt = v.trim().to_owned(),
                    "APP_LANG"         => ui_lang = if v.trim() == "fr" { UiLang::Fr } else { UiLang::En },
                    "PROCESSOR_STYLE"  => processor_style = v.trim().to_owned(),
                    _ => {}
                }
            }
        }

        if let Ok(k) = std::env::var("OPENAI_API_KEY") { openai_key = Some(k); }
        if let Ok(k) = std::env::var("ANTHROPIC_API_KEY") { anthropic_key = Some(k); }
        if let Ok(m) = std::env::var("WISPR_OLLAMA_MODEL") { ollama_model = m; }

        // Load vocab file and prepend to stt_prompt.
        let vocab_path = data_dir().join("vocab.txt");
        if !vocab_path.exists() {
            let _ = std::fs::create_dir_all(data_dir());
            let _ = std::fs::write(&vocab_path, include_str!("default_vocab.txt"));
        }
        if let Ok(vocab) = std::fs::read_to_string(&vocab_path) {
            let words: String = vocab.lines()
                .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            if !words.is_empty() {
                stt_prompt = if stt_prompt.is_empty() {
                    words
                } else {
                    format!("{words}, {stt_prompt}")
                };
            }
        }

        Ok(Self {
            openai_key, anthropic_key, hotkey: parse_key(&hotkey_str)?,
            mode, model_path, model_url, model_sha256, model_repo, model_pattern,
            ollama_model, openai_processor_model, anthropic_model,
            processor, output, stt_lang, stt_model, stt_prompt, ui_lang, processor_style,
        })
    }
}

pub fn parse_key(s: &str) -> Result<Key> {
    Ok(match s.trim() {
        "KEY_RIGHTCTRL"  => Key::KEY_RIGHTCTRL,
        "KEY_LEFTCTRL"   => Key::KEY_LEFTCTRL,
        "KEY_RIGHTALT"   => Key::KEY_RIGHTALT,
        "KEY_LEFTALT"    => Key::KEY_LEFTALT,
        "KEY_RIGHTMETA"  => Key::KEY_RIGHTMETA,
        "KEY_CAPSLOCK"   => Key::KEY_CAPSLOCK,
        "KEY_SCROLLLOCK" => Key::KEY_SCROLLLOCK,
        "KEY_PAUSE"      => Key::KEY_PAUSE,
        "KEY_INSERT"     => Key::KEY_INSERT,
        "KEY_F1"  => Key::KEY_F1,  "KEY_F2"  => Key::KEY_F2,
        "KEY_F3"  => Key::KEY_F3,  "KEY_F4"  => Key::KEY_F4,
        "KEY_F5"  => Key::KEY_F5,  "KEY_F6"  => Key::KEY_F6,
        "KEY_F7"  => Key::KEY_F7,  "KEY_F8"  => Key::KEY_F8,
        "KEY_F9"  => Key::KEY_F9,  "KEY_F10" => Key::KEY_F10,
        "KEY_F11" => Key::KEY_F11, "KEY_F12" => Key::KEY_F12,
        other => anyhow::bail!(
            "unknown HOTKEY '{other}'; valid: KEY_RIGHTCTRL, KEY_RIGHTALT, KEY_PAUSE, KEY_INSERT, KEY_F1..F12, etc."
        ),
    })
}
