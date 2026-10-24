# ADR 0010 — Garde-fous mémoire et disque contre le manque de mémoire (OOM)

[English](../../en/adr/0010-memory-and-disk-guards.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Plusieurs chemins menaient à un manque de mémoire sans message : un modèle de 3 Go sur une machine de 4 Go (le noyau tue Plume, l'autostart recommence à chaque connexion) ; un enregistrement sans limite (environ 1,4 Go par heure à 48 kHz stéréo) ; un cache du LLM fixé à 4096 tokens (environ 450 Mo alloués à chaque dictée).

## Décision

- `sysmem::model_fits` compare le modèle à la RAM disponible (`MemAvailable`) : taille du fichier × 1,9 (mesuré pour Whisper `small`) + 256 Mo de marge. S'il ne tient pas, Plume le dit et propose un modèle plus petit. RAM inconnue : on tente.
- L'espace disque libre est vérifié avant un téléchargement (`statvfs`).
- L'enregistrement est plafonné à 10 minutes.
- Le cache du LLM est dimensionné sur le prompt plus 1024 tokens de génération.
- Whisper utilise les trois quarts des cœurs, pour que le bureau reste fluide.

## Conséquences

- Pic mesuré du LLM : 1131 → 799 Mo, avec la même sortie et la même latence.
- Le facteur 1,9 est empirique et propre à Whisper ; le revoir pour d'autres modèles.
- Le refus est explicite, avec le nom des modèles à essayer.
