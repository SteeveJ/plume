# Plume — démon de dictée vocale (notes pour les agents)

Dépôt : `~/plume` · binaire installé : `~/.local/bin/plume` · config : `~/.config/plume.conf` · données : `~/.local/share/plume/`
Machine de développement : Linux, session **COSMIC (Wayland)**, RTX 5080 16 Go, CUDA dans `/opt/cuda`.

## Règles du projet (à respecter avant tout)

1. **Public visé : tout le monde, pas seulement les développeurs.** L'auteur est développeur, ce n'est pas la cible. Ne jamais écarter une fonctionnalité parce qu'elle serait « peu utile pour un outil de développeur » (exemple : le Feature Tour a été rouvert pour cette raison).
2. **Les commentaires du code sont en anglais** : commentaires `//`, `///`, `//!`, commentaires de tests, de scripts de build (`vendor/whisper-rs-sys/build.rs`), de `Cargo.toml`, de `.cargo/config.toml` et du fichier `src/default_vocab.txt`. Le texte affiché à l'utilisateur n'est pas un commentaire : notifications et fenêtre de réglages passent par `t(en, fr, lang)` et existent en anglais et en français. `agents.md` et `todo.md` restent en français (documents de travail).
3. **Documentation bilingue** : `README.md` (EN) et `README.fr.md` (FR), `docs/en/` et `docs/fr/`, ADR dans les deux langues (même numéro, liens croisés). Toute modification d'un côté se répercute de l'autre.
4. **Application indépendante, prête à l'emploi : zéro dépendance externe à l'exécution.** L'utilisateur installe le binaire et ça marche, sans `apt install` ni `dnf install`. Préférer une crate Rust pure ou une bibliothèque compilée et liée statiquement ; les modèles et assets sont embarqués ou téléchargés par Plume. Une fonctionnalité qui exige une bibliothèque système est une feature Cargo désactivée par défaut, qui dégrade gracieusement. Avant d'ajouter une dépendance : vérifier qu'elle se compile et se lie sans rien d'installé côté utilisateur. **État réel au 2026-10-25 : cette règle n'est PAS encore respectée.** `libgomp` a été supprimée (llama.cpp sans OpenMP), mais le binaire lie encore GTK4 (+ ~100 bibliothèques), libdbus, libasound et libsqlite3 (voir `todo.md`, section « prête à l'emploi »). Ne jamais présenter ces bibliothèques comme acceptables dans la doc ; la matrice de distributions (`ldd_missing`) doit finir par passer sans aucun paquet d'exécution.
5. **Un test par bug réel**, et on vérifie qu'il échoue quand on réintroduit le bug (mutation à la main). Chaque chiffre de la doc vient d'une mesure ; ne pas en inventer, les refaire si le matériel ou les modèles changent.
6. **Aucune erreur de micro, de modèle ou de réseau n'arrête le démon** : notification avec la cause, puis on attend la touche suivante.

## Pile technique
Rust (édition 2021) : tokio, cpal, evdev, arboard (presse-papiers et sélection primaire), ksni (icône), llama-cpp-2 (LLM local), **whisper-rs** (Whisper en processus, copie vendorisée de `whisper-rs-sys`), rusqlite, hound, reqwest, atspi (contexte), opus + ogg (envoi OpenAI), sha2/hex, notify-rust, gtk4 (réglages), libc, mockall (tests).

## Architecture (hexagonale, voir ADR 0001)
```
src/
  main.rs                    démarrage ; --install, --settings, --download-model, --restart ; notifie les échecs de démarrage
  config.rs                  AppConfig, parse/valide plume.conf, STYLES, INSTRUCT_PROMPT, write_private, update_conf
  sysmem.rs  restart.rs  hints.rs  tray.rs  settings_gtk.rs
  domain/model.rs            AudioSamples, Transcription{text, speech_confirmed}, HotkeyEvent (Record*, Instruct*)
  domain/ports/              AudioRecorder, SpeechToText (+warm), TextProcessor, TextInjector, HotkeyListener,
                             HistoryStore, Notifier, ScratchpadWriter, ContextReader, SelectionReader
  adapters/audio/            CpalRecorder (micro relu à chaque enregistrement, plafond 10 min)
  adapters/hotkey/           EvdevListener (ré-énumère les claviers toutes les 3 s ; touche de dictée + touche d'instruction)
  adapters/injector/         ClipboardInjector (presse-papiers + Ctrl+V via uinput)
  adapters/selection/        PrimarySelection (texte sélectionné, sans rien taper)
  adapters/stt/              WhisperCpp (+ détecteur de voix Silero), OpenAiWhisper, audio_util, opus_enc, shared_vocab
  adapters/processor/        LocalProcessor (llama.cpp), Ollama, OpenAi, Anthropic
  adapters/history/ scratchpad/ notifier/ context/ http/
  application/dictation.rs   DictationService : toute la chaîne ci-dessous
  application/snippets.rs  hallucination.rs
tests/                       config_load, pipeline, hallucinations, french, instruct, e2e_models, e2e_french, e2e_llm_eval, common/, fixtures/
vendor/whisper-rs-sys/       copie patchée (ADR 0003)
docs/                        en/, fr/, adr/ dans chaque langue
```

## Chaîne de dictée (`DictationService`)
1. Touche : `RecordStart/Stop` (maintien ou bascule) ou `InstructStart/Stop` (maintien seulement).
2. `hear()` : arrêt du micro, trop court → ignoré, volume quasi nul → « Aucun son » sans appeler Whisper ; détecteur de voix → Whisper (`spawn_blocking`) → balises `[BLANK_AUDIO]` retirées → filtre d'hallucinations.
3. Dictée : correction par morceaux de ~350 caractères ; **un morceau vide, ou dont un seul mot change (`application/fidelity.rs`, accents, majuscules et ponctuation exclus), garde le texte brut** (ADR 0005) → snippets → historique → injection (ou bloc-notes).
4. Instruction (ADR 0016) : sélection lue avant l'enregistrement, modèle en ligne réécrit, résultat collé par-dessus ; réponse vide ou erreur = rien n'est collé.
5. Une nouvelle pression de touche pendant 2–4 est vue tout de suite (`select!` en mode `biased`, calcul dans `spawn_blocking`, ADR 0007).

## Mémoire, CPU, OOM (ADR 0006, 0010)
Whisper est déchargé après 5 min d'inactivité et rechargé par `SpeechToText::warm()` à l'appui sur la touche. `sysmem::model_fits` avant de charger (facteur mesuré 1,9 × la taille du fichier + 256 Mo sur CPU ; 0,6 dans un build GPU où les poids sont en VRAM ; la RAM disponible tient compte des limites cgroup d'un conteneur ou d'un service systemd, pas seulement de `/proc/meminfo`), espace disque avant de télécharger, enregistrement plafonné à 10 min, cache du LLM dimensionné sur le prompt, Whisper limité aux trois quarts des cœurs.

## Un seul binaire : whisper.cpp + llama.cpp (ADR 0003)
Chacun embarque son ggml ; `vendor/whisper-rs-sys/build.rs` préfixe (`pw_`) tous les symboles de celui de whisper, locaux compris (groupes COMDAT), sauf `whisper_*` et la lib standard. Le build exige `nm` et `objcopy`. **Ne pas mettre à jour `whisper-rs` ou `llama-cpp-2` sans lancer `e2e_models`** : un conflit ne plante qu'à l'exécution.

## Qwen3 et le LLM local (ADR 0004, 0005)
Réflexion activée, aucun suffixe `/nothink` (Qwen3 ne le connaît pas, le vrai est `/no_think`). Le bloc `<think>` est retiré de la sortie. Le 0,6B garde la place : le 1,7B a été essayé et écarté (`tests/e2e_llm_eval.rs` le note).

## Hallucinations de Whisper (ADR 0008, 0014)
Un détecteur de voix Silero (`ggml-silero-v5.1.2.bin`, `WhisperCpp::with_vad`) écarte le bruit avant Whisper et confirme la parole (`speech_confirmed`) ; sans lui, `application/hallucination.rs` filtre par volume et par liste `INVENTED`. Toute nouvelle phrase inventée s'ajoute à `INVENTED` avec son cas dans `tests/hallucinations.rs`.

## Sécurité
`SECURITY.md` et `SECURITY.fr.md` (racine) : politique de signalement ; la section « Security and privacy » des README dit ce qui reste en local, ce qui sort, les points faibles connus et ce qui n'est pas vérifié. Toute nouvelle donnée envoyée à un service en ligne, ou tout nouveau droit demandé à l'utilisateur, s'y ajoute dans les deux langues.

## Fichiers privés (ADR 0009)
Config, historique, bloc-notes : mode `0600` via `config::write_private` ; dossier de données `0700`.

## Commandes
```bash
cargo build --release                                              # CPU (14 Mo)
CUDA_PATH=/opt/cuda CMAKE_CUDA_ARCHITECTURES=native \
  cargo build --release --features cuda                            # GPU NVIDIA (604 Mo, ~11 min la première fois)
pkill -x plume; ./target/release/plume --install; ~/.local/bin/plume --restart   # installer et relancer
plume --restart | --settings | --download-model                    # outils
RUST_LOG=debug ~/.local/bin/plume                                   # journal complet
```
Cargo ne voit pas un changement de `CMAKE_CUDA_ARCHITECTURES` : pour le forcer, `cargo clean -p llama-cpp-sys-2 --release` et supprimer `target/release/build/whisper-rs-sys-*`.

## Tests, du plus rapide au plus complet
- `cargo test --lib` : unitaires, sans modèle ni matériel.
- `cargo test --test config_load --test pipeline --test hallucinations --test french --test instruct` : vrais adaptateurs, faux micro/clavier/Whisper/modèle en ligne.
- `PLUME_TEST_GGML=… PLUME_TEST_GGUF=… [PLUME_TEST_VAD=…] cargo test --release --test e2e_models --test e2e_french -- --ignored` (+ `--features cuda`) : vrais modèles. Les tests qui chargent le LLM tournent l'un après l'autre (llama.cpp ne s'initialise qu'une fois par processus).
- `PLUME_TEST_GGUF=… cargo test --release --test e2e_llm_eval -- --ignored --nocapture` : note un LLM de correction ; à relancer avant de changer de modèle ou de prompt.
- `PLUME_TEST_GGML=… cargo test --release --lib -- --ignored` : la RAM baisse bien au déchargement.
- `cargo clippy --all-targets` doit rester propre (hors `tray.rs` et `bench.rs`, anciens).

## Documentation et décisions
- Toute décision qui change l'architecture, le build ou le comportement de Whisper/du LLM s'écrit en **ADR** dans `docs/fr/adr/` **et** `docs/en/adr/` (16 à ce jour), avec les index. Ne jamais réécrire un ADR accepté : en ajouter un qui le complète ou le remplace, ou y ajouter une note datée.
- Les guides (`docs/en`, `docs/fr`) : configuration, architecture, développement, dépannage.
- Fixtures de test : `tests/fixtures/jfk.wav` et `tests/fixtures/fr/*.wav` (voix Piper `fr_FR-siwis-medium`, textes de référence dans `tests/common/mod.rs`).

## Tester sur d'autres distributions (conteneurs, pas VirtualBox)
`MODELS_DIR=… scripts/distro-matrix/run.sh` : étape 1 (compilation, sans limite) puis étape 2 (utilisation dans un conteneur propre avec **seulement les bibliothèques d'exécution**, limité à **2 Go de RAM** ; cas GPU avec plafond de **4 Go de VRAM** appliqué par `vram-watch.sh` car Docker ne sait pas plafonner la VRAM, le pilote NVIDIA étant monté à la main, sans `nvidia-container-toolkit`). **Consigne de l'utilisateur : lancer les tests d'utilisation trois distributions à la fois** (`MAX_PARALLEL=3` reste à ajouter au script). Les crates vendorisées sont `whisper-rs-sys` (symboles ggml) et `audiopus_sys` (cherche aussi `lib64`, pour Fedora/RHEL/openSUSE). Ce que les conteneurs ne testent pas : icône de la barre, frappe via `uinput`, micro, AT-SPI. Résultats et suites : `todo.md`.

## Décisions écartées (ne pas les reprendre sans raison nouvelle)
Python pour Whisper · second exécutable `plume-whisper` · mode commande vocal (risque d'effacer du texte) · Ctrl+C simulé pour lire la sélection (SIGINT dans un terminal) · overlay, bulle d'état et fenêtre « hub » (ADR 0015 ; **à reconsidérer sous COSMIC**, qui gère `layer-shell`) · Qwen3-1.7B · VirtualBox pour la matrice de distributions (`sudo` avec mot de passe, modules noyau ; les conteneurs suffisent pour compiler et utiliser).

## Piège connu : groupe `input` et systemd --user
Le démon `systemd --user` ne reçoit pas le groupe `input`. `--install` écrit donc `~/.config/autostart/plume.desktop` (XDG autostart, groupes complets) ; le service systemd est secondaire. Pour tester sans reconnexion : lancer `~/.local/bin/plume` depuis un terminal.
