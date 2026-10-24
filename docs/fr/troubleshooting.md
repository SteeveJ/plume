# Dépannage

Plume démarre sans terminal : les échecs s'affichent donc en notifications de bureau. Pour le journal complet, lancez `plume` dans un terminal (`RUST_LOG=debug plume`).

| Symptôme | Cause et solution |
|----------|-------------------|
| Rien ne se passe quand j'appuie sur la touche | Droit uinput ou input. Installez `assets/99-plume.rules`, rejoignez le groupe `input`, déconnectez-vous puis reconnectez-vous. Plume cherche un clavier toutes les 3 s : un clavier branché plus tard fonctionne |
| « Plume could not start: … uinput » | Idem |
| « Micro indisponible : … » | Pas de micro, ou il est occupé. Plume continue de tourner ; réglez le micro et appuyez à nouveau |
| « Aucun son — le micro est-il coupé ? » | L'audio était presque silencieux : micro coupé ou débranché |
| « Rien entendu — parlez plus près du micro » | Whisper n'a entendu que du bruit, ou une phrase inventée comme « Thank you. » a été filtrée |
| Un « merci » très bas est ignoré | N'arrive que si le fichier du détecteur de voix `ggml-silero-v5.1.2.bin` manque (il est téléchargé au premier lancement). Sans lui, sous le seuil de bruit, les quelques phrases inventées connues sont traitées comme du bruit |
| Le téléchargement semble bloqué au premier lancement | Le modèle vocal est gros (`small` 470 Mo, `turbo` 1,6 Go). Une notification l'annonce ; il s'arrête sur une erreur après 60 s sans données |
| « needs about … MB of free RAM » | Le `STT_MODEL` choisi ne tient pas. Prenez un modèle plus petit ou quantifié (`small-q8_0`, `large-v3-turbo-q5_0`) ou libérez de la mémoire |
| La transcription prend des secondes | Vous êtes sur CPU. Compilez avec `--features cuda` (NVIDIA). `turbo` sur CPU demande environ 4 s pour 11 s d'audio |
| Le texte n'est pas corrigé | Le modèle de 0,6B rend parfois le texte inchangé, et sur un texte long il peut ne pas répondre ; Plume garde alors vos mots bruts. Un LLM local plus gros (Qwen3-1.7B) a été testé et n'est pas meilleur ; essayez plutôt un `PROCESSOR` en ligne |
| Les réglages ne s'appliquent pas | Utilisez *Sauvegarder et relancer*, icône → *Reload config*, ou `plume --restart` |
| Pas assez d'espace disque | Plume vérifie l'espace libre avant un téléchargement et indique ce qu'il faut |

## Commandes utiles

```bash
plume --restart          # redémarre le démon
ls -l ~/.config/plume.conf ~/.local/share/plume/   # les fichiers doivent être -rw-------
nvidia-smi               # mémoire GPU utilisée par plume
```
