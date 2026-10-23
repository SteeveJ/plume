use crate::config::{AppConfig, APP_NAME, HotkeyMode, OutputMode, ProcessorKind, UiLang};

fn tl(en: &'static str, fr: &'static str, lang: UiLang) -> &'static str {
    match lang { UiLang::Fr => fr, _ => en }
}
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GBox, Button, ComboBoxText,
    Entry, Label, Orientation, PasswordEntry, ScrolledWindow, Separator,
};

pub fn run_settings_window(cfg: AppConfig) {
    let app = Application::builder()
        .application_id("io.plume.settings")
        .build();

    app.connect_activate(move |app| {
        let lang = cfg.ui_lang;
        let t = move |en: &'static str, fr: &'static str| tl(en, fr, lang);
        let win = ApplicationWindow::builder()
            .application(app)
            .title(t("Plume — Settings", "Plume — Paramètres"))
            .default_width(460)
            .default_height(620)
            .resizable(true)
            .build();

        let scroll = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .build();
        win.set_child(Some(&scroll));

        let root = GBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_top(16)
            .margin_bottom(16)
            .margin_start(16)
            .margin_end(16)
            .build();
        scroll.set_child(Some(&root));

        macro_rules! row {
            ($label:expr, $widget:expr) => {{
                let r = GBox::builder().orientation(Orientation::Horizontal).spacing(12).build();
                let l = Label::builder()
                    .label($label)
                    .width_chars(22)
                    .xalign(1.0)
                    .build();
                r.append(&l);
                r.append(&$widget);
                root.append(&r);
            }};
        }

        macro_rules! section {
            ($title:expr) => {{
                let sep = Separator::new(Orientation::Horizontal);
                sep.set_margin_top(10);
                sep.set_margin_bottom(2);
                root.append(&sep);
                let l = Label::builder()
                    .label(format!("<b>{}</b>", $title).as_str())
                    .use_markup(true)
                    .xalign(0.0)
                    .margin_bottom(4)
                    .build();
                root.append(&l);
            }};
        }

        // ── Recording ────────────────────────────────────────────────
        section!(t("Recording", "Enregistrement"));

        let hotkey_cb = ComboBoxText::new();
        hotkey_cb.append(Some("KEY_RIGHTCTRL"),  "Right Ctrl");
        hotkey_cb.append(Some("KEY_LEFTCTRL"),   "Left Ctrl");
        hotkey_cb.append(Some("KEY_RIGHTALT"),   "Right Alt");
        hotkey_cb.append(Some("KEY_LEFTALT"),    "Left Alt");
        hotkey_cb.append(Some("KEY_RIGHTMETA"),  "Right Meta (Super)");
        hotkey_cb.append(Some("KEY_CAPSLOCK"),   "Caps Lock");
        hotkey_cb.append(Some("KEY_SCROLLLOCK"), "Scroll Lock");
        hotkey_cb.append(Some("KEY_PAUSE"),      "Pause");
        hotkey_cb.append(Some("KEY_INSERT"),     "Insert");
        hotkey_cb.append(Some("KEY_F1"),  "F1");  hotkey_cb.append(Some("KEY_F2"),  "F2");
        hotkey_cb.append(Some("KEY_F3"),  "F3");  hotkey_cb.append(Some("KEY_F4"),  "F4");
        hotkey_cb.append(Some("KEY_F5"),  "F5");  hotkey_cb.append(Some("KEY_F6"),  "F6");
        hotkey_cb.append(Some("KEY_F7"),  "F7");  hotkey_cb.append(Some("KEY_F8"),  "F8");
        hotkey_cb.append(Some("KEY_F9"),  "F9");  hotkey_cb.append(Some("KEY_F10"), "F10");
        hotkey_cb.append(Some("KEY_F11"), "F11"); hotkey_cb.append(Some("KEY_F12"), "F12");
        hotkey_cb.set_active_id(Some(format!("{:?}", cfg.hotkey).as_str()));
        row!(t("Hotkey", "Touche"), hotkey_cb);

        let mode_cb = ComboBoxText::new();
        mode_cb.append(Some("hold"),   t("Hold — record while key is held", "Maintien — enregistre tant que la touche est tenue"));
        mode_cb.append(Some("toggle"), t("Toggle — press once to start, again to stop", "Bascule — une pression pour démarrer, une pour arrêter"));
        mode_cb.set_active_id(Some(match cfg.mode { HotkeyMode::Hold => "hold", HotkeyMode::Toggle => "toggle" }));
        row!(t("Mode", "Mode"), mode_cb);

        let out_cb = ComboBoxText::new();
        out_cb.append(Some("inject"),     t("Inject — paste in focused window", "Injecter — coller dans la fenêtre active"));
        out_cb.append(Some("scratchpad"), t("Scratchpad — append to file + clipboard", "Bloc-notes — ajouter au fichier + presse-papier"));
        out_cb.append(Some("both"),       t("Both", "Les deux"));
        out_cb.set_active_id(Some(match cfg.output {
            OutputMode::Inject     => "inject",
            OutputMode::Scratchpad => "scratchpad",
            OutputMode::Both       => "both",
        }));
        row!(t("Output", "Sortie"), out_cb);

        // ── Speech-to-text ───────────────────────────────────────────
        section!(t("Speech Recognition", "Reconnaissance vocale"));

        let stt_cb = ComboBoxText::new();
        stt_cb.append(Some(""),   "Auto-detect");
        stt_cb.append(Some("fr"), "Français");
        stt_cb.append(Some("en"), "English");
        stt_cb.append(Some("es"), "Español");
        stt_cb.append(Some("de"), "Deutsch");
        stt_cb.append(Some("it"), "Italiano");
        stt_cb.append(Some("pt"), "Português");
        stt_cb.append(Some("nl"), "Nederlands");
        stt_cb.append(Some("ja"), "日本語");
        stt_cb.append(Some("zh"), "中文");
        stt_cb.set_active_id(Some(if cfg.stt_lang.is_empty() { "" } else { cfg.stt_lang.as_str() }));
        row!(t("Language spoken", "Langue parlée"), stt_cb);

        let stt_model_cb = ComboBoxText::new();
        stt_model_cb.append(Some("tiny"),    t("Tiny — fastest, rough accuracy", "Tiny — très rapide, qualité approximative"));
        stt_model_cb.append(Some("base"),    t("Base — fast", "Base — rapide"));
        stt_model_cb.append(Some("small"),   t("Small — recommended (GPU: ~0.3s)", "Small — recommandé (GPU: ~0.3s)"));
        stt_model_cb.append(Some("medium"),  t("Medium — accurate", "Medium — précis"));
        stt_model_cb.append(Some("large-v2"),t("Large v2 — best accuracy, slow on CPU", "Large v2 — meilleure précision, lent sans GPU"));
        stt_model_cb.append(Some("turbo"),   t("Turbo — fast + accurate", "Turbo — rapide et précis"));
        stt_model_cb.set_active_id(Some(cfg.stt_model.as_str()));
        row!(t("Whisper model", "Modèle Whisper"), stt_model_cb);

        let stt_prompt_e = Entry::builder()
            .text(cfg.stt_prompt.as_str())
            .hexpand(true)
            .placeholder_text(t("Yves, ChatGPT, proper nouns…", "Yves, ChatGPT, noms propres…"))
            .build();
        row!(t("Vocabulary hints", "Vocabulaire personnalisé"), stt_prompt_e);

        // ── Text processor ───────────────────────────────────────────
        section!(t("Text Processor", "Traitement du texte"));

        let proc_cb = ComboBoxText::new();
        proc_cb.append(Some("local"),     t("Local — Qwen3 offline (~390 MB)", "Local — Qwen3 hors-ligne (~390 Mo)"));
        proc_cb.append(Some("openai"),    "OpenAI");
        proc_cb.append(Some("anthropic"), "Anthropic");
        proc_cb.append(Some("ollama"),    t("Ollama (local server)", "Ollama (serveur local)"));
        proc_cb.append(Some("none"),      t("None — raw transcription", "Aucun — transcription brute"));
        proc_cb.set_active_id(Some(match &cfg.processor {
            ProcessorKind::Local     => "local",
            ProcessorKind::OpenAi    => "openai",
            ProcessorKind::Anthropic => "anthropic",
            ProcessorKind::Ollama    => "ollama",
            ProcessorKind::None      => "none",
        }));
        row!(t("Processor", "Processeur"), proc_cb);

        let style_cb = ComboBoxText::new();
        style_cb.append(Some("clean"),        t("🔓 Clean — fix errors only", "🔓 Correction — erreurs uniquement"));
        style_cb.append(Some("professional"), t("🔒 Professional tone (needs API key)", "🔒 Professionnel (nécessite une clé API)"));
        style_cb.append(Some("diplomatic"),   t("🔒 Diplomatic / Soften (needs API key)", "🔒 Diplomatique / Adouci (nécessite une clé API)"));
        style_cb.append(Some("assertive"),    t("🔒 Assertive (needs API key)", "🔒 Assertif (nécessite une clé API)"));
        style_cb.append(Some("humor"),        t("🔒 Humor (needs API key)", "🔒 Humour (nécessite une clé API)"));
        style_cb.set_active_id(Some(if cfg.processor_style.is_empty() { "clean" } else { cfg.processor_style.as_str() }));

        let style_cb_clone = style_cb.clone();
        proc_cb.connect_changed(move |cb| {
            let local = cb.active_id().as_deref() == Some("local");
            // hint: switch to clean when local selected; user can override
            if local { style_cb_clone.set_active_id(Some("clean")); }
        });

        row!(t("Style", "Style"), style_cb);

        let ollama_e = Entry::builder()
            .text(cfg.ollama_model.as_str())
            .hexpand(true)
            .build();
        row!(t("Ollama model", "Modèle Ollama"), ollama_e);

        // ── API keys ─────────────────────────────────────────────────
        section!(t("API Keys", "Clés API"));

        let openai_e = PasswordEntry::builder()
            .show_peek_icon(true)
            .hexpand(true)
            .placeholder_text("sk-…")
            .build();
        if let Some(k) = &cfg.openai_key { openai_e.set_text(k); }
        row!(t("OpenAI key", "Clé OpenAI"), openai_e);

        let anthropic_e = PasswordEntry::builder()
            .show_peek_icon(true)
            .hexpand(true)
            .placeholder_text("sk-ant-…")
            .build();
        if let Some(k) = &cfg.anthropic_key { anthropic_e.set_text(k); }
        row!(t("Anthropic key", "Clé Anthropic"), anthropic_e);

        // ── App ──────────────────────────────────────────────────────
        section!(t("App", "Application"));

        let lang_cb = ComboBoxText::new();
        lang_cb.append(Some("en"), "English");
        lang_cb.append(Some("fr"), "Français");
        lang_cb.set_active_id(Some(match cfg.ui_lang { UiLang::Fr => "fr", _ => "en" }));
        row!(t("App language", "Langue de l'interface"), lang_cb);

        // Vocabulary hints — open in editor button
        let vocab_path = crate::config::data_dir().join("vocab.txt");
        let open_vocab_btn = Button::builder()
            .label(t("📝 Open vocabulary file", "📝 Ouvrir le fichier de vocabulaire"))
            .hexpand(true)
            .build();
        {
            let vp = vocab_path.clone();
            open_vocab_btn.connect_clicked(move |_| {
                let _ = std::process::Command::new("xdg-open").arg(&vp).spawn();
            });
        }
        root.append(&open_vocab_btn);

        // ── Save ─────────────────────────────────────────────────────
        let save_btn = Button::builder()
            .label(t("Save & Restart", "Sauvegarder et relancer"))
            .margin_top(8)
            .build();
        save_btn.add_css_class("suggested-action");
        root.append(&save_btn);

        let win_clone = win.clone();
        save_btn.connect_clicked(move |_| {
            let mut lines: Vec<String> = Vec::new();

            if let Some(v) = hotkey_cb.active_id()     { lines.push(format!("HOTKEY={v}")); }
            if let Some(v) = mode_cb.active_id()       { lines.push(format!("MODE={v}")); }
            if let Some(v) = out_cb.active_id()        { lines.push(format!("OUTPUT={v}")); }
            if let Some(v) = stt_cb.active_id()        { lines.push(format!("STT_LANG={v}")); }
            if let Some(v) = stt_model_cb.active_id()  { lines.push(format!("STT_MODEL={v}")); }
            let sp = stt_prompt_e.text();
            lines.push(format!("STT_PROMPT={sp}"));
            if let Some(v) = proc_cb.active_id()       { lines.push(format!("PROCESSOR={v}")); }
            if let Some(v) = style_cb.active_id()      { lines.push(format!("PROCESSOR_STYLE={v}")); }
            if let Some(v) = lang_cb.active_id()       { lines.push(format!("APP_LANG={v}")); }

            let ok = openai_e.text();
            if !ok.is_empty() { lines.push(format!("OPENAI_API_KEY={ok}")); }
            let ak = anthropic_e.text();
            if !ak.is_empty() { lines.push(format!("ANTHROPIC_API_KEY={ak}")); }
            let om = ollama_e.text();
            if !om.is_empty() { lines.push(format!("OLLAMA_MODEL={om}")); }

            let home = std::env::var("HOME").unwrap_or_default();
            let path = std::path::PathBuf::from(&home)
                .join(".config")
                .join(format!("{APP_NAME}.conf"));
            if std::fs::write(&path, lines.join("\n") + "\n").is_ok() {
                let home = std::env::var("HOME").unwrap_or_default();
                let bin = std::path::PathBuf::from(&home).join(".local/bin").join(APP_NAME);
                // Shell waits until all instances are dead, then relaunches.
                // Spawn it first — pkill will kill this process too.
                let cmd = format!(
                    "sleep 0.6 && pkill -x '{app}'; sleep 0.3 && exec '{bin}' &",
                    app = APP_NAME,
                    bin = bin.display(),
                );
                let _ = std::process::Command::new("sh").args(["-c", &cmd]).spawn();
            }
            win_clone.close();
        });

        win.present();
    });

    app.run_with_args::<String>(&[]);
}
