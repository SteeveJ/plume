# ADR 0003 — Prefix whisper's ggml symbols to link it with llama.cpp

**English** | [Français](../../fr/adr/0003-symboles-ggml-prefixes.md)

- Status: accepted
- Date: 2026-10-24

## Context

`whisper-rs-sys` and `llama-cpp-sys-2` each bundle their own copy of ggml, in different versions. Linking them into one binary fails: hundreds of duplicate symbols (`ggml_backend_*`, `ggml-quants`). Forcing the link with `--allow-multiple-definition` would have mixed two incompatible versions and crashed at run time.

## Decision

`vendor/whisper-rs-sys` is a copy of the crate whose `build.rs` rewrites whisper's static archives right after they are built: `nm` lists the defined symbols, then `objcopy --redefine-syms` adds the prefix `pw_` to all of them, **local ones included**, except the public `whisper_*` API and the C++ standard library instantiations (`std::`, `__gnu_cxx::`). The crate is wired in through `[patch.crates-io]`.

Two traps found on the way:
1. Renaming the standard library symbols made a `std::string` call from llama point to nothing (crash before `main`).
2. Both ggml copies define the same C++ class in a **COMDAT group** of the same name; the linker kept whisper's and discarded llama's. The local symbols that act as the signature of those groups must be renamed too.

## Consequences

- One binary; tested on x86_64 Linux, on CPU and with CUDA (Whisper `turbo` and Qwen in the same process).
- The build needs `nm` and `objcopy` (binutils).
- The `vendor/` folder adds about 14 MB of whisper.cpp sources.
- Fragile when updating: after any change to `whisper-rs` or `llama-cpp-2`, run `tests/e2e_models.rs`. A clash only crashes at run time, compiling does not show it.
