# Politique de sécurité

[English](SECURITY.md) | **Français**

## Signaler une faille

N'ouvrez pas de ticket public pour un problème de sécurité. Utilisez plutôt le signalement privé de GitHub : sur la page du dépôt, ouvrez l'onglet **Security** puis choisissez **Report a vulnerability**.

Un bon signalement dit ce que vous avez trouvé, comment le reproduire, et ce que gagne un attaquant. Le commit testé (`git rev-parse HEAD`) et les lignes utiles de votre `plume.conf` aident beaucoup. Retirez vos vraies clés d'API et tout ce que vous avez dicté et voulez garder privé.

Plume est maintenu par une seule personne. Comptez environ une semaine pour une première réponse. Les corrections prennent le temps qu'elles prennent, et tout ce qui expose des clés, du texte dicté ou la machine passe en premier. Une fois le correctif publié, le signalement peut l'être aussi, et je vous cite si vous le souhaitez.

## Versions prises en charge

Plume n'a pas encore de version publiée : seul le dernier commit de `main` reçoit des corrections.

## Ce qui compte

Dans le périmètre :
- La gestion des clés d'API, du fichier de configuration, de l'historique et du bloc-notes (droits, fuites, journaux).
- Tout ce qui fait envoyer à Plume plus de données à un service en ligne que ce que dit le README.
- Les téléchargements de modèles et les fichiers que Plume en charge.
- L'installation, l'assistant de redémarrage, et l'usage de `uinput` et du groupe `input`.
- Le script de build de `vendor/whisper-rs-sys/`, qui réécrit les archives de whisper.cpp.

Hors périmètre :
- Les compromis déjà listés dans « Points faibles connus » du [README](README.fr.md#sécurité-et-vie-privée) : le groupe `input` est plus large que Plume, les secrets sont en clair en mode 600, et le processus n'est pas cloisonné. Si vous pouvez aggraver l'un d'eux, c'est dans le périmètre.
- Les problèmes qui exigent root ou un accès physique à la machine.
- Les bugs de whisper.cpp, de llama.cpp ou des modèles eux-mêmes. Signalez-les en amont, et dites-moi si la façon dont Plume les utilise aggrave les choses.
- Whisper qui invente du texte sur du bruit. C'est un défaut de qualité, suivi dans les tests, sauf s'il fait fuiter des données.

## Tester chez soi

Testez sur votre propre machine et vos propres comptes. Ne dirigez pas Plume vers les données ou les services de quelqu'un d'autre.
