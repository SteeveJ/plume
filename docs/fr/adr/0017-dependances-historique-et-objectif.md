# ADR 0017 — Dépendances : historique, état actuel et objectif « prêt à l'emploi »

[English](../../en/adr/0017-dependencies-history-and-goal.md) | **Français**

- Statut : objectif accepté, mise en œuvre en cours
- Date : 2026-10-25
- Complète : ADR 0002, 0003, 0011

## Contexte

La règle de l'utilisateur : Plume s'installe et fonctionne sans rien installer à côté, pas même une bibliothèque. Voici comment la situation a évolué.

| Quand | Dépendance | Ce qui s'est passé |
|-------|-----------|--------------------|
| Départ | Python, `openai-whisper`, `torch`, `pip` | Serveur Whisper persistant (1 à 2 Go de RAM). Remplacé par whisper.cpp dans le processus (ADR 0002). |
| Départ | `sh`, `pkill`, `sleep`, `which`, `touch` | Redémarrage et installation passaient par le shell. Remplacés par `plume --restart` (lecture de `/proc`, ADR 0012) et du Rust pur. |
| Opus | `libopus` système | D'abord liée dynamiquement ; compilée depuis les sources et liée statiquement après la règle d'indépendance (ADR 0011). Cassée sur Fedora (`lib64`), réparée par la copie vendorisée de `audiopus_sys`. |
| Deux ggml | symboles dupliqués | Copie vendorisée de `whisper-rs-sys` qui préfixe les symboles (ADR 0003). Demande `nm` et `objcopy` à la compilation. |
| CUDA | toolkit CUDA | Lié statiquement ; à l'exécution, seul le pilote NVIDIA est nécessaire (ADR 0011). |
| Test en conteneur | `libgomp` (OpenMP) | Le binaire ne démarrait pas sans elle. llama.cpp est compilé sans OpenMP. |
| Aujourd'hui | GTK4 (+ ~100 bibliothèques), libdbus, libasound, libsqlite3 | Encore liées. Objet de cet ADR. |

**État mesuré le 2026-10-25** sur six distributions propres : le binaire lie encore `libgtk-4` (et la centaine de bibliothèques qu'elle entraîne : cairo, pango, gstreamer, cups, avahi…), `libdbus-1`, `libasound` et `libsqlite3`, en plus de la libc, de libstdc++ et de libgcc_s. Elles viennent des crates `gtk4` (fenêtre de réglages), `dbus` (`notify-rust` en mode `d` et `ksni` 0.2), `alsa` (`cpal`) et `libsqlite3-sys` (`rusqlite`).

**Pourquoi cela n'a pas été vu plus tôt.** Le test `ldd` de la matrice de distributions tournait dans un conteneur où j'avais d'abord installé ces quatre paquets d'exécution. Il vérifiait donc que les bibliothèques installées suffisent, pas qu'aucune n'est nécessaire. La documentation les présentait même comme des prérequis normaux.

## Décision

L'objectif est **zéro bibliothèque système à l'exécution**, hors la libc et les bibliothèques de base du compilateur. Le critère de vérification change : `ldd` sur une image propre où **aucun paquet d'exécution n'est installé**.

Moyens décidés :
- `rusqlite` avec la feature `bundled` (SQLite compilé dans le binaire) ;
- `notify-rust` avec la feature `z` (client D-Bus en Rust pur, `zbus`) ;
- `ksni` 0.3 (également `zbus`) pour l'icône de la barre d'état.

Moyens **à décider par l'utilisateur** :
- la fenêtre de réglages : en `egui` (Rust pur, bibliothèques graphiques chargées seulement à l'ouverture, présentes sur tout bureau), sans fenêtre (réglages par l'icône et le fichier `plume.conf`), ou GTK4 en feature optionnelle éteinte par défaut ;
- l'audio : garder `libasound` (l'API audio de Linux) ou écrire un client PulseAudio/PipeWire en Rust pur (crate `pulseaudio`), qui exclut les systèmes sans serveur de son.

## Conséquences

- Le binaire grossit un peu (SQLite environ 1 Mo ; `egui` environ 10 Mo).
- Réécrire l'icône de la barre ne se vérifie pas en conteneur : il faut le bureau réel.
- Un enregistreur audio en Rust pur serait un nouveau code, à tester avec un vrai micro.
- Restent hors périmètre : la libc, le pilote NVIDIA (builds GPU), et les modèles téléchargés au premier lancement (réseau nécessaire une fois).
- Tant que ce n'est pas fait, la documentation ne doit pas présenter ces bibliothèques comme normales.
