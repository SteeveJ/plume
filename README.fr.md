# Plume — Dictée vocale

[English](README.md) | **Français**

Un démon de dictée vocale pour Linux. Maintenez (ou basculez) un raccourci clavier et parlez : Plume transcrit votre voix avec Whisper, nettoie le texte avec un petit LLM local, et le tape dans la fenêtre active. Tout tourne sur votre machine, dans un seul binaire, sans Python et sans compte en ligne.

## Fonctionnalités

- **Reconnaissance vocale avec whisper.cpp**, dans le processus, sur GPU (CUDA, Vulkan) ou CPU. La langue est détectée automatiquement ou fixée avec `STT_LANG`.
- **Correction par un LLM local** — Qwen3 0,6B via llama.cpp corrige la ponctuation, les majuscules et les erreurs évidentes de transcription sans changer vos mots.
- **Protection contre le texte inventé** — un détecteur de voix (Silero) tient le silence et le bruit loin de Whisper, avec en renfort un contrôle du volume, les balises « non-parole » de Whisper et une liste d'hallucinations connues comme « Thank you. ».
- **Snippets** — dites une phrase, obtenez un texte (`~/.local/share/plume/snippets.txt`).
- **Vocabulaire vivant** — noms et sigles sont appris dans `vocab.txt` et renvoyés à Whisper.
- **Sobre en mémoire** — Whisper est déchargé après 5 minutes d'inactivité et rechargé pendant que vous commencez à parler.
- **Icône dans la barre + fenêtre de réglages GTK4**, English / Français.
- **Mode instruction (facultatif)** — sélectionnez un texte n'importe où, maintenez une seconde touche et dites « rends ça plus formel » ou « traduis en anglais » ; un modèle en ligne réécrit la sélection sur place. Demande une clé OpenAI ou Anthropic et lui envoie le texte sélectionné.
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

## Sécurité et vie privée

Aucun logiciel ne peut promettre d'être sûr. Voici donc ce que Plume fait, ce qu'il ne fait pas, et ce que personne n'a encore vérifié.

**Ce qui reste sur votre machine.** Avec les réglages par défaut (`PROCESSOR=local` et aucune clé d'API), votre voix, vos textes et votre historique ne quittent jamais l'ordinateur : la reconnaissance et la correction tournent toutes les deux en local. Le démon n'écoute sur aucun port réseau et n'a aucun privilège Linux élevé. Votre fichier de configuration, l'historique et le bloc-notes ne sont lisibles que par votre utilisateur, et aucun message de son journal ne mentionne une clé d'API.

**Ce qui sort, et quand.**
- Les modèles, téléchargés une fois depuis Hugging Face en HTTPS.
- Avec `PROCESSOR=openai` ou `anthropic`, ou un style en ligne comme `formal` : le texte dicté, plus jusqu'à 2 000 caractères avant votre curseur (lus par l'accessibilité). Il n'existe pas encore d'interrupteur pour cette seconde partie.
- Avec le mode instruction : le texte sélectionné et votre instruction orale.
- Si le modèle vocal local ne se charge pas et qu'une clé OpenAI est définie, Plume se rabat sur l'API vocale d'OpenAI, et cela envoie votre audio.

**Points faibles connus.**
- Les modèles Whisper et du détecteur de voix sont téléchargés sans empreinte de contrôle, puis lus par du code C++. Un fichier trafiqué serait un vrai problème. Épingler les empreintes est la prochaine correction.
- Plume a besoin du groupe `input` pour voir votre touche et pour taper. Ce groupe n'est pas propre à Plume : n'importe quel programme que vous lancez peut s'en servir pour lire les frappes ou en injecter. Plume, lui, ne regarde que la touche que vous avez choisie.
- Les clés d'API sont dans `plume.conf` en clair, et `history.db` garde vos 10 000 dernières dictées de la même façon. Les deux sont en mode 600 : si la machine est perdue, c'est le chiffrement du disque qui les protège. Le texte dicté reste aussi dans le presse-papiers, où un gestionnaire peut le conserver.
- Rien ne cloisonne le processus.

**Pas encore vérifié.** Les quelque 400 dépendances n'ont jamais été passées dans `cargo audit` ni `cargo deny`. La copie de whisper.cpp dans `vendor/` ne se met pas à jour toute seule. Et personne en dehors du projet n'a relu le code. Pour signaler une faille, voir [SECURITY.fr.md](SECURITY.fr.md).

## Documentation

Les guides complets sont dans [`docs/`](docs/README.md) : [configuration](docs/fr/configuration.md), [architecture](docs/fr/architecture.md), [développement et tests](docs/fr/development.md), [dépannage](docs/fr/troubleshooting.md), et les [décisions d'architecture (ADR)](docs/fr/adr/README.md).

## Limites connues

- Le LLM de 0,6B corrige environ quatre phrases sur six ; sur un texte long il peut ne pas répondre et Plume garde alors votre texte brut. Un modèle plus grand (Qwen3-1.7B) a été essayé et n'est pas meilleur avec les réglages actuels (voir l'ADR 0004).
- La protection contre le bruit repose sur un détecteur de voix testé seulement sur du bruit de synthèse (blanc, bourdonnement, claquements) ; une télé ou une voix lointaine peut être prise pour de la parole.
- Seul CUDA a été testé sur GPU ; le build Vulkan n'est pas testé.
- Il n'y a ni overlay flottant, ni bulle d'état, ni fenêtre « hub » : GNOME Wayland ne peut pas garder une fenêtre au premier plan sans extension, le retour passe donc par l'icône de la barre et les notifications (ADR 0015).

## Compatibilité distributions

| Distribution | Statut | Notes |
|--------------|--------|-------|
| Arch / CachyOS | Testé | |
| Ubuntu 22.04+ | Devrait fonctionner | `libgtk-4-dev`, CUDA dans `/usr/local/cuda` |
| Fedora 38+ | Devrait fonctionner | `gtk4-devel` |
| Debian 12+ | Devrait fonctionner | `libgtk-4-dev`, `gtk-update-icon-cache` optionnel |
| NixOS | Non testé | Chemin CUDA à configurer à la main |
