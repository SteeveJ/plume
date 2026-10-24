# Plume — Dictée vocale

[English](README.md) | **Français**

Un démon de dictée vocale pour Linux. Maintenez (ou basculez) un raccourci clavier et parlez : Plume transcrit votre voix avec Whisper, nettoie le texte avec un petit LLM local, et le tape dans la fenêtre active. Tout tourne sur votre machine, dans un seul binaire, sans Python et sans compte en ligne.

## Fonctionnalités

- **Reconnaissance vocale avec whisper.cpp**, dans le processus, sur GPU (CUDA, Vulkan) ou CPU. La langue est détectée automatiquement ou fixée avec `STT_LANG`.
- **Correction par un LLM local** — Qwen3 0,6B via llama.cpp corrige la ponctuation, les majuscules et les erreurs évidentes de transcription sans changer vos mots.
- **Protection contre le texte inventé** — le silence et le bruit n'atteignent jamais votre document (contrôle du volume, balises « non-parole » de Whisper, hallucinations connues comme « Thank you. »).
- **Snippets** — dites une phrase, obtenez un texte (`~/.local/share/plume/snippets.txt`).
- **Vocabulaire vivant** — noms et sigles sont appris dans `vocab.txt` et renvoyés à Whisper.
- **Sobre en mémoire** — Whisper est déchargé après 5 minutes d'inactivité et rechargé pendant que vous commencez à parler.
- **Icône dans la barre + fenêtre de réglages GTK4**, English / Français.
- **Styles en ligne facultatifs** — formel, décontracté, enthousiaste, professionnel, diplomatique, assertif, humour (clé OpenAI ou Anthropic nécessaire).

## Prérequis

- Linux avec `CONFIG_INPUT_UINPUT`, GTK4, une session D-Bus.
- Le droit d'utiliser uinput :

```bash
sudo cp assets/99-plume.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo usermod -aG input $USER   # puis se déconnecter et se reconnecter
```

- Pour compiler : Rust, `cmake`, un compilateur C/C++ et binutils (`nm`, `objcopy`). Rien de tout cela n'est nécessaire à l'exécution. Les builds GPU demandent aussi le toolkit CUDA ou le SDK Vulkan.

## Installation

```bash
git clone https://github.com/SteeveJ/plume && cd plume

cargo build --release                                              # CPU seulement
CUDA_PATH=/usr/local/cuda cargo build --release --features cuda    # NVIDIA (Arch : /opt/cuda)
cargo build --release --features vulkan                            # AMD / autres

./target/release/plume --install
```

Au premier lancement, Plume télécharge le modèle vocal de `STT_MODEL` et le LLM (environ 390 Mo) dans `~/.local/share/plume/`, et vous prévient par une notification. Si la machine ne peut pas contenir le modèle vocal en RAM, Plume le dit et en propose un plus petit.

## Utilisation

Maintenez **Ctrl droit**, parlez, relâchez. Changez la touche, le mode, la langue, le modèle et le style dans le menu de l'icône → *Paramètres*, ou modifiez `~/.config/plume.conf`.

| Commande | Effet |
|----------|-------|
| `plume` | lance le démon (fait par l'autostart après `--install`) |
| `plume --settings` | ouvre la fenêtre de réglages |
| `plume --restart` | arrête le démon en cours et en démarre un neuf |
| `plume --download-model` | télécharge le LLM tout de suite |
| `plume --install` | installe le binaire, les icônes, l'entrée de menu et l'autostart |

## Performances mesurées

RTX 5080 16 Go, Whisper `turbo` (f16) et Qwen3 0,6B sur le GPU, un seul processus :

| | |
|---|---|
| Reconnaissance vocale, 11 s d'audio | ~115 ms (CPU seul : 4,3 s) |
| Reconnaissance vocale, 5 minutes d'audio | 2,4 s |
| Correction d'une phrase par le LLM | ~0,5 s |
| Mémoire du processus | ~680 Mo de RAM, ~2,6 Go de VRAM |
| Taille du binaire | 14 Mo (CPU) / 604 Mo (CUDA, cuBLAS lié statiquement) |

## Documentation

Les guides complets sont dans [`docs/`](docs/README.md) : [configuration](docs/fr/configuration.md), [architecture](docs/fr/architecture.md), [développement et tests](docs/fr/development.md), [dépannage](docs/fr/troubleshooting.md), et les [décisions d'architecture (ADR)](docs/fr/adr/README.md).

## Limites connues

- Le LLM de 0,6B corrige environ quatre phrases sur six ; sur un texte long il peut ne pas répondre et Plume garde alors votre texte brut. Un modèle plus grand est dans la liste de tâches.
- Une voix très basse (sous le seuil de bruit) qui dit exactement « merci » ou « thank you » est traitée comme du bruit.
- Seul CUDA a été testé sur GPU ; le build Vulkan n'est pas testé.
- Les surcouches toujours au premier plan ne sont pas disponibles sur GNOME Wayland sans extension du compositeur.

## Compatibilité distributions

| Distribution | Statut | Notes |
|--------------|--------|-------|
| Arch / CachyOS | Testé | |
| Ubuntu 22.04+ | Devrait fonctionner | `libgtk-4-dev`, CUDA dans `/usr/local/cuda` |
| Fedora 38+ | Devrait fonctionner | `gtk4-devel` |
| Debian 12+ | Devrait fonctionner | `libgtk-4-dev`, `gtk-update-icon-cache` optionnel |
| NixOS | Non testé | Chemin CUDA à configurer à la main |
