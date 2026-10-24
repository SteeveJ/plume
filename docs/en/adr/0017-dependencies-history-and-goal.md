# ADR 0017 — Dependencies: history, current state and the "ready to use" goal

**English** | [Français](../../fr/adr/0017-dependances-historique-et-objectif.md)

- Status: goal accepted, implementation in progress
- Date: 2026-10-25
- Complements: ADR 0002, 0003, 0011

## Context

The user's rule: Plume installs and works without installing anything beside it, not even a library. Here is how the situation evolved.

| When | Dependency | What happened |
|------|-----------|---------------|
| Start | Python, `openai-whisper`, `torch`, `pip` | Persistent Whisper server (1 to 2 GB of RAM). Replaced by whisper.cpp in the process (ADR 0002). |
| Start | `sh`, `pkill`, `sleep`, `which`, `touch` | Restart and install went through the shell. Replaced by `plume --restart` (reads `/proc`, ADR 0012) and plain Rust. |
| Opus | system `libopus` | First linked dynamically; built from source and linked statically once the independence rule was applied (ADR 0011). Broke on Fedora (`lib64`), fixed by the vendored copy of `audiopus_sys`. |
| Two ggml | duplicate symbols | Vendored copy of `whisper-rs-sys` that prefixes the symbols (ADR 0003). Needs `nm` and `objcopy` at build time. |
| CUDA | CUDA toolkit | Linked statically; at run time only the NVIDIA driver is needed (ADR 0011). |
| Container test | `libgomp` (OpenMP) | The binary did not start without it. llama.cpp is now built without OpenMP. |
| Today | GTK4 (+ ~100 libraries), libdbus, libasound, libsqlite3 | Still linked. The subject of this ADR. |

**State measured on 2026-10-25** on six clean distributions: the binary still links `libgtk-4` (and the hundred or so libraries it drags in: cairo, pango, gstreamer, cups, avahi…), `libdbus-1`, `libasound` and `libsqlite3`, besides libc, libstdc++ and libgcc_s. They come from the crates `gtk4` (settings window), `dbus` (`notify-rust` in `d` mode and `ksni` 0.2), `alsa` (`cpal`) and `libsqlite3-sys` (`rusqlite`).

**Why it was not seen earlier.** The `ldd` check of the distribution matrix ran in a container where I had first installed these four run-time packages. It therefore checked that the installed libraries are enough, not that none is needed. The documentation even listed them as normal requirements.

## Decision

The goal is **zero system libraries at run time**, apart from libc and the compiler's base libraries. The check changes: `ldd` on a clean image where **no run-time package is installed**.

Means decided:
- `rusqlite` with the `bundled` feature (SQLite compiled into the binary);
- `notify-rust` with the `z` feature (pure-Rust D-Bus client, `zbus`);
- `ksni` 0.3 (also `zbus`) for the tray icon.

Means **for the user to decide**:
- the settings window: in `egui` (pure Rust, graphics libraries loaded only when it opens, present on any desktop), no window (settings through the icon and `plume.conf`), or GTK4 as an optional feature that is off by default;
- audio: keep `libasound` (Linux's audio API) or write a pure-Rust PulseAudio/PipeWire client (the `pulseaudio` crate), which excludes systems without a sound server.

## Consequences

- The binary grows a little (SQLite about 1 MB; `egui` about 10 MB).
- Rewriting the tray icon cannot be checked in a container: it needs the real desktop.
- A pure-Rust audio recorder would be new code, to test with a real microphone.
- Out of scope: libc, the NVIDIA driver (GPU builds), and the models downloaded on first launch (network needed once).
- Until this is done, the documentation must not present these libraries as normal.
