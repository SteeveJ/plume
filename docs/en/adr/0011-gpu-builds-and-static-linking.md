# ADR 0011 — Optional GPU builds, static linking, Opus by default

**English** | [Français](../../fr/adr/0011-build-gpu-et-liaison-statique.md)

- Status: accepted
- Date: 2026-10-24

## Context

The CPU binary is 14 MB, the CUDA one 604 MB (cuBLAS statically linked in both ggml copies). The independence rule forbids requiring system libraries at runtime. Opus encoding (upload to OpenAI about 6 times smaller) first required `libopus-dev`.

## Decision

- `cuda` and `vulkan` are Cargo features that are **off by default**; a CPU build works everywhere.
- A CUDA build links its libraries statically: only the NVIDIA driver is needed at runtime, not the toolkit.
- `opus` is **on by default**: libopus is built from source and linked statically (variables in `.cargo/config.toml`, `cmake` needed at build time only).
- `CMAKE_CUDA_ARCHITECTURES=native` compiles kernels only for the installed card (about 100 MB less).

## Consequences

- Measured: Whisper `turbo` + Qwen on an RTX 5080 fit in ~680 MB of RAM and ~2.6 GB of VRAM; 11 s of audio is transcribed in ~115 ms (4.3 s on CPU).
- A CUDA build takes about 11 minutes and Cargo does not notice an architecture change: the rebuild must be forced (see `docs/*/development.md`).
- ~~`libgomp` (OpenMP) stays a dynamic system library, inherited from llama.cpp.~~ **Lifted on 2026-10-24:** a test on clean distributions showed the binary did not start without `libgomp`. llama.cpp is now built without OpenMP (`default-features = false`); measured on CPU, 3.35 s against 3.40 s per sentence, so nothing is lost. The binary now depends only on GTK4, ALSA, D-Bus, SQLite and libc.
- A CUDA binary does not start without the NVIDIA driver: hence the choice not to enable it by default.
