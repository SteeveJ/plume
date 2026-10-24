# ADR 0006 — Décharger Whisper à l'inactivité, le recharger à l'appui sur la touche

[English](../../en/adr/0006-unload-and-warm-up.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Whisper `turbo` en f16 occupe environ 2,1 Go de RAM sur CPU, en permanence, pour un outil qu'on utilise par à-coups.

## Décision

Après 5 minutes sans dictée, un thread décharge le modèle et ses tampons de calcul. Le trait `SpeechToText` reçoit une méthode `warm()` (vide par défaut) que `DictationService` appelle dès le début de l'enregistrement : le modèle se recharge pendant que l'utilisateur parle. `warm()` ne fait rien si le modèle est déjà chargé ou occupé (`try_lock`).

## Conséquences

- Mesuré avec `small` : 891 Mo → 43 Mo au repos ; un rechargement à froid ne coûte que ~60 ms de plus quand le fichier est dans le cache disque.
- La VRAM est libérée en même temps sur un build GPU.
- Les doubles de test doivent prévoir `warm` (`expect_warm`).
- Test réel : `whisper_memory_is_released_on_unload_and_restored_by_warm` vérifie que la RAM baisse vraiment.
