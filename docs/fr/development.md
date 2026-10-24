# Développement

## Compiler

Prérequis : Rust, `cmake`, un compilateur C/C++, binutils (`nm`, `objcopy`) ; pour le GPU, le toolkit CUDA (définir `CUDA_PATH`) ou le SDK Vulkan.

```bash
cargo build --release                                           # CPU
CUDA_PATH=/opt/cuda CMAKE_CUDA_ARCHITECTURES=native \
  cargo build --release --features cuda                         # votre GPU seulement
```

Un build CUDA prend environ 11 minutes la première fois (deux copies des noyaux CUDA). Cargo ne voit pas un changement de `CMAKE_CUDA_ARCHITECTURES` : lancez `cargo clean -p llama-cpp-sys-2 --release` et supprimez `target/release/build/whisper-rs-sys-*` pour le forcer.

## Tests

| Commande | Ce qu'elle couvre | Il faut |
|----------|-------------------|---------|
| `cargo test --lib` | Tests unitaires : lecture de la config, snippets, filtres, calculs mémoire, touches, historique, redémarrage | rien |
| `cargo test --test config_load --test pipeline --test hallucinations --test french --test instruct` | Vrais adaptateurs avec faux micro, faux clavier et faux Whisper : fichier de config, chaîne de dictée complète, cas d'hallucination, accents et notifications en français, mode instruction | rien |
| `PLUME_TEST_GGML=… PLUME_TEST_GGUF=… [PLUME_TEST_VAD=…] cargo test --release --test e2e_models -- --ignored` | Vrais Whisper et Qwen dans un processus, bruit, silence. Avec `PLUME_TEST_VAD` (le fichier Silero) le bruit ne doit produire aucun texte | un modèle ggml, `model.gguf`, et si possible le détecteur de voix |
| `… cargo test --release --test e2e_french -- --ignored` | Compréhension du français parlé par Whisper (langue forcée et détectée) et par le LLM | idem |
| `PLUME_TEST_GGUF=… cargo test --release --test e2e_llm_eval -- --ignored --nocapture` | Note un LLM de correction sur 12 cas français (réponses vides, mots changés, phrases bien formées). Réussit pour Qwen3-0.6B, échoue pour le 1,7B | `model.gguf` |
| `PLUME_TEST_GGML=… cargo test --release --lib -- --ignored` | La RAM est libérée au déchargement et restaurée au préchauffage | un modèle ggml |

Ajoutez `--features cuda` pour lancer ceux qui utilisent de vrais modèles sur le GPU. Les tests avec vrais modèles attrapent ce que la compilation ne voit pas : un conflit de symboles entre les deux copies de ggml ne plante qu'à l'exécution.

Fixtures : `tests/fixtures/jfk.wav` (discours du domaine public) et `tests/fixtures/fr/*.wav` (six phrases françaises produites avec la voix Piper `fr_FR-siwis-medium` ; les textes de référence sont dans `tests/common/mod.rs`).

## Chaque test est un bug qui a existé

Chaque test d'intégration fige un vrai défaut : un changement de `PROCESSOR` par l'icône qui effaçait `PROCESSOR_STYLE`, une erreur de micro qui arrêtait le démon, `[BLANK_AUDIO]` tapé dans un document, un LLM qui recopiait `/nothink` dans le texte, un canal fermé qui annulait la transcription. Quand vous corrigez un bug, ajoutez d'abord son cas et vérifiez qu'il échoue sans le correctif.

## Tâches courantes

- **Nouvelle hallucination :** ajoutez la phrase (minuscules, sans ponctuation) à `INVENTED` dans `application/hallucination.rs` et un cas dans `tests/hallucinations.rs`. Après un changement de modèle vocal, lancez `e2e_models` : il affiche toute nouvelle phrase inventée.
- **Nouveau style :** ajoutez-le à `STYLES` et `style_prompt` dans `config.rs` et à la liste de la fenêtre de réglages.
- **Nouvelle décision :** copiez un ADR dans `docs/fr/adr/` **et** `docs/en/adr/` (même numéro), puis ajoutez-le aux deux index `README.md` et à `docs/README.md`.
- **Mise à jour de whisper ou llama :** lisez d'abord l'[ADR 0003](adr/0003-symboles-ggml-prefixes.md).

## Conventions

Le calcul lourd passe par `spawn_blocking`. Une erreur de micro, de modèle ou de réseau n'arrête jamais le démon : on notifie la cause et on attend l'appui suivant. Les fichiers qui contiennent des clés ou du texte dicté sont créés avec `config::write_private`.

**Les commentaires du code sont écrits en anglais** (commentaires de documentation, de tests et scripts de build compris). Le texte affiché à l'utilisateur n'est pas un commentaire : notifications et fenêtre de réglages passent par `t(en, fr, lang)` et existent en anglais et en français.
