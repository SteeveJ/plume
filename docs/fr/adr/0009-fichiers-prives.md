# ADR 0009 — Fichiers privés pour les clés et les dictées

[English](../../en/adr/0009-private-files.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Sur la machine de développement, `~/.config/plume.conf` (clés OpenAI et Anthropic) et `history.db` (tout ce qui a été dicté) étaient en mode `644` : lisibles par tous les utilisateurs. Le bloc-notes `scratch.txt` aussi.

## Décision

Tout fichier qui contient des clés ou du texte dicté est écrit avec `config::write_private` (mode `0600`, appliqué même à un fichier existant). Le dossier de données est en `0700`. Au démarrage, les fichiers déjà créés sont resserrés. La fenêtre de réglages et le menu de l'icône écrivent aussi par cette fonction.

## Conséquences

- Les droits sont vérifiés par des tests (`write_private_forces_owner_only_even_on_existing_file`, `database_is_owner_only`, `appends_entries_to_a_private_file`).
- Cela ne protège pas contre `root` ni contre une sauvegarde mal protégée.
