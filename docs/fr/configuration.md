# Configuration

Plume lit `~/.config/plume.conf` (créé au premier lancement, mode `600`). La fenêtre de réglages (icône → *Paramètres*) modifie le même fichier et **conserve toutes les clés qu'elle n'affiche pas**. Les changements s'appliquent après `plume --restart` ou icône → *Reload config*.

Les clés inconnues et les valeurs invalides sont signalées par une notification au démarrage et dans le journal (par exemple `PROCESSOR=olama`).

## Clés

| Clé | Défaut | Signification |
|-----|--------|---------------|
| `HOTKEY` | `KEY_RIGHTCTRL` | Touche d'enregistrement : `KEY_RIGHTCTRL`, `KEY_LEFTCTRL`, `KEY_RIGHTALT`, `KEY_LEFTALT`, `KEY_RIGHTMETA`, `KEY_CAPSLOCK`, `KEY_SCROLLLOCK`, `KEY_PAUSE`, `KEY_INSERT`, `KEY_F1` … `KEY_F12` |
| `MODE` | `hold` | `hold` = enregistre tant que la touche est appuyée ; `toggle` = un appui démarre, un appui arrête |
| `STT_LANG` | vide | Langue parlée (`fr`, `en`, …). Vide = détection automatique |
| `STT_MODEL` | `small` | `tiny`, `base`, `small`, `medium`, `large-v2`, `turbo`, ou une variante quantifiée comme `small-q8_0`, `small-q5_1`, `large-v3-turbo-q5_0` |
| `STT_PROMPT` | vide | Mots que Whisper doit privilégier (noms, jargon) |
| `PROCESSOR` | `local` | `local` (Qwen3 0,6B), `openai`, `anthropic`, `ollama`, `none` |
| `PROCESSOR_STYLE` | `clean` | `clean` corrige seulement les erreurs. Les autres réécrivent et demandent un modèle en ligne : `casual`, `excited`, `formal`, `professional`, `diplomatic` (alias `soften`), `assertive`, `humor` |
| `INSTRUCT_HOTKEY` | vide (désactivé) | Seconde touche du **mode instruction** : sélectionnez un texte dans n'importe quelle appli, maintenez cette touche et dites quoi en faire (« rends ça plus formel », « traduis en anglais »). Demande `OPENAI_API_KEY` ou `ANTHROPIC_API_KEY` ; le texte sélectionné est envoyé à ce service. Mêmes valeurs que `HOTKEY`, différente de celle-ci |
| `OUTPUT` | `inject` | `inject` tape dans la fenêtre active ; `scratchpad` ajoute à `scratch.txt` et au presse-papiers ; `both` |
| `APP_LANG` | `en` | Langue des notifications et de la fenêtre de réglages (`en`, `fr`) |
| `OPENAI_API_KEY`, `OPENAI_PROCESSOR_MODEL` | — / `gpt-4o-mini` | Pour `PROCESSOR=openai`, les styles en ligne, et en secours pour la reconnaissance vocale |
| `ANTHROPIC_API_KEY`, `ANTHROPIC_MODEL` | — / `claude-haiku-4-5` | Pour `PROCESSOR=anthropic` |
| `OLLAMA_MODEL` | `qwen3:0.6b` | Pour `PROCESSOR=ollama` (serveur sur `localhost:11434`) |
| `MODEL_PATH`, `MODEL_URL`, `MODEL_SHA256`, `MODEL_REPO`, `MODEL_PATTERN` | `~/.local/share/plume/model.gguf`, —, —, `unsloth/Qwen3-0.6B-GGUF`, `Q4_K_M` | Où se trouve le LLM local et comment il est téléchargé |

Les variables d'environnement `OPENAI_API_KEY` et `ANTHROPIC_API_KEY` l'emportent sur le fichier.

## Choisir le modèle vocal

Mesuré sur 5 minutes d'audio, mémoire du processus au pic. Sur GPU les poids vont en VRAM et la RAM tombe à quelques centaines de Mo.

| `STT_MODEL` | Mémoire CPU | Notes |
|-------------|------------|-------|
| `small` | ~890 Mo | Bon choix par défaut |
| `small-q8_0` | ~680 Mo | Qualité presque identique |
| `small-q5_1` | ~610 Mo | Le plus léger des `small` |
| `turbo` | ~2,1 Go | Meilleure qualité ; utilisez un GPU |
| `large-v3-turbo-q8_0` | ~1,35 Go | |
| `large-v3-turbo-q5_0` | ~1,06 Go | |

Plume refuse un modèle qui ne tient pas dans la RAM libre et indique lequel choisir à la place. Comparez la qualité sur votre propre voix avant de garder un modèle quantifié.

## Fichiers dans `~/.local/share/plume/`

| Fichier | Contenu |
|---------|---------|
| `ggml-*.bin` | Modèle(s) Whisper, téléchargés à la demande |
| `ggml-silero-v5.1.2.bin` | Détecteur de voix (885 Ko), téléchargé avec le premier modèle Whisper |
| `model.gguf` | LLM local |
| `vocab.txt` | Mots donnés à Whisper. Les lignes commençant par `#` sont des commentaires. Noms et sigles s'ajoutent automatiquement |
| `snippets.txt` | Une règle par ligne : `phrase dite => texte`. `\n` fait un saut de ligne. La recherche ignore la casse (majuscules accentuées comprises), ne trouve que des mots entiers, phrase la plus longue d'abord. Appliqué après le LLM. Lu au démarrage |
| `history.db` | Chaque dictée (texte brut et final), les 10 000 dernières conservées, mode `600` |
| `scratch.txt` | Sortie de `OUTPUT=scratchpad`, mode `600` |

Le dossier de données est en mode `700`.
