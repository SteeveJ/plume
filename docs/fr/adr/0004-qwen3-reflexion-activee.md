# ADR 0004 — Qwen3 garde sa réflexion activée, sans suffixe `/nothink`

[English](../../en/adr/0004-qwen3-thinking-enabled.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

Le prompt de correction ajoutait ` /nothink` au message de l'utilisateur. Qwen3 ne reconnaît pas cette commande (la vraie s'écrit `/no_think`) : le modèle réfléchissait quand même, et recopiait parfois `/nothink` dans le texte corrigé, qui aurait été tapé dans le document. Le gabarit officiel pour couper la réflexion est un bloc `<think></think>` vide au début de la réponse.

## Décision

Mesure sur quatre phrases françaises et anglaises avec le prompt de production :

| Mode | Résultat |
|------|----------|
| Bloc `<think>` vide (réflexion coupée) | texte rendu inchangé presque à chaque fois (35 à 260 ms) |
| Réflexion activée | majuscules, accents et ponctuation corrigés (~0,5 s sur GPU) |

Décision : réflexion activée, aucun suffixe, le bloc `<think>…</think>` est retiré de la sortie. Budget de 1024 tokens ; si la réflexion ne se termine pas, la sortie est vide.

## Conséquences

- Corrections nettement meilleures, au prix de 0,5 s sur GPU (plusieurs secondes sur CPU).
- Sur un texte long le petit modèle peut tourner en boucle dans sa réflexion (« The user's name is "nous" which is correct » répété) et ne jamais répondre. Une sortie vide ne doit jamais effacer la dictée : voir l'ADR 0005.
- **Un modèle plus gros a été essayé et écarté pour l'instant.** Douze cas français (six phrases courtes, trois paragraphes longs, chacun dégradé puis déjà ponctué), sur GPU :

  | Modèle et mode | Bien formées | Vides | Mots changés | Latence |
  |----------------|--------------|-------|--------------|---------|
  | 0,6B, réflexion (actuel) | 8/12 | 0 | **0** | ~620 ms |
  | 0,6B, sans réflexion | 6/12 | 0 | 1 (perd « jeudi ») | ~90 ms |
  | 1,7B, réflexion | 3/12 | 7 | 2 | ~1,9 s |
  | 1,7B, sans réflexion | 8/12 | 0 | 2 (ajoute des mots) | ~120 ms |

  Le 1,7B réfléchit si longtemps qu'il épuise les 1024 tokens et ne répond rien. Sans réflexion il est cinq fois plus rapide pour le même nombre de phrases bien formées, mais il ajoute des mots, ce qui contredit « préserver les mots exacts ». À relancer avec `tests/e2e_llm_eval.rs` avant de changer de modèle ou de prompt.
