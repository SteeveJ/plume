# ADR 0001 — Hexagonal architecture (ports and adapters)

**English** | [Français](../../fr/adr/0001-architecture-hexagonale.md)

- Status: accepted
- Date: 2026-10-24

## Context

Plume connects hardware (microphone, virtual keyboard, clipboard), models and online services. The whole dictation chain must be testable without a microphone, a GPU or a network.

## Decision

The logic lives in `application/` and depends only on traits defined in `domain/ports/` (`AudioRecorder`, `SpeechToText`, `TextProcessor`, `TextInjector`, `HotkeyListener`, `HistoryStore`, `Notifier`, `ScratchpadWriter`, `ContextReader`). Each piece of hardware or service is an adapter in `adapters/`. `DictationService` is generic over the microphone, the speech recognizer and the injector.

## Consequences

- Tests replace the microphone, keyboard and Whisper with fakes (`mockall` for unit tests, plain structs in `tests/common`): see ADR 0013.
- Switching engines (OpenAI, whisper.cpp, Ollama…) does not touch the logic.
- Cost: one more layer of indirection, and `Box<dyn …>` for optional components.
- Names must tell the truth: `RdevListener` and `EnigoInjector` used neither `rdev` nor `enigo`; they became `EvdevListener` and `ClipboardInjector`.
