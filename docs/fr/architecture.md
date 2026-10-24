# Architecture

Plume suit une architecture hexagonale ([ADR 0001](adr/0001-architecture-hexagonale.md)) : la logique de dictée ne dépend que de traits (les *ports*), et chaque matériel ou logiciel est un *adaptateur*.

```
src/
  domain/ports/        traits : AudioRecorder, SpeechToText, TextProcessor, TextInjector,
                       HotkeyListener, HistoryStore, Notifier, ScratchpadWriter, ContextReader
  adapters/
    audio/             CpalRecorder          micro, relu à chaque enregistrement
    hotkey/            EvdevListener         claviers ré-énumérés toutes les 3 s
    stt/               WhisperCpp, OpenAiWhisper, audio_util, opus_enc
    processor/         LocalProcessor (llama.cpp), Ollama, OpenAi, Anthropic
    injector/          ClipboardInjector     presse-papiers + Ctrl+V via uinput
    context/           AtspiContext          texte avant le curseur, via l'accessibilité
    history/ scratchpad/ notifier/ http/
  application/
    dictation.rs       DictationService      tout le déroulé ci-dessous
    snippets.rs        hallucination.rs
  config.rs sysmem.rs restart.rs hints.rs tray.rs settings_gtk.rs
```

## Une dictée

1. Le thread du clavier envoie `RecordStart`. Le service démarre l'enregistreur, demande à Whisper de se charger s'il était déchargé (`warm`), et notifie.
2. À `RecordStop` l'audio est arrêté puis contrôlé : trop court → ignoré ; volume presque nul → « Aucun son — le micro est-il coupé ? » et Whisper n'est pas appelé.
3. Whisper (`spawn_blocking`) rend un texte. Les balises « non-parole » (`[BLANK_AUDIO]`) sont retirées. Un texte fait de ponctuation seule et les phrases que Whisper invente sur du bruit sont ignorés ([ADR 0008](adr/0008-filtre-hallucinations.md)).
4. Le texte est corrigé par morceaux d'environ 350 caractères. Un morceau qui revient vide ou plus court de plus de 10 % garde ses mots bruts ([ADR 0005](adr/0005-correction-par-morceaux.md)).
5. Les snippets sont appliqués, le résultat est enregistré dans l'historique, puis tapé (ou écrit dans le bloc-notes).
6. Les noms et sigles trouvés dans le texte sont ajoutés à `vocab.txt`.

Un nouvel appui sur la touche pendant l'étape 3 ou 4 est vu immédiatement et démarre un nouvel enregistrement ([ADR 0007](adr/0007-calcul-hors-boucle-async.md)).

## Mémoire et ressources

- Whisper est chargé au démarrage, déchargé après 5 minutes d'inactivité, et rechargé par `warm()` quand vous appuyez sur la touche, sans attente pour vous ([ADR 0006](adr/0006-dechargement-et-prechauffage.md)).
- Le cache du LLM est dimensionné sur le prompt, pas à 4096 tokens fixes (environ 330 Mo de moins au pic, mesuré).
- L'enregistrement cesse de grandir après 10 minutes. Les modèles sont comparés à la RAM et au disque libres avant chargement ou téléchargement ([ADR 0010](adr/0010-garde-fous-memoire.md)).
- Whisper utilise les trois quarts des cœurs du processeur (de 2 à 8).

## Un binaire, deux copies de ggml

whisper.cpp et llama.cpp embarquent chacun leur propre copie de ggml, de versions différentes, qu'on ne peut pas lier ensemble telles quelles. `vendor/whisper-rs-sys/build.rs` préfixe les symboles de la copie de whisper une fois celle-ci compilée ([ADR 0003](adr/0003-symboles-ggml-prefixes.md)). Ne mettez pas à jour `whisper-rs` ou `llama-cpp-2` sans lancer le test de bout en bout avec de vrais modèles.

## Features Cargo

| Feature | Par défaut | Effet |
|---------|-----------|-------|
| `opus` | oui | La voix envoyée à OpenAI est en Ogg/Opus, environ 6 fois plus petite que le WAV. libopus est compilée depuis les sources et liée statiquement |
| `cuda` | non | GPU NVIDIA pour Whisper et le LLM |
| `vulkan` | non | GPU Vulkan pour les deux |
