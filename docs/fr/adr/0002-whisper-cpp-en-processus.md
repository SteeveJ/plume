# ADR 0002 — Whisper tourne dans le processus avec whisper.cpp, sans Python

[English](../../en/adr/0002-in-process-whisper-cpp.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

La première version lançait un serveur Python persistant (`openai-whisper` + `torch`). Il occupait 1 à 2 Go de RAM en permanence, exigeait Python et `pip` chez l'utilisateur, et pouvait mourir (tué par le noyau en cas de manque de mémoire) sans que Plume s'en aperçoive : toutes les dictées suivantes échouaient. Cela contredit la règle d'indépendance : aucune dépendance externe à l'exécution.

## Décision

Whisper tourne dans le processus Plume via `whisper-rs` (whisper.cpp). Le modèle `ggml` correspondant à `STT_MODEL` est téléchargé au premier lancement, après une notification. Si le modèle ne peut pas être chargé ou téléchargé, Plume bascule sur l'API OpenAI. Le serveur Python, `whisper_server.py` et `requirements.txt` sont supprimés.

## Conséquences

- Plus de Python, plus de processus à surveiller ; le modèle peut être déchargé et rechargé (ADR 0006).
- GPU par les builds CUDA ou Vulkan (ADR 0011).
- Il faut télécharger un fichier de modèle (`small` 470 Mo, `turbo` 1,6 Go).
- whisper.cpp et llama.cpp partagent le processus, d'où l'ADR 0003.
- **Écartés** : garder Python ; un second exécutable `plume-whisper` (refusé : « pas deux versions de l'app ») ; une implémentation en Rust pur comme `candle` (non mesurée, risque de lenteur sur CPU).
