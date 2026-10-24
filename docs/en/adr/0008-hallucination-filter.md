# ADR 0008 — Filter Whisper hallucinations on noise

**English** | [Français](../../fr/adr/0008-filtre-hallucinations.md)

- Status: accepted
- Date: 2026-10-24

## Context

On noise or silence, Whisper `turbo` invents text. Measured on white noise, a 50 Hz hum and key clicks, in French, English and automatic detection: "Thank you.", "Sous-titrage Société Radio-Canada", and above all "." or "...", punctuation alone that would have been typed into the document. `set_suppress_nst` makes it worse (the model then writes "you").

## Decision

The `application/hallucination.rs` module decides:
- an output with no letter or digit is always dropped;
- a phrase from the `INVENTED` list (the measured ones plus classics such as "merci d'avoir regardé…" and "subtitles by the Amara.org community") is dropped when the audio is weak (RMS < 0.03);
- almost null audio (RMS < 0.005) is not sent to Whisper and the user is told ("No sound — is the microphone muted?");
- the adapter removes the tags `[BLANK_AUDIO]`, `[MUSIC]`, `(silence)`.

## Consequences

- The same words said at normal volume pass ("Merci.", "Thank you.").
- Limit: a very quiet voice saying exactly "merci" is dropped. **Lifted by ADR 0014** (Silero voice detector) when the detector is present.
- The list needs upkeep: `tests/e2e_models.rs` replays real noise and prints any new phrase after a model change.
- Accents and capitals are handled ("Merci À Tous"), checked by a test.
