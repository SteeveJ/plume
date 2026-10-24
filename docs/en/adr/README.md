# Architecture decision records

[Français](../../fr/adr/README.md)

Each record gives the context, the decision and its consequences. A record is never rewritten once accepted: a new one replaces it.

| # | Decision |
|---|----------|
| [0001](0001-hexagonal-architecture.md) | Hexagonal architecture (ports and adapters) |
| [0002](0002-in-process-whisper-cpp.md) | Whisper runs in-process with whisper.cpp, no Python |
| [0003](0003-prefixed-ggml-symbols.md) | Prefix whisper's ggml symbols to link it with llama.cpp |
| [0004](0004-qwen3-thinking-enabled.md) | Qwen3 keeps its thinking enabled, no `/nothink` suffix |
| [0005](0005-chunked-correction.md) | LLM correction in chunks with a content-loss guard |
| [0006](0006-unload-and-warm-up.md) | Unload Whisper when idle, warm it up on key press |
| [0007](0007-compute-off-the-async-loop.md) | Heavy compute runs in `spawn_blocking`, never in the async loop |
| [0008](0008-hallucination-filter.md) | Filter Whisper hallucinations on noise |
| [0009](0009-private-files.md) | Private files for keys and dictations |
| [0010](0010-memory-and-disk-guards.md) | Memory and disk guards against out-of-memory |
| [0011](0011-gpu-builds-and-static-linking.md) | Optional GPU builds, static linking, Opus by default |
| [0012](0012-restart-without-a-shell.md) | Restart without a shell with `plume --restart` |
| [0013](0013-test-strategy.md) | Four test levels, real models and real bugs |
| [0014](0014-silero-voice-detector.md) | Silero voice detector in front of Whisper |
| [0015](0015-no-floating-overlay.md) | No floating overlay, status bubble or "hub" window |
| [0016](0016-instruction-mode.md) | Instruction mode: rewrite the selection with an online model |
