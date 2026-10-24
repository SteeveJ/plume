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

# --- Instruction mode (optional) ---
# Select text in any app, hold this key and say what to do with it ('make it more formal', 'translate to English').
# Plume rewrites the selection with an online model and pastes the result over it (Ctrl+Z undoes it).
# Needs OPENAI_API_KEY or ANTHROPIC_API_KEY: the selected text is sent to that service. Leave empty to disable.
# Must be a different key from HOTKEY. Same values as HOTKEY.
INSTRUCT_HOTKEY=

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

pub fn conf_path() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config").join(format!("{APP_NAME}.conf"))
}

/// Writes a file readable by its owner only (API keys, dictation history).
pub fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
    f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    f.write_all(contents.as_bytes())
}

/// Creates the data folder as 0700 and tightens an existing one.
pub fn ensure_data_dir() -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    dir
}

/// Tightens an existing file to 0600 (files created before this fix were readable by everyone).
pub fn restrict_file(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if path.exists() {
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

/// Replaces the value of `key` (exact key) or adds it; the rest of the file is kept.
pub fn update_conf(contents: &str, key: &str, value: &str) -> String {
    let new_line = format!("{key}={value}");
    let (mut out, mut done) = (Vec::new(), false);
    for line in contents.lines() {
        let is_key = !line.trim_start().starts_with('#')
            && line.split_once('=').is_some_and(|(k, _)| k.trim() == key);
        if !is_key { out.push(line.to_string()); }
        else if !done { out.push(new_line.clone()); done = true; }
    }
    if !done { out.push(new_line); }
    out.join("\n") + "\n"
}

/// Updates one key of the config file without touching the other settings.
pub fn set_conf_key(key: &str, value: &str) {
    let path = conf_path();
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = write_private(&path, &update_conf(&existing, key, value));
}

const KNOWN_KEYS: &[&str] = &[
    "OPENAI_API_KEY", "ANTHROPIC_API_KEY", "HOTKEY", "MODE", "OLLAMA_MODEL", "OPENAI_PROCESSOR_MODEL",
    "ANTHROPIC_MODEL", "MODEL_PATH", "MODEL_URL", "MODEL_SHA256", "MODEL_REPO", "MODEL_PATTERN", "PROCESSOR",
    "OUTPUT", "STT_LANG", "STT_MODEL", "STT_PROMPT", "APP_LANG", "PROCESSOR_STYLE", "INSTRUCT_HOTKEY",
];

/// Spots typos that would otherwise be silently ignored (e.g. `PROCESSOR=olama` falls back to the local LLM and starts a download).
pub fn conf_warnings(contents: &str) -> Vec<String> {
    let choices: &[(&str, &[&str])] = &[
        ("MODE", &["hold", "toggle"]),
        ("PROCESSOR", &["local", "ollama", "openai", "anthropic", "none"]),
        ("OUTPUT", &["inject", "scratchpad", "both"]),
        ("APP_LANG", &["en", "fr"]),
        ("PROCESSOR_STYLE", STYLES),
    ];
    let mut out = Vec::new();
    for (i, raw) in contents.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let n = i + 1;
        let Some((k, v)) = line.split_once('=') else {
            out.push(format!("line {n}: expected KEY=VALUE, ignored"));
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if !KNOWN_KEYS.contains(&k) {
            out.push(format!("line {n}: unknown setting {k}"));
        } else if let Some((_, ok)) = choices.iter().find(|(name, _)| *name == k) {
            if !ok.contains(&v) { out.push(format!("line {n}: {k}={v} is not valid (use: {})", ok.join(", "))); }
        } else if (k == "HOTKEY" || (k == "INSTRUCT_HOTKEY" && !v.is_empty())) && parse_key(v).is_err() {
            out.push(format!("line {n}: {k}={v} is not a supported key"));
        } else if k == "STT_MODEL" && !v.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
            out.push(format!("line {n}: STT_MODEL={v} is not a valid model name"));
        }
    }
    let value_of = |key: &str| contents.lines().filter(|l| !l.trim_start().starts_with('#')).filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_string());
    if let (Some(a), Some(b)) = (value_of("HOTKEY"), value_of("INSTRUCT_HOTKEY")) {
        if !b.is_empty() && a == b { out.push(format!("INSTRUCT_HOTKEY must differ from HOTKEY ({a})")); }
    }
    out
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

/// Rewrite styles (empty = correction only). `clean` corrects without rewriting; the others rewrite and need an online LLM.
pub const STYLES: &[&str] = &[
    "", "clean", "casual", "excited", "formal", "professional", "diplomatic", "soften", "assertive", "humor",
];

/// Brief for instruction mode: the selected text is rewritten according to the phrase the user spoke.
pub const INSTRUCT_PROMPT: &str = "You edit the user's text exactly as they asked. The message contains an Instruction (spoken, so it may have small \
speech-to-text mistakes) and the Text to change. Apply the instruction to the text. Keep the language of the text unless the instruction asks for \
another one. Return ONLY the edited text: no explanation, no quotes, no markdown fence.";

pub fn style_prompt(style: &str, lang: &str) -> String {
    let base = match style {
        "casual" =>
            "Reformulate this message in a casual, friendly tone, as if writing to a colleague you know well. Keep the original meaning. Return ONLY the reformulated text.",
        "excited" =>
            "Reformulate this message in an enthusiastic, upbeat tone. Keep the original meaning and use exclamation marks sparingly. Return ONLY the reformulated text.",
        "formal" =>
            "Reformulate this message in a formal, polite tone suitable for an email to someone you do not know. Use complete sentences and avoid slang and contractions. Keep the original meaning. Return ONLY the reformulated text.",
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
    /// Second key of instruction mode; `None` = disabled.
    pub instruct_hotkey: Option<Key>,
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
        std::env::var("HOME").context("HOME not set")?;
        let path = conf_path();

        let mut hotkey_str = "KEY_RIGHTCTRL".to_owned();
        let mut instruct_str = String::new();
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
            let _ = write_private(&path, DEFAULT_CONF);
        }
        restrict_file(&path);

        if let Ok(contents) = std::fs::read_to_string(&path) {
            for w in conf_warnings(&contents) { tracing::warn!("{}: {w}", path.display()); }
            for raw in contents.lines() {
                let line = raw.trim();
                if line.starts_with('#') { continue; }
                let Some((k, v)) = line.split_once('=') else { continue; };
                match k.trim() {
                    "OPENAI_API_KEY"          => openai_key = Some(v.trim().to_owned()),
                    "ANTHROPIC_API_KEY"        => anthropic_key = Some(v.trim().to_owned()),
                    "HOTKEY"                   => hotkey_str = v.trim().to_owned(),
                    "INSTRUCT_HOTKEY"          => instruct_str = v.trim().to_owned(),
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

        let hotkey = parse_key(&hotkey_str)?;
        let instruct_hotkey = if instruct_str.is_empty() { None } else {
            let k = parse_key(&instruct_str)?;
            if k == hotkey { tracing::warn!("INSTRUCT_HOTKEY is the same key as HOTKEY: instruction mode disabled"); None } else { Some(k) }
        };

        Ok(Self {
            openai_key, anthropic_key, hotkey, instruct_hotkey,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_conf_replaces_exact_key_only() {
        let c = "PROCESSOR=local\nPROCESSOR_STYLE=professional\n#PROCESSOR=none\n";
        let u = update_conf(c, "PROCESSOR", "openai");
        assert_eq!(u, "PROCESSOR=openai\nPROCESSOR_STYLE=professional\n#PROCESSOR=none\n");
        assert_eq!(update_conf("A=1\n", "B", "2"), "A=1\nB=2\n");
    }

    #[test]
    fn conf_warnings_catch_typos_and_accept_defaults() {
        assert!(conf_warnings(DEFAULT_CONF).is_empty(), "{:?}", conf_warnings(DEFAULT_CONF));
        let w = conf_warnings("PROCESSOR=olama\nMODE=toogle\nHOTKEY=KEY_NOPE\nFOO=1\nbad line\nSTT_MODEL=small q8\n");
        assert_eq!(w.len(), 6, "{w:?}");
        assert!(w[0].contains("PROCESSOR=olama"));
    }

    #[test]
    fn every_style_has_its_own_prompt_and_unknown_falls_back_to_clean() {
        let clean = style_prompt("clean", "en");
        assert_eq!(style_prompt("", "en"), clean);
        assert_eq!(style_prompt("does-not-exist", "en"), clean);
        let rewriting: Vec<&&str> = STYLES.iter().filter(|s| !matches!(**s, "" | "clean")).collect();
        let mut prompts: Vec<String> = rewriting.iter().map(|s| style_prompt(s, "en")).collect();
        assert!(prompts.iter().all(|p| *p != clean && p.contains("Return ONLY")), "{rewriting:?}");
        prompts.sort();
        prompts.dedup();
        // "soften" and "diplomatic" intentionally share the same prompt.
        assert_eq!(prompts.len(), rewriting.len() - 1);
        assert!(style_prompt("casual", "fr").contains("français"));
    }

    #[test]
    fn instruction_key_is_validated_and_must_differ_from_the_main_key() {
        assert!(conf_warnings("INSTRUCT_HOTKEY=\n").is_empty(), "vide = désactivé");
        assert!(conf_warnings("INSTRUCT_HOTKEY=KEY_F10\n").is_empty());
        assert_eq!(conf_warnings("INSTRUCT_HOTKEY=KEY_BANANA").len(), 1);
        let same = conf_warnings("HOTKEY=KEY_F9\nINSTRUCT_HOTKEY=KEY_F9\n");
        assert!(same.iter().any(|w| w.contains("must differ")), "{same:?}");
        assert!(conf_warnings("HOTKEY=KEY_F9\nINSTRUCT_HOTKEY=KEY_F10\n").is_empty());
    }

    #[test]
    fn parse_key_accepts_known_and_rejects_unknown() {
        assert!(matches!(parse_key(" KEY_F9 ").unwrap(), Key::KEY_F9));
        assert!(parse_key("KEY_BANANA").is_err());
    }

    #[test]
    fn write_private_forces_owner_only_even_on_existing_file() {
        use std::os::unix::fs::PermissionsExt;
        let p = std::env::temp_dir().join(format!("plume-test-{}.conf", std::process::id()));
        std::fs::write(&p, "old").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private(&p, "OPENAI_API_KEY=sk-x\n").unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        let body = std::fs::read_to_string(&p).unwrap();
        let _ = std::fs::remove_file(&p);
        assert_eq!((mode, body.as_str()), (0o600, "OPENAI_API_KEY=sk-x\n"));
    }
}
