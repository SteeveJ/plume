# ADR 0008 — Filtrer les hallucinations de Whisper sur le bruit

[English](../../en/adr/0008-hallucination-filter.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Sur du bruit ou du silence, Whisper `turbo` invente du texte. Mesuré sur du bruit blanc, un bourdonnement de 50 Hz et des claquements de touches, en français, anglais et détection automatique : « Thank you. », « Sous-titrage Société Radio-Canada », et surtout « . » ou « ... », de la ponctuation seule qui aurait été tapée dans le document. `set_suppress_nst` aggrave le problème (le modèle écrit alors « you »).

## Décision

Le module `application/hallucination.rs` décide :
- une sortie sans aucune lettre ni chiffre est toujours écartée ;
- une phrase de la liste `INVENTED` (celles mesurées plus les classiques « merci d'avoir regardé… », « subtitles by the Amara.org community ») est écartée quand l'audio est faible (RMS < 0,03) ;
- un audio presque nul (RMS < 0,005) n'est pas envoyé à Whisper et l'utilisateur est prévenu (« Aucun son — le micro est-il coupé ? ») ;
- l'adaptateur retire les balises `[BLANK_AUDIO]`, `[MUSIC]`, `(silence)`.

## Conséquences

- Les mêmes mots dits à voix normale passent (« Merci. », « Thank you. »).
- Limite : une voix très basse qui dit exactement « merci » est écartée. **Levée par l'ADR 0014** (détecteur de voix Silero) quand le détecteur est présent.
- La liste est à maintenir : `tests/e2e_models.rs` rejoue du bruit réel et affiche toute nouvelle phrase après un changement de modèle.
- Les accents et majuscules sont gérés (« Merci À Tous »), vérifié par un test.
