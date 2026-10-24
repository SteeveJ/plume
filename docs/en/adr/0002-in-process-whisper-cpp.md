# ADR 0002 — Whisper runs in-process with whisper.cpp, no Python

**English** | [Français](../../fr/adr/0002-whisper-cpp-en-processus.md)

- Status: accepted
- Date: 2026-10-24

## Context

The first version ran a persistent Python server (`openai-whisper` + `torch`). It held 1 to 2 GB of RAM at all times, required Python and `pip` on the user's machine, and could die (killed by the kernel when memory ran out) without Plume noticing: every following dictation then failed. This contradicts the independence rule: no external dependency at runtime.

## Decision

Whisper runs inside the Plume process through `whisper-rs` (whisper.cpp). The `ggml` model matching `STT_MODEL` is downloaded on first launch, after a notification. If the model cannot be loaded or downloaded, Plume falls back to the OpenAI API. The Python server, `whisper_server.py` and `requirements.txt` are removed.

## Consequences

- No more Python, no more process to watch; the model can be unloaded and reloaded (ADR 0006).
- GPU through the CUDA or Vulkan builds (ADR 0011).
- A model file must be downloaded (`small` 470 MB, `turbo` 1.6 GB).
- whisper.cpp and llama.cpp share the process, hence ADR 0003.
- **Rejected**: keeping Python; a second executable `plume-whisper` (refused: "not two versions of the app"); a pure-Rust implementation such as `candle` (not measured, risk of slowness on CPU).
