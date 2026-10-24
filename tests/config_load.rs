//! Non-regression: real loading of the config file (HOME isolated in a temporary folder).
use plume::config::{conf_path, set_conf_key, AppConfig, HotkeyMode, OutputMode, ProcessorKind};
use std::os::unix::fs::PermissionsExt;

// A single test: the process environment is shared, so concurrent access is avoided.
#[test]
fn config_defaults_overrides_and_partial_updates() {
    let home = std::env::temp_dir().join(format!("plume-home-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    std::env::set_var("HOME", &home);
    for k in ["OPENAI_API_KEY", "ANTHROPIC_API_KEY", "WISPR_OLLAMA_MODEL"] { std::env::remove_var(k); }

    // 1. First launch: the default file is created, private, and gives the documented values.
    let cfg = AppConfig::load().unwrap();
    assert_eq!(format!("{:?}", cfg.hotkey), "KEY_RIGHTCTRL");
    assert!(matches!(cfg.mode, HotkeyMode::Hold));
    assert!(matches!(cfg.processor, ProcessorKind::Local));
    assert!(matches!(cfg.output, OutputMode::Inject));
    assert_eq!((cfg.stt_model.as_str(), cfg.stt_lang.as_str()), ("small", ""));
    assert!(cfg.openai_key.is_none() && cfg.anthropic_key.is_none());
    assert_eq!(std::fs::metadata(conf_path()).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(home.join(".local/share/plume/vocab.txt").exists(), "le vocabulaire par défaut est créé");
    assert!(!cfg.stt_prompt.is_empty(), "le vocabulaire alimente le prompt Whisper");

    // 2. Custom settings, including those the settings window does not show.
    std::fs::write(conf_path(), "HOTKEY=KEY_F9\nMODE=toggle\nPROCESSOR=none\nOUTPUT=both\nSTT_LANG=fr\n\
        STT_MODEL=small-q8_0\nPROCESSOR_STYLE=formal\nMODEL_PATH=/x/y.gguf\nOPENAI_API_KEY=sk-file\n").unwrap();
    let cfg = AppConfig::load().unwrap();
    assert_eq!(format!("{:?}", cfg.hotkey), "KEY_F9");
    assert!(matches!(cfg.mode, HotkeyMode::Toggle) && matches!(cfg.processor, ProcessorKind::None));
    assert!(matches!(cfg.output, OutputMode::Both));
    assert_eq!((cfg.stt_lang.as_str(), cfg.stt_model.as_str(), cfg.processor_style.as_str()), ("fr", "small-q8_0", "formal"));
    assert_eq!(cfg.model_path.to_str(), Some("/x/y.gguf"));
    assert_eq!(cfg.openai_key.as_deref(), Some("sk-file"));

    // 3. The environment variable wins over the file.
    std::env::set_var("OPENAI_API_KEY", "sk-env");
    assert_eq!(AppConfig::load().unwrap().openai_key.as_deref(), Some("sk-env"));
    std::env::remove_var("OPENAI_API_KEY");

    // 4. Regression: changing PROCESSOR (tray menu) must erase neither PROCESSOR_STYLE nor MODEL_PATH.
    set_conf_key("PROCESSOR", "openai");
    let cfg = AppConfig::load().unwrap();
    assert!(matches!(cfg.processor, ProcessorKind::OpenAi));
    assert_eq!((cfg.processor_style.as_str(), cfg.model_path.to_str()), ("formal", Some("/x/y.gguf")));
    assert_eq!(std::fs::metadata(conf_path()).unwrap().permissions().mode() & 0o777, 0o600);

    // 4b. Instruction mode: off by default, enabled by a second key, refused if it is the same key.
    assert!(cfg.instruct_hotkey.is_none());
    set_conf_key("HOTKEY", "KEY_F9");
    set_conf_key("INSTRUCT_HOTKEY", "KEY_F10");
    assert_eq!(format!("{:?}", AppConfig::load().unwrap().instruct_hotkey), "Some(KEY_F10)");
    set_conf_key("INSTRUCT_HOTKEY", "KEY_F9");
    assert!(AppConfig::load().unwrap().instruct_hotkey.is_none(), "même touche que HOTKEY : désactivé");
    set_conf_key("INSTRUCT_HOTKEY", "");
    assert!(AppConfig::load().unwrap().instruct_hotkey.is_none());

    // 5. An invalid HOTKEY is a clear error, not a silent default.
    set_conf_key("HOTKEY", "KEY_BANANA");
    let err = AppConfig::load().err().expect("hotkey invalide").to_string();
    assert!(err.contains("KEY_BANANA"), "{err}");

    let _ = std::fs::remove_dir_all(&home);
}
