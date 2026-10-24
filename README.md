# Plume — Voice Dictation Daemon

**English** | [Français](README.fr.md)

A voice dictation daemon for Linux. Hold (or toggle) a hotkey and speak; Plume transcribes your voice with Whisper, cleans the text with a small local LLM, and types it into the focused window. Everything runs on your machine, in one binary, with no Python and no cloud account required.

## Features

- **Speech-to-text with whisper.cpp**, in-process, on GPU (CUDA, Vulkan) or CPU. Language is detected automatically or fixed with `STT_LANG`.
- **Local LLM correction** — Qwen3 0.6B through llama.cpp fixes punctuation, capitalisation and obvious transcription errors without changing your words.
- **Protection against invented text** — a voice detector (Silero) keeps silence and noise away from Whisper, backed by a volume check, Whisper's non-speech tags and a list of known hallucinations such as "Thank you.".
- **Snippets** — say a phrase, get a text (`~/.local/share/plume/snippets.txt`).
- **Live vocabulary** — names and acronyms are learned into `vocab.txt` and fed back to Whisper.
- **Light on memory** — Whisper is unloaded after 5 idle minutes and reloaded while you start speaking.
- **System tray + GTK4 settings window**, English / Français.
- **Instruction mode (optional)** — select text anywhere, hold a second key and say "make it more formal" or "translate to English"; an online model rewrites the selection in place. Needs an OpenAI or Anthropic key and sends the selected text to it.
- **Optional online styles** — formal, casual, excited, professional, diplomatic, assertive, humor (need an OpenAI or Anthropic key).

## Requirements

- Linux with `CONFIG_INPUT_UINPUT`, GTK4, a D-Bus session.
- Permission to use uinput:

```bash
sudo cp assets/99-plume.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo usermod -aG input $USER   # then log out and back in
```

- To build: a Rust toolchain, `cmake`, a C/C++ compiler and binutils (`nm`, `objcopy`). Nothing of this is needed at runtime. GPU builds also need the CUDA toolkit or the Vulkan SDK.

## Install

```bash
git clone https://github.com/SteeveJ/plume && cd plume

cargo build --release                                              # CPU only
CUDA_PATH=/usr/local/cuda cargo build --release --features cuda    # NVIDIA (Arch: /opt/cuda)
cargo build --release --features vulkan                            # AMD / others

./target/release/plume --install
```

On first launch Plume downloads the speech model for `STT_MODEL` and the LLM (about 390 MB) into `~/.local/share/plume/` and tells you with a notification. If the machine cannot hold the speech model in RAM it says so and suggests a smaller one.

## Usage

Hold **Right Ctrl**, speak, release. Change the key, mode, language, model and style in the tray menu → *Settings*, or edit `~/.config/plume.conf`.

| Command | Effect |
|---------|--------|
| `plume` | start the daemon (done by autostart after `--install`) |
| `plume --settings` | open the settings window |
| `plume --restart` | stop the running daemon and start a fresh one |
| `plume --download-model` | download the LLM now |
| `plume --install` | install binary, icons, desktop entry and autostart |

## Measured performance

RTX 5080 16 GB, Whisper `turbo` (f16) and Qwen3 0.6B on the GPU, one process:

| | |
|---|---|
| Speech-to-text, 11 s of audio | ~115 ms (CPU only: 4.3 s) |
| Speech-to-text, 5 minutes of audio | 2.4 s |
| LLM correction of a sentence | ~0.5 s |
| Process memory | ~680 MB RAM, ~2.6 GB VRAM |
| Binary size | 14 MB (CPU) / 604 MB (CUDA, cuBLAS linked statically) |

## Security and privacy

No software can promise to be secure, so here is what Plume does, what it doesn't, and what nobody has checked yet.

**What stays on your machine.** With the defaults (`PROCESSOR=local` and no API key) your voice, your text and your history never leave the computer: recognition and correction both run locally. The daemon listens on no network port and holds no elevated Linux privileges. Your config file, history and scratchpad can only be read by your user, and none of its log messages mention an API key.

**What leaves it, and when.**
- The models, downloaded once from Hugging Face over HTTPS.
- With `PROCESSOR=openai` or `anthropic`, or an online style such as `formal`: the dictated text, plus up to 2,000 characters before your cursor (read through accessibility). There is no switch for that second part yet.
- With instruction mode: the text you selected and your spoken instruction.
- If the local speech model can't load and an OpenAI key is set, Plume falls back to OpenAI's speech API, and that sends your audio.

**Known weak spots.**
- The Whisper and voice-detector models are downloaded without a checksum, then read by C++ code. A tampered file would be a real problem. Pinning checksums is the next fix.
- Plume needs the `input` group to see your hotkey and to type. That group isn't specific to Plume: any program you run can use it to read keystrokes or inject them. Plume itself only looks at the key you configured.
- API keys sit in `plume.conf` in plain text, and `history.db` keeps your last 10,000 dictations the same way. Both are mode 600, so disk encryption is what protects them if the machine is lost. Dictated text also stays in the clipboard, where a clipboard manager may keep it.
- Nothing sandboxes the process.

**Not checked yet.** The roughly 400 dependencies have never been through `cargo audit` or `cargo deny`. The copy of whisper.cpp in `vendor/` doesn't update itself. And nobody outside the project has reviewed the code. To report a vulnerability, see [SECURITY.md](SECURITY.md).

## Documentation

Full guides are in [`docs/`](docs/README.md): [configuration](docs/en/configuration.md), [architecture](docs/en/architecture.md), [development and tests](docs/en/development.md), [troubleshooting](docs/en/troubleshooting.md), and the [architecture decision records](docs/en/adr/README.md).

## Known limitations

- The 0.6B LLM corrects roughly four sentences out of six; on a long text it can fail to answer and Plume keeps your raw text. A larger model (Qwen3-1.7B) was tried and is not better with the current settings (see ADR 0004).
- Noise protection relies on a voice detector tested only on synthetic noise (white noise, hum, key clicks); a TV or distant voice may be taken for speech.
- Only CUDA has been tested on GPU; the Vulkan build is untested.
- There is no floating overlay, status bubble or hub window: GNOME Wayland cannot keep a window on top without an extension, so feedback goes through the tray icon and notifications (ADR 0015).

## Distro compatibility

| Distro | Status | Notes |
|--------|--------|-------|
| Arch / CachyOS | Tested | |
| Ubuntu 22.04+ | Should work | `libgtk-4-dev`, CUDA at `/usr/local/cuda` |
| Fedora 38+ | Should work | `gtk4-devel` |
| Debian 12+ | Should work | `libgtk-4-dev`, `gtk-update-icon-cache` optional |
| NixOS | Untested | CUDA path must be set manually |
