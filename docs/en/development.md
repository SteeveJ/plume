# Development

## Build

Requirements: Rust, `cmake`, a C/C++ compiler, binutils (`nm`, `objcopy`); for GPU, the CUDA toolkit (set `CUDA_PATH`) or the Vulkan SDK.

```bash
cargo build --release                                           # CPU
CUDA_PATH=/opt/cuda CMAKE_CUDA_ARCHITECTURES=native \
  cargo build --release --features cuda                         # your GPU only
```

A CUDA build takes about 11 minutes the first time (two copies of the CUDA kernels). Cargo does not notice a change of `CMAKE_CUDA_ARCHITECTURES`: run `cargo clean -p llama-cpp-sys-2 --release` and delete `target/release/build/whisper-rs-sys-*` to force it.

## Tests

| Command | What it covers | Needs |
|---------|----------------|-------|
| `cargo test --lib` | Unit tests: config parsing, snippets, filters, memory maths, hotkey mapping, history, restart | nothing |
| `cargo test --test config_load --test pipeline --test hallucinations --test french` | Real adapters with a fake microphone, keyboard and Whisper: config file, full dictation flow, hallucination cases, French accents and notifications | nothing |
| `PLUME_TEST_GGML=… PLUME_TEST_GGUF=… [PLUME_TEST_VAD=…] cargo test --release --test e2e_models -- --ignored` | Real Whisper and Qwen in one process, noise, silence. With `PLUME_TEST_VAD` (the Silero file) noise must produce no text at all | a ggml model, `model.gguf`, optionally the voice detector |
| `… cargo test --release --test e2e_french -- --ignored` | Understanding of spoken French by Whisper (forced and auto-detected language) and by the LLM | same |
| `PLUME_TEST_GGUF=… cargo test --release --test e2e_llm_eval -- --ignored --nocapture` | Rates a correction LLM on 12 French cases (empty answers, changed words, well-formed sentences). Passes for Qwen3-0.6B, fails for 1.7B | `model.gguf` |
| `PLUME_TEST_GGML=… cargo test --release --lib -- --ignored` | RAM is freed on unload and restored by warm-up | a ggml model |

Add `--features cuda` to run the real-model ones on the GPU. The real-model tests catch what compiling cannot: a symbol clash between the two ggml copies only crashes at run time.

Fixtures: `tests/fixtures/jfk.wav` (public-domain speech) and `tests/fixtures/fr/*.wav` (six French sentences made with the Piper voice `fr_FR-siwis-medium`; the reference texts are in `tests/common/mod.rs`).

## Every test is a bug that happened

Each integration test pins a real defect: a tray change of `PROCESSOR` that erased `PROCESSOR_STYLE`, a microphone error that stopped the daemon, `[BLANK_AUDIO]` typed into a document, an LLM that copied `/nothink` into the text, a closed channel that cancelled the transcription. When you fix a bug, add its case first and check it fails without the fix.

## Common tasks

- **New hallucination:** add the phrase (lowercase, no punctuation) to `INVENTED` in `application/hallucination.rs` and a case in `tests/hallucinations.rs`. After changing the speech model, run `e2e_models`: it prints any new invented phrase.
- **New style:** add it to `STYLES` and `style_prompt` in `config.rs` and to the settings combo.
- **New decision:** copy an ADR in `docs/en/adr/` **and** `docs/fr/adr/` (same number), and add it to both `README.md` indexes and to `docs/README.md`.
- **Updating whisper or llama bindings:** read [ADR 0003](adr/0003-prefixed-ggml-symbols.md) first.

## Conventions

Heavy work goes through `spawn_blocking`. Errors from the microphone, models or network never stop the daemon: notify with the cause and wait for the next key press. Files with keys or dictated text are created with `config::write_private`.
