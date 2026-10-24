# Plume — daemon de dictée vocale

Répertoire : `~/plume`  
Binary installé : `~/.local/bin/plume`  
Config : `~/.config/plume.conf`  
Data dir : `~/.local/share/plume/` (model.gguf, history.db, scratch.txt)

## Stack Cargo
tokio, cpal, evdev=0.12, arboard=3.7, ksni=0.2, llama-cpp-2=0.1, rusqlite=0.31,
hound=3, reqwest=0.12, sha2, hex, notify-rust=4 (d feature),
tracing-subscriber (env-filter), gtk4=0.9, mockall=0.13 (dev)

## Architecture hexagonale
```
src/
  main.rs                  ← bootstrap, --install, --settings, --download-model
  config.rs                ← APP_NAME="plume", AppConfig, HotkeyMode, OutputMode, ProcessorKind
  tray.rs                  ← WisprTray { recording, mode_toggle, processor_label }
  settings_gtk.rs          ← fenêtre GTK4 native (plume --settings)
  domain/model.rs          ← AudioSamples, Transcription, HotkeyEvent
  domain/ports/            ← traits: AudioRecorder, SpeechToText, TextInjector,
                              HotkeyListener, TextProcessor, HistoryStore, Notifier,
                              ScratchpadWriter  (tous #[automock])
  adapters/audio/          ← CpalRecorder
  adapters/hotkey/         ← RdevListener (evdev, hold+toggle)
  adapters/injector/       ← EnigoInjector (arboard clipboard + uinput Ctrl+V)
  adapters/stt/            ← OpenAiWhisper, WhisperCpp (whisper.cpp en processus, modèle ggml téléchargé au 1er lancement)
  adapters/processor/      ← LocalProcessor (llama-cpp-2), OllamaProcessor,
                              OpenAiProcessor, AnthropicProcessor
  adapters/history/        ← SqliteHistory
  adapters/notifier/       ← NotifySend (notify-rust D-Bus)
  adapters/scratchpad/     ← FileScratchpad
  application/dictation.rs ← DictationService<R,S,I>
  application/snippets.rs  ← expansion de phrases (snippets.txt `phrase => texte`)
  adapters/stt/audio_util.rs ← mono 16 kHz + WAV 16-bit partagés, opus_enc.rs ← Ogg/Opus (feature `opus`, défaut)
```

## Comportement clé — DictationService.run()
- Bridge std::sync::mpsc → tokio unbounded channel (pour pouvoir select!)
- Hold: RecordStart en appui, RecordStop au relâché
- Toggle: RecordStart/Stop à chaque appui
- **Cancel/restart** : si RecordStart arrive pendant la transcription (tokio::select!),
  la transcription est annulée et l'enregistrement redémarre immédiatement
- OutputMode::Inject → uinput Ctrl+V
- OutputMode::Scratchpad → écrit scratch.txt + clipboard
- OutputMode::Both → les deux

## Processeur local
- Modèle : `unsloth/Qwen3-0.6B-GGUF` Q4_K_M (~390 MB), auto-téléchargé au premier lancement
- Prompt : `/nothink` pour désactiver le chain-of-thought Qwen3
- Strip automatique du bloc `<think>…</think>` en sortie (au cas où)
- Tâche : corriger ponctuation/capitalisation/erreurs STT

## Tray (ksni)
- Clic droit → menu : statut, toggle mode, processor submenu, Settings…, Open scratchpad…, Reload, Quit
- Settings… → lance `plume --settings` (fenêtre GTK4 native)
- Reload → `systemctl --user restart plume`

## Hotkey par défaut
- `KEY_RIGHTCTRL` (hold), configurable dans plume.conf

## Logging
- `tracing_subscriber` avec filtre `info,arboard=error,winit=error`
- Override : `RUST_LOG=debug ~/.local/bin/plume`

## Problème connu : groupe `input` dans systemd --user
Le daemon `systemd --user` ne reçoit pas le groupe `input` (démarré avant l'ajout au groupe,
ou PAM ne passe pas les groupes supplémentaires). `SupplementaryGroups=input` refusé dans user services.

**Fix appliqué** :
- `--install` écrit `~/.config/autostart/plume.desktop` (XDG autostart via session GNOME, a tous les groupes)
- Systemd service toujours écrit mais secondaire
- Pour tester sans re-login : lancer `~/.local/bin/plume` depuis un terminal (terminal a le groupe `input`)

## Commandes
```bash
# Build release
cd ~/plume && cargo build --release --bin plume

# Install (autostart + icons + service file)
./target/release/plume --install

# Run maintenant (terminal = groupe input OK)
~/.local/bin/plume

# Settings UI
~/.local/bin/plume --settings

# Télécharger le modèle manuellement
~/.local/bin/plume --download-model

# Logs (si service actif)
journalctl --user -u plume -f

# Tests unitaires
cd ~/plume && cargo test
```

## Config plume.conf
```
HOTKEY=KEY_RIGHTCTRL
MODE=hold          # ou toggle
PROCESSOR=local    # local | openai | anthropic | ollama | none
OUTPUT=inject      # inject | scratchpad | both
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
OLLAMA_MODEL=qwen3:0.6b
```

## Why: décisions non-évidentes
- arboard + uinput (pas wl-copy/xdotool) : zéro dépendances binaires runtime
- notify-rust D-Bus (pas notify-send) : fonctionne sur toutes les distros
- XDG autostart > systemd user service : héritage des groupes POSIX complet
- /nothink dans le prompt : désactive le CoT Qwen3, réduit latence ~60%
- select! cancel/restart : drop du future HTTP annule proprement la requête Whisper

## Règle : app indépendante, zéro dépendance externe
- Plume doit rester autonome : aucun binaire, service ni bibliothèque système à installer pour qu'une feature fonctionne (pas de wl-copy, xdotool, notify-send, ffmpeg, libopus-dev, etc.).
- Préférer une crate Rust pure ou une lib compilée et liée statiquement au build ; les assets (modèles, icônes) sont embarqués ou téléchargés par Plume lui-même.
- Toute feature qui exige une lib système doit être optionnelle (feature Cargo, off par défaut), dégrader gracieusement et ne jamais casser le build par défaut.
- Avant d'ajouter une dépendance : vérifier qu'elle se compile et se lie sans rien d'installé côté utilisateur.

## Un seul binaire : whisper.cpp + llama.cpp
- whisper.cpp et llama.cpp embarquent chacun leur ggml (versions différentes) : liés ensemble, ils entrent en conflit (symboles dupliqués, groupes COMDAT C++).
- `vendor/whisper-rs-sys` est une copie de `whisper-rs-sys` dont le `build.rs` préfixe (`pw_`) tous les symboles des archives whisper/ggml après le build cmake (sauf `whisper_*` et la lib standard C++). Branchée via `[patch.crates-io]`.
- Build : nécessite `nm` et `objcopy` (binutils). Ne pas mettre à jour `whisper-rs`/`llama-cpp-2` sans relancer un test de bout en bout (un STT puis une génération LLM dans le même processus).

## Mémoire
- Whisper (`WhisperCpp`) est déchargé après 5 min sans dictée et rechargé à l'appui sur la touche (`SpeechToText::warm`) ; `STT_MODEL=small-q8_0` etc. sélectionne un modèle ggml quantifié.
- Le LLM local dimensionne son KV cache sur le prompt (+1024 tokens de génération) ; les textes sont corrigés par morceaux avec un garde-fou contre la perte de mots (`DictationService::run_processor`).

## Robustesse (règles à garder)
- Les erreurs de micro, de modèle ou de réseau ne doivent jamais arrêter le démon : notification avec la cause, puis on attend la touche suivante.
- Tout fichier qui contient des clés ou du texte dicté est créé en 0600 (`config::write_private`) ; le dossier de données en 0700.
- Avant de charger un modèle : `sysmem::model_fits` (RAM) ; avant de le télécharger : espace disque. Facteur mesuré pour Whisper `small` : RAM ≈ 1,9 × taille du fichier.
- Le calcul lourd (Whisper, LLM) passe par `spawn_blocking` ; jamais `block_in_place` dans la boucle de dictée.
- Tests, du plus rapide au plus complet :
  - `cargo test --lib` : unitaires, sans modèle ni matériel.
  - `cargo test --test config_load --test pipeline --test hallucinations` : non-régression avec les vrais adaptateurs (fichier de config réel dans un HOME temporaire ; chaîne parole → LLM → snippets → historique → clavier avec faux micro/clavier). Chaque test correspond à un bug réel déjà rencontré.
  - `PLUME_TEST_GGML=… PLUME_TEST_GGUF=… cargo test --release --test e2e_models -- --ignored` (+ `--features cuda`) : vrais modèles, Whisper et Qwen dans le même processus. Seul test qui attrape un conflit de symboles ggml à l'exécution.
  - `PLUME_TEST_GGML=… cargo test --release --lib -- --ignored` : libération de la RAM au déchargement de Whisper.
- `plume --restart` arrête l'ancien démon puis relance le programme courant (utilisé par le tray et les réglages).
- Hallucinations : toute phrase que Whisper invente sur du bruit doit être ajoutée à `INVENTED` (`application/hallucination.rs`) avec le cas correspondant dans `tests/hallucinations.rs`. Le test `e2e_models` rejoue du bruit réel et montre les nouvelles phrases après un changement de modèle.

## Documentation
- Utilisateur et développeur : `README.md` (EN), `README.fr.md` (FR), `docs/en/` et `docs/fr/` (configuration, architecture, développement, dépannage).
- Décisions d'architecture : `docs/fr/adr/` et `docs/en/adr/` (13 ADR dans chaque langue, même numéro, liens croisés ; index dans chaque dossier et dans `docs/README.md`). Toute décision qui change l'architecture, le build ou le comportement du LLM/Whisper s'écrit en ADR ; ne jamais réécrire un ADR accepté, en ajouter un qui le remplace.
- Chaque chiffre de la doc vient d'une mesure faite sur la RTX 5080 de développement ; les refaire si le matériel ou les modèles changent.
- Tests de compréhension du français : `tests/e2e_french.rs` + `tests/fixtures/fr/*.wav` (voix Piper `fr_FR-siwis-medium`, textes de référence dans `tests/common/mod.rs`).
