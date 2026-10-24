# ADR 0011 — Builds GPU facultatifs, liaison statique, Opus par défaut

[English](../../en/adr/0011-gpu-builds-and-static-linking.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Le binaire CPU fait 14 Mo, le CUDA 604 Mo (cuBLAS statique dans les deux copies de ggml). La règle d'indépendance interdit d'exiger des bibliothèques système à l'exécution. L'encodage Opus (envoi vers OpenAI environ 6 fois plus petit) demandait d'abord `libopus-dev`.

## Décision

- `cuda` et `vulkan` sont des features Cargo **désactivées par défaut** ; un build CPU fonctionne partout.
- Une build CUDA lie ses bibliothèques statiquement : seul le pilote NVIDIA est nécessaire à l'exécution, pas le toolkit.
- `opus` est **activé par défaut** : libopus est compilée depuis les sources et liée statiquement (variables dans `.cargo/config.toml`, `cmake` nécessaire au build seulement).
- `CMAKE_CUDA_ARCHITECTURES=native` ne compile les noyaux que pour la carte présente (environ 100 Mo de moins).

## Conséquences

- Mesure : Whisper `turbo` + Qwen sur une RTX 5080 tiennent en ~680 Mo de RAM et ~2,6 Go de VRAM ; 11 s d'audio sont transcrites en ~115 ms (4,3 s sur CPU).
- Un build CUDA prend environ 11 minutes et Cargo ne remarque pas un changement d'architecture : il faut forcer la recompilation (voir `docs/*/development.md`).
- ~~`libgomp` (OpenMP) reste une bibliothèque dynamique du système, héritée de llama.cpp.~~ **Levé le 2026-10-24 :** un test sur des distributions propres a montré que le binaire ne démarrait pas sans `libgomp`. llama.cpp est maintenant compilé sans OpenMP (`default-features = false`) ; mesuré sur CPU, 3,35 s contre 3,40 s par phrase, donc aucune perte. Le binaire ne dépend plus que de GTK4, ALSA, D-Bus, SQLite et de la libc.
- Un binaire CUDA ne démarre pas sans pilote NVIDIA : d'où le choix de ne pas l'activer par défaut.
