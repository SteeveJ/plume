# Plume documentation

| | English | Français |
|---|---|---|
| Overview and install | [README](../README.md) | [README.fr](../README.fr.md) |
| Configuration | [en/configuration](en/configuration.md) | [fr/configuration](fr/configuration.md) |
| Architecture | [en/architecture](en/architecture.md) | [fr/architecture](fr/architecture.md) |
| Development and tests | [en/development](en/development.md) | [fr/development](fr/development.md) |
| Troubleshooting | [en/troubleshooting](en/troubleshooting.md) | [fr/troubleshooting](fr/troubleshooting.md) |

## Architecture decision records (ADR) · Décisions d'architecture

Available in both languages · Disponibles dans les deux langues : [English](en/adr/README.md) | [Français](fr/adr/README.md)

| # | English | Français |
|---|---------|----------|
| 0001 | [Hexagonal architecture (ports and adapters)](en/adr/0001-hexagonal-architecture.md) | [Architecture hexagonale (ports et adaptateurs)](fr/adr/0001-architecture-hexagonale.md) |
| 0002 | [Whisper runs in-process with whisper.cpp, no Python](en/adr/0002-in-process-whisper-cpp.md) | [Whisper tourne dans le processus avec whisper.cpp, sans Python](fr/adr/0002-whisper-cpp-en-processus.md) |
| 0003 | [Prefix whisper's ggml symbols to link it with llama.cpp](en/adr/0003-prefixed-ggml-symbols.md) | [Préfixer les symboles ggml de whisper pour le lier avec llama.cpp](fr/adr/0003-symboles-ggml-prefixes.md) |
| 0004 | [Qwen3 keeps its thinking enabled, no `/nothink` suffix](en/adr/0004-qwen3-thinking-enabled.md) | [Qwen3 garde sa réflexion activée, sans suffixe `/nothink`](fr/adr/0004-qwen3-reflexion-activee.md) |
| 0005 | [LLM correction in chunks with a content-loss guard](en/adr/0005-chunked-correction.md) | [Correction du LLM par morceaux avec un garde-fou contre la perte de mots](fr/adr/0005-correction-par-morceaux.md) |
| 0006 | [Unload Whisper when idle, warm it up on key press](en/adr/0006-unload-and-warm-up.md) | [Décharger Whisper à l'inactivité, le recharger à l'appui sur la touche](fr/adr/0006-dechargement-et-prechauffage.md) |
| 0007 | [Heavy compute runs in `spawn_blocking`, never in the async loop](en/adr/0007-compute-off-the-async-loop.md) | [Le calcul lourd passe par `spawn_blocking`, jamais dans la boucle async](fr/adr/0007-calcul-hors-boucle-async.md) |
| 0008 | [Filter Whisper hallucinations on noise](en/adr/0008-hallucination-filter.md) | [Filtrer les hallucinations de Whisper sur le bruit](fr/adr/0008-filtre-hallucinations.md) |
| 0009 | [Private files for keys and dictations](en/adr/0009-private-files.md) | [Fichiers privés pour les clés et les dictées](fr/adr/0009-fichiers-prives.md) |
| 0010 | [Memory and disk guards against out-of-memory](en/adr/0010-memory-and-disk-guards.md) | [Garde-fous mémoire et disque contre le manque de mémoire (OOM)](fr/adr/0010-garde-fous-memoire.md) |
| 0011 | [Optional GPU builds, static linking, Opus by default](en/adr/0011-gpu-builds-and-static-linking.md) | [Builds GPU facultatifs, liaison statique, Opus par défaut](fr/adr/0011-build-gpu-et-liaison-statique.md) |
| 0012 | [Restart without a shell with `plume --restart`](en/adr/0012-restart-without-a-shell.md) | [Redémarrer sans shell avec `plume --restart`](fr/adr/0012-redemarrage-sans-shell.md) |
| 0013 | [Four test levels, real models and real bugs](en/adr/0013-test-strategy.md) | [Quatre niveaux de tests, vrais modèles et vrais bugs](fr/adr/0013-strategie-de-tests.md) |
| 0014 | [Silero voice detector in front of Whisper](en/adr/0014-silero-voice-detector.md) | [Détecteur de voix Silero avant Whisper](fr/adr/0014-detecteur-de-voix-silero.md) |
| 0015 | [No floating overlay, status bubble or "hub" window](en/adr/0015-no-floating-overlay.md) | [Pas d'overlay flottant, de bulle d'état ni de fenêtre « hub »](fr/adr/0015-pas-d-overlay-flottant.md) |
| 0016 | [Instruction mode: rewrite the selection with an online model](en/adr/0016-instruction-mode.md) | [Mode instruction : réécrire la sélection avec un modèle en ligne](fr/adr/0016-mode-instruction.md) |
| 0017 | [Dependencies: history, current state and the "ready to use" goal](en/adr/0017-dependencies-history-and-goal.md) | [Dépendances : historique, état actuel et objectif « prêt à l'emploi »](fr/adr/0017-dependances-historique-et-objectif.md) |
