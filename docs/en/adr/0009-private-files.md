# ADR 0009 — Private files for keys and dictations

**English** | [Français](../../fr/adr/0009-fichiers-prives.md)

- Status: accepted
- Date: 2026-10-24

## Context

On the development machine, `~/.config/plume.conf` (OpenAI and Anthropic keys) and `history.db` (everything ever dictated) were mode `644`: readable by every user. So was the scratchpad `scratch.txt`.

## Decision

Every file holding keys or dictated text is written with `config::write_private` (mode `0600`, applied even to an existing file). The data folder is `0700`. At startup, files created earlier are tightened. The settings window and the tray menu also write through that function.

## Consequences

- Permissions are checked by tests (`write_private_forces_owner_only_even_on_existing_file`, `database_is_owner_only`, `appends_entries_to_a_private_file`).
- This does not protect against `root` or a poorly protected backup.
