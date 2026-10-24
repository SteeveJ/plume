# ADR 0006 — Unload Whisper when idle, warm it up on key press

**English** | [Français](../../fr/adr/0006-dechargement-et-prechauffage.md)

- Status: accepted
- Date: 2026-10-24

## Context

Whisper `turbo` in f16 takes about 2.1 GB of RAM on CPU, permanently, for a tool used in bursts.

## Decision

After 5 minutes without dictation, a thread unloads the model and its compute buffers. The `SpeechToText` trait gets a `warm()` method (empty by default) that `DictationService` calls as soon as recording starts: the model reloads while the user speaks. `warm()` does nothing if the model is already loaded or busy (`try_lock`).

## Consequences

- Measured with `small`: 891 MB → 43 MB when idle; a cold reload costs only ~60 ms more when the file is in the disk cache.
- VRAM is freed at the same time on a GPU build.
- Test doubles must expect `warm` (`expect_warm`).
- Real test: `whisper_memory_is_released_on_unload_and_restored_by_warm` checks that RAM really drops.
