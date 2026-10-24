# Architecture

Plume follows a hexagonal layout ([ADR 0001](adr/0001-hexagonal-architecture.md)): the dictation logic depends only on traits (*ports*), and each piece of hardware or software is an *adapter*.

```
src/
  domain/ports/        traits: AudioRecorder, SpeechToText, TextProcessor, TextInjector,
                       HotkeyListener, HistoryStore, Notifier, ScratchpadWriter, ContextReader
  adapters/
    audio/             CpalRecorder          microphone, re-read at every recording
    hotkey/            EvdevListener         keyboards rescanned every 3 s
    stt/               WhisperCpp (+ Silero voice detector), OpenAiWhisper, audio_util, opus_enc
    processor/         LocalProcessor (llama.cpp), Ollama, OpenAi, Anthropic
    injector/          ClipboardInjector     clipboard + uinput Ctrl+V
    context/           AtspiContext          text before the cursor, via accessibility
    selection/         PrimarySelection      selected text, for instruction mode
    history/ scratchpad/ notifier/ http/
  application/
    dictation.rs       DictationService      the whole flow below
    snippets.rs        hallucination.rs
  config.rs sysmem.rs restart.rs hints.rs tray.rs settings_gtk.rs
```

## One dictation

1. The hotkey thread sends `RecordStart`. The service starts the recorder, tells Whisper to load if it was unloaded (`warm`), and notifies.
2. On `RecordStop` the audio is stopped and checked: too short → ignored; volume almost zero → "No sound — is the microphone muted?" and Whisper is not called.
3. A voice detector (Silero, [ADR 0014](adr/0014-silero-voice-detector.md)) looks for speech first: none → Whisper is not called; otherwise only the spoken zone is sent. Whisper (`spawn_blocking`) returns text. Non-speech tags (`[BLANK_AUDIO]`) are dropped. Punctuation-only text and phrases Whisper invents on noise are ignored ([ADR 0008](adr/0008-hallucination-filter.md)).
4. The text is corrected in chunks of about 350 characters by the processor. A chunk that comes back empty or more than 10 % shorter keeps the raw words ([ADR 0005](adr/0005-chunked-correction.md)).
5. Snippets are expanded, the result is saved to history, then typed (or written to the scratchpad).
6. Names and acronyms found in the text are appended to `vocab.txt`.

A new key press during step 3 or 4 is seen immediately and starts a new recording ([ADR 0007](adr/0007-compute-off-the-async-loop.md)).

## Instruction mode

With `INSTRUCT_HOTKEY` set and an online key, a second key rewrites the selected text ([ADR 0016](adr/0016-instruction-mode.md)). On press `PrimarySelection` reads the desktop selection (no key is simulated); the recording and the listening chain are the same as dictation (`hear()`), then the online model gets `Instruction` + `Text` and the answer is pasted over the selection. An empty answer or an error pastes nothing.

## Memory and resources

- Whisper is loaded at startup, unloaded after 5 idle minutes, and reloaded by `warm()` when you press the key, so you do not wait ([ADR 0006](adr/0006-unload-and-warm-up.md)).
- The LLM's cache is sized to the prompt, not to a fixed 4096 tokens (about 330 MB less at peak, measured).
- Recording stops growing after 10 minutes. Models are checked against free RAM and disk before loading or downloading ([ADR 0010](adr/0010-memory-and-disk-guards.md)).
- Whisper uses three quarters of the CPU cores (2 to 8).

## One binary, two ggml copies

whisper.cpp and llama.cpp each bundle their own, different, copy of ggml, which cannot be linked together as is. `vendor/whisper-rs-sys/build.rs` prefixes the symbols of whisper's copy after it is built ([ADR 0003](adr/0003-prefixed-ggml-symbols.md)). Do not update `whisper-rs` or `llama-cpp-2` without running the real-model end-to-end test.

## Cargo features

| Feature | Default | Effect |
|---------|---------|--------|
| `opus` | yes | Speech sent to OpenAI is Ogg/Opus, about 6× smaller than WAV. libopus is built from source and linked statically |
| `cuda` | no | NVIDIA GPU for Whisper and the LLM |
| `vulkan` | no | Vulkan GPU for both |
