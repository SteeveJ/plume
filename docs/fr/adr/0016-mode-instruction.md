# ADR 0016 — Mode instruction : réécrire la sélection avec un modèle en ligne

[English](../../en/adr/0016-instruction-mode.md) | **Français**

- Statut : accepté
- Date : 2026-10-24

## Contexte

On veut sélectionner un texte dans n'importe quelle application, dire « rends ça plus formel » ou « traduis en anglais », et obtenir le texte réécrit à la place. Le petit modèle local (Qwen3-0,6B) reformule trop mal : il change ou perd des mots (ADR 0004 et 0005). Il faut un modèle en ligne. Lire le texte sélectionné pouvait se faire de trois façons : simuler Ctrl+C (dangereux : dans un terminal c'est SIGINT et cela interrompt le programme en cours), lire la sélection via l'accessibilité AT-SPI (non vérifié sur cette machine), ou lire la sélection « primaire » du bureau (celle du clic milieu). Essai sur la machine de développement (COSMIC, Wayland) : la sélection primaire se lit avec `arboard`, sans rien taper.

## Décision

- Une seconde touche `INSTRUCT_HOTKEY`, **désactivée par défaut**, toujours en maintien et différente de `HOTKEY` (sinon elle est ignorée avec un avertissement).
- Elle exige `OPENAI_API_KEY` ou `ANTHROPIC_API_KEY` (OpenAI d'abord). Sans clé, Plume prévient et le mode reste désactivé.
- Déroulé : à l'appui, lecture de la sélection primaire (refusée si vide ou de plus de 20 000 caractères), la notification montre le début du texte lu ; enregistrement ; même chaîne d'écoute que la dictée (volume, détecteur de voix, filtre d'hallucinations) ; le modèle reçoit `Instruction: …` et `Text: …` avec la consigne `INSTRUCT_PROMPT` ; le résultat est collé par-dessus la sélection avec le même injecteur que la dictée.
- Réponse vide ou erreur du modèle : rien n'est collé, le texte de l'utilisateur reste intact, une notification explique pourquoi. L'historique garde `[instruction] …` et le résultat.
- Le mode commande (« supprime le dernier paragraphe ») est **écarté** : un ordre mal compris pourrait effacer du texte, et il demande une liste d'actions à concevoir.

## Conséquences

- **Confidentialité :** le texte sélectionné et l'instruction sont envoyés au fournisseur choisi. C'est dit dans la configuration par défaut et dans la doc.
- La sélection primaire garde le dernier texte sélectionné même si l'application l'a désélectionné depuis : l'aperçu dans la notification et Ctrl+Z servent de filet.
- Pas de garde-fou contre la perte de mots (ADR 0005) : une réécriture peut légitimement raccourcir ou traduire.
- Testé avec de faux composants (micro, sélection, modèle) et avec le vrai historique SQLite ; **jamais testé contre l'API réelle**, ni sur une application réelle.
- Une seconde touche demande une touche libre : éviter Alt Gr (caractères) et les touches déjà prises par le bureau.
