# ADR 0005 — Correction du LLM par morceaux avec un garde-fou contre la perte de mots

[English](../../en/adr/0005-chunked-correction.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Avec le prompt de production (« Preserve the user's exact words »), Qwen3-0.6B ne rendait que 374 mots sur 572 en un seul appel : 35 % du texte disparaissait, silencieusement, dans le document. Il supprime les phrases répétées et saute des passages.

## Décision

Le texte est découpé en morceaux d'environ 350 caractères, à la fin d'une phrase, sans jamais couper une lettre accentuée. Chaque morceau est corrigé séparément. Sa correction est **rejetée au profit du texte brut** si elle est vide, ou si elle perd plus de 10 % des mots (pour les morceaux de 12 mots et plus). Le garde-fou est désactivé pour les styles qui reformulent, où raccourcir est légitime. Le contexte du curseur n'est envoyé qu'avec le premier morceau.

## Conséquences

- Sur le texte de test, 100 % des mots sont conservés (4 morceaux sur 8 sont revenus au texte brut, donc non corrigés).
- La correction d'un texte long prend environ deux fois plus de temps.
- Une sortie vide du LLM n'efface plus jamais la dictée (cas qui existait avant ce changement).
- Les tests : `processor_that_drops_content_falls_back_to_raw`, `empty_processor_output_never_erases_the_dictation`, `tests/pipeline.rs`.
