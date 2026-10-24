# ADR 0003 — Préfixer les symboles ggml de whisper pour le lier avec llama.cpp

[English](../../en/adr/0003-prefixed-ggml-symbols.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

`whisper-rs-sys` et `llama-cpp-sys-2` embarquent chacun leur copie de ggml, dans des versions différentes. Les lier dans un même binaire échoue : des centaines de symboles dupliqués (`ggml_backend_*`, `ggml-quants`). Forcer l'édition de liens avec `--allow-multiple-definition` aurait mélangé deux versions incompatibles et planté à l'exécution.

## Décision

`vendor/whisper-rs-sys` est une copie de la crate dont le `build.rs` modifie les archives statiques de whisper juste après leur compilation : `nm` liste les symboles définis, puis `objcopy --redefine-syms` ajoute le préfixe `pw_` à tous, **locaux compris**, sauf l'API publique `whisper_*` et les instanciations de la bibliothèque standard C++ (`std::`, `__gnu_cxx::`). La crate est branchée par `[patch.crates-io]`.

Deux pièges découverts en route :
1. Renommer les symboles de la bibliothèque standard faisait pointer un appel `std::string` de llama vers du vide (plantage avant `main`).
2. Les deux ggml définissent la même classe C++ dans un **groupe COMDAT** de même nom ; l'éditeur de liens gardait celui de whisper et jetait celui de llama. Il faut renommer aussi les symboles locaux qui servent de signature à ces groupes.

## Conséquences

- Un seul binaire ; testé sur x86_64 Linux, en CPU et en CUDA (Whisper `turbo` et Qwen dans le même processus).
- Le build exige `nm` et `objcopy` (binutils).
- Le dossier `vendor/` ajoute environ 14 Mo de sources whisper.cpp.
- Fragile aux mises à jour : après tout changement de `whisper-rs` ou `llama-cpp-2`, lancer `tests/e2e_models.rs`. Un conflit ne plante qu'à l'exécution, la compilation ne le voit pas.
