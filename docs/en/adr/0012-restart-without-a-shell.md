# ADR 0012 — Restart without a shell with `plume --restart`

**English** | [Français](../../fr/adr/0012-redemarrage-sans-shell.md)

- Status: accepted
- Date: 2026-10-24

## Context

The settings window and the tray menu ran `sh -c "sleep; pkill -x plume; exec ~/.local/bin/plume &"`. That depended on `sh`, `sleep` and `pkill`, had a fixed path (a development build restarted the installed binary, sometimes stale), and `pkill -x plume` killed every process with that name, including the settings window.

## Decision

`plume --restart` finds the daemons in `/proc` (name `plume`, excluding `--settings`, `--restart` and itself), sends `SIGTERM`, waits up to 3 s then `SIGKILL`, and relaunches `current_exe()`. Settings and the tray just run that command.

## Consequences

- No external dependency, same behavior however Plume was started.
- The process search is tested with a fake `/proc` (`finds_only_real_daemons`).
