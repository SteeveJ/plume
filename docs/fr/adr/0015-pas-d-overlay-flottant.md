# ADR 0015 — Pas d'overlay flottant, de bulle d'état ni de fenêtre « hub »

[English](../../en/adr/0015-no-floating-overlay.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Wispr Flow, que Plume prend pour référence, affiche un badge flottant d'enregistrement, une bulle d'état et une fenêtre principale (« hub »). Sur GNOME avec Wayland, une application ne peut pas garder une fenêtre au premier plan : le protocole `layer-shell` n'est pas implémenté par le compositeur de GNOME. Les options étaient : une extension GNOME séparée (autre langage, à installer à part, fragile d'une version de GNOME à l'autre), un mode X11 seulement (Wayland est le défaut), ou y renoncer.

## Décision

On y renonce. L'utilisateur est informé par ce qui existe déjà et marche partout : l'icône de la barre d'état (rond rouge pendant l'enregistrement), les notifications du bureau (« Enregistrement… », « Transcription… », erreurs avec leur cause) et la fenêtre de réglages GTK4 à la demande.

## Conséquences

- Aucune extension ni composant à installer à côté : cohérent avec la règle d'indépendance.
- Pas de badge visible au-dessus des autres fenêtres pendant qu'on parle ; le retour passe par l'icône et les notifications.
- Les historique, snippets et dictionnaire restent des fichiers texte et une base SQLite, éditables sans fenêtre dédiée.
- À reconsidérer si GNOME adopte `layer-shell`, ou pour KDE et les compositeurs wlroots qui le gèrent déjà.
