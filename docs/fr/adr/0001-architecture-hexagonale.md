# ADR 0001 — Architecture hexagonale (ports et adaptateurs)

[English](../../en/adr/0001-hexagonal-architecture.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Plume relie du matériel (micro, clavier virtuel, presse-papiers), des modèles et des services en ligne. On doit pouvoir tester toute la chaîne de dictée sans micro, sans GPU et sans réseau.

## Décision

La logique vit dans `application/` et ne dépend que de traits définis dans `domain/ports/` (`AudioRecorder`, `SpeechToText`, `TextProcessor`, `TextInjector`, `HotkeyListener`, `HistoryStore`, `Notifier`, `ScratchpadWriter`, `ContextReader`). Chaque matériel ou service est un adaptateur dans `adapters/`. `DictationService` est générique sur le micro, la reconnaissance vocale et l'injecteur.

## Conséquences

- Les tests remplacent le micro, le clavier et Whisper par des faux (`mockall` pour les unitaires, structures simples dans `tests/common`) : voir l'ADR 0013.
- Changer de moteur (OpenAI, whisper.cpp, Ollama…) ne touche pas la logique.
- Coût : une indirection de plus, et des `Box<dyn …>` pour les composants facultatifs.
- Les noms doivent dire la vérité : `RdevListener` et `EnigoInjector` n'utilisaient ni `rdev` ni `enigo`, ils sont devenus `EvdevListener` et `ClipboardInjector`.
