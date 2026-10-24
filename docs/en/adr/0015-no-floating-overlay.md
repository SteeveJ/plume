# ADR 0015 — No floating overlay, status bubble or "hub" window

**English** | [Français](../../fr/adr/0015-pas-d-overlay-flottant.md)

- Status: accepted
- Date: 2026-10-24

## Context

Wispr Flow, Plume's reference, shows a floating recording badge, a status bubble and a main window (the "hub"). On GNOME with Wayland, an application cannot keep a window on top: the `layer-shell` protocol is not implemented by GNOME's compositor. The options were: a separate GNOME extension (another language, installed apart, fragile from one GNOME version to the next), an X11-only mode (Wayland is the default), or giving up.

## Decision

We give up. The user is informed by what already exists and works everywhere: the tray icon (red circle while recording), desktop notifications ("Recording…", "Transcribing…", errors with their cause) and the GTK4 settings window on demand.

## Consequences

- No extension or side component to install: consistent with the independence rule.
- No badge above other windows while you speak; feedback goes through the icon and notifications.
- History, snippets and the dictionary stay text files and a SQLite database, editable without a dedicated window.
- To reconsider if GNOME adopts `layer-shell`, or for KDE and wlroots compositors that already support it.
