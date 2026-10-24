# ADR 0014 — Silero voice detector in front of Whisper

**English** | [Français](../../fr/adr/0014-detecteur-de-voix-silero.md)

- Status: accepted
- Date: 2026-10-24
- Complements: ADR 0008

## Context

The filters of ADR 0008 are heuristics: a volume threshold and a list of invented phrases. Their known limit: a very quiet "merci" looks like the "Merci." Whisper invents on noise, and it was dropped. whisper.cpp's built-in voice detector (`enable_vad`) does not work through `whisper-rs`: whisper.cpp only applies it in `whisper_full`, while `whisper-rs` calls `whisper_full_with_state`. Tried: identical results with and without it, and no detector log line.

## Decision

`WhisperCpp` uses the Silero v5.1.2 detector (`WhisperVadContext`, model `ggml-silero-v5.1.2.bin`, 885 KB, on CPU) **before** Whisper:
- no speech detected: Whisper is not called and the text is empty;
- speech: only the spoken zone, widened by 0.25 s on each side, is sent to Whisper, and `Transcription.speech_confirmed` is true;
- in that case the ADR 0008 filter no longer applies the invented-phrase rule (punctuation-only text is still dropped).

The model is downloaded on first launch together with Whisper's. If it is missing or fails to load, Plume carries on with the ADR 0008 filters (`with_vad` is optional).

## Consequences

- Measured on 6 French sentences: speech detected at normal volume, at RMS 0.02 and at RMS 0.01. Silence, white noise (up to RMS 0.05), a 50 Hz hum and key clicks: 0 speech segments.
- With the detector, noise produces no text at all in French, English or automatic language (real-model test `e2e_models`), and a "Merci." at RMS 0.01 or 0.02 is kept (`e2e_french`).
- One more very small download and a negligible computation; on noise Whisper no longer runs, which saves GPU time. Trimming silence at the start and end also removes edge hallucinations.
- **Not verified:** only synthetic noises were tried (white, 50 Hz, clicks). Real room noise (a fan, a distant TV voice) is still to be measured; a TV voice may well be detected as speech.
- OpenAI does not go through this detector: `speech_confirmed` stays false there and ADR 0008 applies alone.
