# ADR 0014 — Détecteur de voix Silero avant Whisper

[English](../../en/adr/0014-silero-voice-detector.md) | **Français**

- Statut : accepté
- Date : 2026-10-24
- Complète : ADR 0008

## Contexte

Les filtres de l'ADR 0008 sont des heuristiques : un seuil de volume et une liste de phrases inventées. Leur limite connue : un « merci » dit à voix très basse ressemble au « Merci. » que Whisper invente sur du bruit, et il était écarté. Le détecteur de voix intégré à whisper.cpp (`enable_vad`) ne marche pas avec `whisper-rs` : whisper.cpp ne l'applique que dans `whisper_full`, alors que `whisper-rs` appelle `whisper_full_with_state`. Essayé : résultats identiques avec et sans, aucune ligne de journal du détecteur.

## Décision

`WhisperCpp` utilise le détecteur Silero v5.1.2 (`WhisperVadContext`, modèle `ggml-silero-v5.1.2.bin`, 885 Ko, sur CPU) **avant** Whisper :
- aucune parole détectée : Whisper n'est pas appelé, le texte est vide ;
- de la parole : seule la zone parlée, élargie de 0,25 s de chaque côté, est envoyée à Whisper, et `Transcription.speech_confirmed` vaut vrai ;
- dans ce cas le filtre de l'ADR 0008 n'applique plus la règle des phrases inventées (la ponctuation seule reste écartée).

Le modèle est téléchargé au premier lancement avec celui de Whisper. S'il manque ou ne se charge pas, Plume continue avec les filtres de l'ADR 0008 (`with_vad` est facultatif).

## Conséquences

- Mesuré sur 6 phrases françaises : parole détectée à volume normal, à RMS 0,02 et à RMS 0,01. Silence, bruit blanc (jusqu'à RMS 0,05), bourdonnement 50 Hz et claquements : 0 segment de parole.
- Avec le détecteur, le bruit ne produit plus aucun texte pour la langue française, anglaise ou automatique (test réel `e2e_models`), et un « Merci. » à RMS 0,01 ou 0,02 est conservé (`e2e_french`).
- Un téléchargement de plus, très petit, et un calcul négligeable ; sur du bruit Whisper n'est plus lancé, ce qui économise du GPU. Supprimer le silence au début et à la fin retire aussi les hallucinations de bord.
- **Non vérifié :** seuls des bruits de synthèse ont été essayés (blanc, 50 Hz, claquements). Le bruit réel d'une pièce (ventilateur, voix lointaine à la télé) reste à mesurer ; une voix de télévision pourrait être détectée comme de la parole.
- L'OpenAI ne passe pas par ce détecteur : `speech_confirmed` y reste faux et l'ADR 0008 s'applique seul.
