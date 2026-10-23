# Plume — Voice Dictation Daemon

**EN** | [FR](#plume--dictée-vocale-fr)

A lightweight voice dictation daemon for Linux. Hold (or toggle) a hotkey to record your voice; Plume transcribes it with Whisper and injects the text into any focused window.

## Features

- **GPU-accelerated STT** — Whisper turbo via a persistent Python server (~60ms warm)
- **Local LLM correction** — Qwen3 0.6B on GPU via llama.cpp (~50ms, fixes typos & punctuation)
- **Silence detection** — RMS + Whisper `no_speech_prob` filters reject noise/disconnected mic
- **Live vocabulary learning** — capitalised words and acronyms auto-appended to `vocab.txt`
- **System tray** — blue circle (idle) / red circle (recording)
- **GTK4 settings window** — English / Français UI
- **Processor escalation** — if style≠clean and no local GPU, falls back to OpenAI/Anthropic

## Install

```bash
# Dependencies (Arch / CachyOS)
sudo pacman -S python-openai-whisper python-torch-cuda

# Build & install
git clone <repo> plume && cd plume
CUDA_PATH=/opt/cuda PATH="/opt/cuda/bin:$PATH" cargo build --release
./target/release/plume --install
```

The `--install` command copies the binary to `~/.local/bin/plume`, registers the autostart entry, and writes the default config.

## Usage

| Action | Default |
|--------|---------|
| Hold to record | Right Ctrl (hold) |
| Open settings | Tray → Settings |
| Download model | `plume --download-model` |

## Configuration

Edit `~/.config/plume.conf` or use the settings window (tray → Settings).

| Key | Default | Description |
|-----|---------|-------------|
| `HOTKEY` | `KEY_RIGHTCTRL` | Recording key |
| `MODE` | `hold` | `hold` or `toggle` |
| `STT_MODEL` | `turbo` | Whisper model size |
| `STT_LANG` | `fr` | Language (`fr`, `en`, … or empty for auto) |
| `PROCESSOR` | `local` | `local` / `openai` / `anthropic` / `ollama` / `none` |
| `PROCESSOR_STYLE` | `clean` | `clean` / `professional` / `diplomatic` / `assertive` / `humor` |
| `OUTPUT` | `inject` | `inject` / `scratchpad` / `both` |

## Benchmark (RTX 5080)

```
STT warm (5s audio)    ~60 ms
Processor (50 words)   ~50 ms
Inject                ~150 ms
Total                 ~260 ms
```

## Architecture

```
CpalRecorder → DictationService → LocalWhisper (Python server)
                                → LocalProcessor (llama.cpp CUDA)
                                → EnigoInjector (uinput Ctrl+V)
```

---

# Plume — Dictée Vocale (FR)

Un daemon de dictée vocale léger pour Linux. Maintenez (ou basculez) un raccourci clavier pour enregistrer votre voix ; Plume la transcrit avec Whisper et injecte le texte dans la fenêtre active.

## Fonctionnalités

- **STT accéléré GPU** — Whisper turbo via un serveur Python persistant (~60ms chaud)
- **Correction LLM local** — Qwen3 0.6B sur GPU via llama.cpp (~50ms, corrige fautes & ponctuation)
- **Détection de silence** — RMS + `no_speech_prob` Whisper filtrent le bruit / micro débranché
- **Apprentissage du vocabulaire** — mots capitalisés et acronymes ajoutés automatiquement à `vocab.txt`
- **Icône dans la barre** — rond bleu (inactif) / rond rouge (enregistrement)
- **Fenêtre de paramètres GTK4** — interface English / Français
- **Escalade de processeur** — si style≠correction et pas de GPU local, bascule sur OpenAI/Anthropic

## Installation

```bash
# Dépendances (Arch / CachyOS)
sudo pacman -S python-openai-whisper python-torch-cuda

# Compilation et installation
git clone <dépôt> plume && cd plume
CUDA_PATH=/opt/cuda PATH="/opt/cuda/bin:$PATH" cargo build --release
./target/release/plume --install
```

La commande `--install` copie le binaire dans `~/.local/bin/plume`, enregistre l'entrée de démarrage automatique et écrit la configuration par défaut.

## Utilisation

| Action | Par défaut |
|--------|-----------|
| Maintenir pour enregistrer | Ctrl droit (maintien) |
| Ouvrir les paramètres | Icône → Paramètres |
| Télécharger le modèle | `plume --download-model` |

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

## Benchmark (RTX 5080)

```
STT chaud (audio 5s)   ~60 ms
Processeur (50 mots)   ~50 ms
Injection             ~150 ms
Total                 ~260 ms
```

## Architecture

```
CpalRecorder → DictationService → LocalWhisper (serveur Python)
                                → LocalProcessor (llama.cpp CUDA)
                                → EnigoInjector (uinput Ctrl+V)
```
