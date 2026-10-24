# Décisions d'architecture (ADR)

[English](../../en/adr/README.md)

Chaque fiche donne le contexte, la décision et ses conséquences. Une fiche acceptée n'est jamais réécrite : une nouvelle la remplace.

| # | Décision |
|---|----------|
| [0001](0001-architecture-hexagonale.md) | Architecture hexagonale (ports et adaptateurs) |
| [0002](0002-whisper-cpp-en-processus.md) | Whisper tourne dans le processus avec whisper.cpp, sans Python |
| [0003](0003-symboles-ggml-prefixes.md) | Préfixer les symboles ggml de whisper pour le lier avec llama.cpp |
| [0004](0004-qwen3-reflexion-activee.md) | Qwen3 garde sa réflexion activée, sans suffixe `/nothink` |
| [0005](0005-correction-par-morceaux.md) | Correction du LLM par morceaux avec un garde-fou contre la perte de mots |
| [0006](0006-dechargement-et-prechauffage.md) | Décharger Whisper à l'inactivité, le recharger à l'appui sur la touche |
| [0007](0007-calcul-hors-boucle-async.md) | Le calcul lourd passe par `spawn_blocking`, jamais dans la boucle async |
| [0008](0008-filtre-hallucinations.md) | Filtrer les hallucinations de Whisper sur le bruit |
| [0009](0009-fichiers-prives.md) | Fichiers privés pour les clés et les dictées |
| [0010](0010-garde-fous-memoire.md) | Garde-fous mémoire et disque contre le manque de mémoire (OOM) |
| [0011](0011-build-gpu-et-liaison-statique.md) | Builds GPU facultatifs, liaison statique, Opus par défaut |
| [0012](0012-redemarrage-sans-shell.md) | Redémarrer sans shell avec `plume --restart` |
| [0013](0013-strategie-de-tests.md) | Quatre niveaux de tests, vrais modèles et vrais bugs |
| [0014](0014-detecteur-de-voix-silero.md) | Détecteur de voix Silero avant Whisper |
| [0015](0015-pas-d-overlay-flottant.md) | Pas d'overlay flottant, de bulle d'état ni de fenêtre « hub » |
| [0016](0016-mode-instruction.md) | Mode instruction : réécrire la sélection avec un modèle en ligne |
| [0017](0017-dependances-historique-et-objectif.md) | Dépendances : historique, état actuel et objectif « prêt à l'emploi » |
