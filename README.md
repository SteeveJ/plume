# Plume — Voice Dictation Daemon

**EN** | [FR](#plume--dictée-vocale-fr)

A lightweight voice dictation daemon for Linux. Hold (or toggle) a hotkey to record your voice; Plume transcribes it with Whisper and injects the text into any focused window.

## Features

- **GPU-accelerated STT** — Whisper turbo via a persistent Python server (~60ms warm)
- **Local LLM correction** — Qwen3 0.6B on GPU via llama.cpp (~50ms, fixes typos & punctuation)
- **Silence detection** — RMS + Whisper `no_speech_prob` filters reject noise and disconnected mic
- **Live vocabulary learning** — capitalised words and acronyms auto-appended to `vocab.txt`
- **System tray** — blue circle (idle) / red circle (recording)
- **GTK4 settings window** — English / Français UI
- **Processor escalation** — if style ≠ clean and no local GPU, escalates to OpenAI/Anthropic

## Requirements

### System
- Linux kernel ≥ 4.5 with `CONFIG_INPUT_UINPUT` (standard on all desktop distros)
- Python 3 in `PATH`
- GTK4 runtime
- A running D-Bus session (standard on all desktop environments)
- **uinput permissions**: add yourself to the `input` group and install the udev rule:

```bash
sudo cp assets/99-plume.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo usermod -aG input $USER
# log out and back in
```

### Python (Whisper)
```bash
pip install -r assets/requirements.txt
# or with CUDA:
pip install openai-whisper torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu124
```

### Rust toolchain
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Install

```bash
git clone https://github.com/SteeveJ/plume && cd plume

# Without GPU (CPU only):
cargo build --release

# With NVIDIA GPU (CUDA):
CUDA_PATH=/usr/local/cuda cargo build --release --features cuda
# Arch/CachyOS: CUDA_PATH=/opt/cuda

# With AMD GPU (Vulkan):
cargo build --release --features vulkan

./target/release/plume --install
```

### CUDA path by distro
| Distro | CUDA_PATH |
|--------|-----------|
| Arch / CachyOS | `/opt/cuda` |
| Ubuntu / Debian | `/usr/local/cuda` |
| Fedora | `/usr/local/cuda` |

## Usage

| Action | Default |
|--------|---------|
| Hold to record | Right Ctrl |
| Open settings | Tray → Settings |
| Download Whisper model | `plume --download-model` |

## Configuration

Edit `~/.config/plume.conf` or use the settings window (tray → Settings).

| Key | Default | Description |
|-----|---------|-------------|
| `HOTKEY` | `KEY_RIGHTCTRL` | Recording key |
| `MODE` | `hold` | `hold` or `toggle` |
| `STT_MODEL` | `turbo` | Whisper model size |
| `STT_LANG` | `fr` | Language (`fr`, `en`, … or empty for auto-detect) |
| `PROCESSOR` | `local` | `local` / `openai` / `anthropic` / `ollama` / `none` |
| `PROCESSOR_STYLE` | `clean` | `clean` / `professional` / `diplomatic` / `assertive` / `humor` |
| `OUTPUT` | `inject` | `inject` / `scratchpad` / `both` |

## Benchmark (RTX 5080, CUDA)

```
STT warm (5s audio)    ~60 ms
Processor (50 words)   ~50 ms
Inject                ~150 ms
Total                 ~260 ms
```

## Architecture

```
CpalRecorder → DictationService → LocalWhisper (persistent Python subprocess)
                                → LocalProcessor (llama.cpp, CUDA/Vulkan/CPU)
                                → EnigoInjector (uinput Ctrl+V)
```

**Design notes**
- `TextProcessor` is the central bridge between STT, LLM, tray, and vocab — the highest-connectivity node in the graph
- Python dependency (openai-whisper, torch) is declared in docs only; the Rust binary itself has zero Python coupling at compile time
- Zero runtime binary deps: arboard+uinput (no wl-copy/xdotool), notify-rust D-Bus (no notify-send), XDG autostart (no systemd user service needed) — everything works out of the box on any desktop

## Distro compatibility

| Distro | Status | Notes |
|--------|--------|-------|
| Arch / CachyOS | ✅ Tested | |
| Ubuntu 22.04+ | ✅ Should work | `libgtk-4-dev`, CUDA at `/usr/local/cuda` |
| Fedora 38+ | ✅ Should work | `gtk4-devel` |
| Debian 12+ | ✅ Should work | `libgtk-4-dev`, `gtk-update-icon-cache` optional |
| NixOS | ⚠ Untested | CUDA store path must be set manually |

## Known Issues / TODO

**Must fix**
- [ ] `local_whisper.rs` — `device="cuda"` hardcoded: CPU-only machines crash silently, OpenAI fallback never triggers
- [ ] `local_whisper.rs` — personal dev path `/home/skyro/WanGP/.venv/bin/python` in Python candidates list

**Should fix**
- [ ] `main.rs` — `ollama_available()` fires a full inference request; replace with `GET /api/version`
- [ ] `local_processor.rs` — `LlamaContext` recreated on every call (~100ms wasted per dictation)
- [ ] `civic_processor.rs` — orphan file, not in `mod.rs`, never compiled; delete
- [ ] `enigo_injector.rs` — clipboard restore races the paste on slow windows

---

# Plume — Dictée Vocale (FR)

Un daemon de dictée vocale léger pour Linux. Maintenez (ou basculez) un raccourci clavier pour enregistrer votre voix ; Plume transcrit avec Whisper et injecte le texte dans la fenêtre active.

## Fonctionnalités

- **STT accéléré GPU** — Whisper turbo via un serveur Python persistant (~60ms chaud)
- **Correction LLM local** — Qwen3 0.6B sur GPU via llama.cpp (~50ms, corrige fautes & ponctuation)
- **Détection de silence** — RMS + `no_speech_prob` Whisper filtrent le bruit / micro débranché
- **Apprentissage du vocabulaire** — mots capitalisés et acronymes ajoutés automatiquement à `vocab.txt`
- **Icône dans la barre** — rond bleu (inactif) / rond rouge (enregistrement)
- **Fenêtre de paramètres GTK4** — interface English / Français
- **Escalade de processeur** — si style ≠ correction et pas de GPU local, bascule sur OpenAI/Anthropic

## Prérequis

### Système
- Noyau Linux ≥ 4.5 avec `CONFIG_INPUT_UINPUT`
- Python 3 dans le `PATH`
- GTK4
- Session D-Bus active

**Permissions uinput** — ajouter l'utilisateur au groupe `input` et installer la règle udev :

```bash
sudo cp assets/99-plume.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo usermod -aG input $USER
# se déconnecter et se reconnecter
```

### Python (Whisper)
```bash
pip install -r assets/requirements.txt
# ou avec CUDA :
pip install openai-whisper torch torchvision torchaudio --index-url https://download.pytorch.org/whl/cu124
```

## Installation

```bash
git clone https://github.com/SteeveJ/plume && cd plume

# Sans GPU (CPU) :
cargo build --release

# Avec GPU NVIDIA (CUDA) :
CUDA_PATH=/usr/local/cuda cargo build --release --features cuda
# Arch/CachyOS : CUDA_PATH=/opt/cuda

# Avec GPU AMD (Vulkan) :
cargo build --release --features vulkan

./target/release/plume --install
```

## Configuration

Modifiez `~/.config/plume.conf` ou utilisez la fenêtre de paramètres (icône → Paramètres).

| Clé | Défaut | Description |
|-----|--------|-------------|
| `HOTKEY` | `KEY_RIGHTCTRL` | Touche d'enregistrement |
| `MODE` | `hold` | `hold` (maintien) ou `toggle` (bascule) |
| `STT_MODEL` | `turbo` | Taille du modèle Whisper |
| `STT_LANG` | `fr` | Langue (`fr`, `en`, … ou vide pour auto) |
| `PROCESSOR` | `local` | `local` / `openai` / `anthropic` / `ollama` / `none` |
| `PROCESSOR_STYLE` | `clean` | `clean` / `professional` / `diplomatic` / `assertive` / `humor` |
| `OUTPUT` | `inject` | `inject` / `scratchpad` / `les deux` |

## Benchmark (RTX 5080, CUDA)

```
STT chaud (audio 5s)   ~60 ms
Processeur (50 mots)   ~50 ms
Injection             ~150 ms
Total                 ~260 ms
```

## Compatibilité distros

| Distro | Statut | Notes |
|--------|--------|-------|
| Arch / CachyOS | ✅ Testé | |
| Ubuntu 22.04+ | ✅ Devrait fonctionner | `libgtk-4-dev`, CUDA dans `/usr/local/cuda` |
| Fedora 38+ | ✅ Devrait fonctionner | `gtk4-devel` |
| Debian 12+ | ✅ Devrait fonctionner | `libgtk-4-dev`, `gtk-update-icon-cache` optionnel |
| NixOS | ⚠ Non testé | Chemin CUDA à configurer manuellement |
