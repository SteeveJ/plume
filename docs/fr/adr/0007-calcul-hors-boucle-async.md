# ADR 0007 — Le calcul lourd passe par `spawn_blocking`, jamais dans la boucle async

[English](../../en/adr/0007-compute-off-the-async-loop.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

`tokio::task::block_in_place` bloquait le thread pendant que Whisper ou le LLM calculait. La boucle de dictée, qui fait la course entre la transcription et un nouvel appui sur la touche (`select!`), ne voyait donc cet appui qu'une fois le calcul terminé : les premiers mots de la dictée suivante étaient perdus (jusqu'à plusieurs secondes sur un texte long). Les tests étaient aussi instables : sur un canal déjà fermé, `select!` choisissait une branche au hasard.

## Décision

Whisper et le LLM calculent dans `spawn_blocking`, avec leur état dans un `Arc`. `select!` est en mode `biased` : la transcription est sondée en premier.

## Conséquences

- Une nouvelle pression de touche est vue tout de suite et démarre un nouvel enregistrement.
- Un calcul lancé ne peut pas être interrompu ; si l'appel est abandonné, son résultat est ignoré et le verrou du modèle sert de file d'attente.
- Les tests de la boucle sont maintenant déterministes.
