# ADR 0010 — Memory and disk guards against out-of-memory

**English** | [Français](../../fr/adr/0010-garde-fous-memoire.md)

- Status: accepted
- Date: 2026-10-24

## Context

Several paths led to running out of memory without a message: a 3 GB model on a 4 GB machine (the kernel kills Plume and autostart tries again at every login); an unbounded recording (about 1.4 GB per hour at 48 kHz stereo); an LLM cache fixed at 4096 tokens (about 450 MB allocated for every dictation).

## Decision

- `sysmem::model_fits` compares the model with available RAM (`MemAvailable`): file size × 1.9 (measured for Whisper `small`) + 256 MB of headroom. If it does not fit, Plume says so and suggests a smaller model. Unknown RAM: try anyway.
- Free disk space is checked before a download (`statvfs`).
- Recording is capped at 10 minutes.
- The LLM cache is sized to the prompt plus 1024 generated tokens.
- Whisper uses three quarters of the cores, so the desktop stays smooth.

## Consequences

- Measured LLM peak: 1131 → 799 MB, with the same output and latency.
- The 1.9 factor is empirical and specific to Whisper; revisit it for other models.
- The refusal is explicit, naming the models to try.
