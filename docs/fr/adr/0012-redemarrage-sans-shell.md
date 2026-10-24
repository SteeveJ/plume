# ADR 0012 — Redémarrer sans shell avec `plume --restart`

[English](../../en/adr/0012-restart-without-a-shell.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

La fenêtre de réglages et le menu de l'icône lançaient `sh -c "sleep; pkill -x plume; exec ~/.local/bin/plume &"`. Cela dépendait de `sh`, `sleep` et `pkill`, avait un chemin figé (une version de développement relançait le binaire installé, parfois périmé) et `pkill -x plume` tuait tous les processus portant ce nom, y compris la fenêtre de réglages.

## Décision

`plume --restart` trouve les démons dans `/proc` (nom `plume`, hors `--settings` et `--restart`, hors lui-même), envoie `SIGTERM`, attend jusqu'à 3 s puis `SIGKILL`, et relance `current_exe()`. Les réglages et l'icône ne font que lancer cette commande.

## Conséquences

- Aucune dépendance externe, comportement identique quelle que soit la façon dont Plume a été lancé.
- La recherche des processus est testée avec un faux `/proc` (`finds_only_real_daemons`).
