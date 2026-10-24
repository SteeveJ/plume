# ADR 0013 — Quatre niveaux de tests, vrais modèles et vrais bugs

[English](../../en/adr/0013-test-strategy.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Les défauts les plus graves de cette période ne se voyaient pas à la compilation : un conflit entre deux copies de ggml qui plante à l'exécution, du texte inventé par Whisper sur du bruit, un LLM qui recopie un suffixe ou supprime des passages. Des tests uniquement faits de doubles ne les auraient pas trouvés.

## Décision

Quatre niveaux, du plus rapide au plus réaliste :
1. **Unitaires** (`cargo test --lib`) : calculs purs, sans modèle ni matériel.
2. **Intégration avec faux matériel** (`config_load`, `pipeline`, `hallucinations`, `french`) : vrais adaptateurs (fichier de config réel dans un `HOME` temporaire, SQLite, bloc-notes), faux micro, clavier et Whisper.
3. **Bout en bout avec vrais modèles** (`e2e_models`, `e2e_french`, ignorés par défaut) : Whisper et Qwen dans un processus, vrai bruit, voix de synthèse française (Piper, `tests/fixtures/fr/`). Les tests qui chargent le LLM s'exécutent l'un après l'autre : llama.cpp ne s'initialise qu'une fois par processus.
4. **Mémoire réelle** : la RAM baisse vraiment au déchargement.

Règles : un test = un bug réellement rencontré ; on vérifie qu'il **échoue** quand on réintroduit le bug ; les seuils des tests de compréhension sont calibrés sur `turbo` et sur `small`.

## Conséquences

- Les tests avec vrais modèles se lancent à la main (gros fichiers) mais ont déjà attrapé des bugs que rien d'autre ne voyait (`[BLANK_AUDIO]`, `/nothink` recopié, LLM vide sur texte long).
- La voix de synthèse est plus propre qu'un vrai micro : les scores réels seront un peu moins bons.
- Le français est testé pour la compréhension : sens conservé, accents, nombres écrits en chiffres, détection automatique de la langue.
