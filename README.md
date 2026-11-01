# EditShell

EditShell est un prototype Linux en Rust et GTK 4 qui associe un éditeur de commandes libre à de vraies sessions Bash et à un explorateur de fichiers intégré.

## État du prototype

- Texte de commande libre, modifiable à tout moment ; une édition ne lance jamais une commande.
- Prompt visuel `utilisateur@machine:répertoire $` au-dessus de l’éditeur, sans l’ajouter à la commande.
- Complétion avec `Tab` des commandes présentes dans `PATH` et des chemins du répertoire courant ; les espaces dans les chemins sont échappés.
- Coloration syntaxique de base en direct dans l’éditeur (commandes, mots-clés shell, chaînes, variables, commentaires et opérateurs).
- Exécution avec `Ctrl+Entrée` ou le bouton **Exécuter**.
- Après l’envoi réussi, l’éditeur est vidé et la commande est ajoutée à l’historique affiché avec ses résultats.
- Session Bash persistante par onglet, avec répertoire courant et variables exportées conservés.
- `exit` ferme l’onglet Bash concerné ; si c’est le dernier onglet, l’application se ferme.
- Exécution via pseudo-terminal Linux, sortie et code de retour associés à chaque exécution.
- `clear` efface la sortie ; `Ctrl+C` ou le bouton d’arrêt interrompt la commande au premier plan sans fermer Bash.
- Les couleurs ANSI des commandes sont rendues dans l’historique ; `ls` affiche les couleurs de fichiers et dossiers. L’activation et la palette (standard ou contraste élevé) se règlent dans **Préférences**.
- Dialogue de saisie masquée pour les invites de mot de passe reconnues ; champ distinct pour les réponses interactives ordinaires, comme la confirmation d’une nouvelle clé hôte SSH.
- Dialogue terminal VTE pour les applications plein écran ncurses, avec flèches, Entrée, redimensionnement et interruption clavier.
- Zone d’historique agrandie et redimensionnable avec séparateur ; le défilement suit automatiquement les nouvelles sorties.
- Explorateur de fichiers : navigation, raccourcis vers le dossier parent et personnel, actualisation, sélection puis insertion explicite d’un chemin cité.
- Menu contextuel au clic droit sur les fichiers et dossiers : insérer ou copier le chemin, naviguer dans un dossier depuis EditShell et ouvrir avec l’application ou l’explorateur de fichiers du système.
- Préférences persistantes pour le thème (système, clair, sombre), la taille de police, l’affichage des couleurs ANSI et la palette de sortie.
- Les commandes s’exécutent en arrière-plan par rapport à la boucle GTK.

Il s’agit toujours d’un prototype. Les dialogues plein écran utilisant les modes d’écran alternatifs du terminal sont rendus dans une fenêtre VTE interactive. La fermeture d’un onglet en cours d’exécution est empêchée jusqu’à la fin ou l’interruption de sa commande.

## Dépendances

- Rust et Cargo (édition 2024).
- Bash.
- GTK 4, VTE GTK 4 et leurs fichiers de développement pour compiler l’interface.

Sur Ubuntu, installer les dépendances système avec :

```sh
sudo apt install build-essential pkg-config libgtk-4-dev libvte-2.91-gtk4-dev
```

EditShell utilise GTK 4 et VTE GTK 4 pour afficher les applications de terminal interactives.

## Installer EditShell dans le menu des applications

Depuis la racine du dépôt, lancez :

```sh
./scripts/install.sh
```

Le script compile EditShell en mode release et installe le binaire dans `~/.local/bin`, le fichier `.desktop` dans le dossier d’applications utilisateur XDG et l’icône dans le thème `hicolor` de l’utilisateur. L’application apparaît alors dans le menu des applications ; vous pouvez aussi l’épingler au dock depuis ce menu. L’installation ne demande pas les droits administrateur. Une icône SVG est fournie dans `assets/`, donc aucune image supplémentaire n’est requise. Vous pouvez remplacer ce SVG par votre propre création en gardant son nom pour conserver l’intégration automatique.

Pour supprimer ces fichiers utilisateur, depuis le dépôt, lancez :

```sh
./scripts/uninstall.sh
```

Les scripts ne suppriment pas le dépôt, les paramètres d’EditShell ni les fichiers créés par l’utilisateur.

## Compiler et lancer

```sh
cargo run --features gui
```

Pour exécuter les tests du moteur et de la configuration sans dépendance de développement GTK :

```sh
cargo test
```

## Utilisation

1. Écrivez ou modifiez librement une ou plusieurs lignes dans l’éditeur.
2. Appuyez sur `Tab` pour compléter une commande ou un chemin. Pour plusieurs correspondances, EditShell complète le préfixe commun puis affiche des suggestions dans la barre d’état.
3. Lancez le contenu avec **Exécuter** ou `Ctrl+Entrée`.
4. La commande envoyée disparaît de l’éditeur et apparaît dans **Historique des commandes et résultats** avec sa sortie et son code de retour.
5. Dans l’éditeur, utilisez `Ctrl+↑` pour retrouver les commandes précédentes et `Ctrl+↓` pour avancer dans l’historique puis retrouver votre texte en cours.
6. Si une commande demande un mot de passe, un dialogue sécurisé s’ouvre ; la saisie n’est ni affichée ni ajoutée à l’historique. Pour une confirmation ordinaire (comme SSH `yes/no`), utilisez le champ **Session active** sous l’historique.
7. Faites glisser le séparateur horizontal entre l’éditeur et l’historique pour régler leur hauteur. L’historique défile automatiquement vers la sortie la plus récente.
8. Une commande SSH/FTP/SMB interactive reste au premier plan dans son onglet ; utilisez **Session active** pour envoyer les réponses ou commandes distantes. Pour garder un shell local disponible en parallèle, le bouton `+` ouvre un onglet Bash indépendant. `Ctrl+C` ou le bouton d’arrêt interrompt la commande au premier plan.
9. L’explorateur latéral permet de parcourir les dossiers ; double-cliquez sur un dossier pour l’ouvrir ou sélectionnez un élément puis utilisez **Insérer le chemin**. Faites un clic droit sur un élément pour afficher les actions adaptées aux fichiers ou dossiers.
10. Dans **Préférences**, activez ou désactivez les couleurs ANSI et choisissez la palette standard ou à contraste élevé. Les couleurs affichées sont celles émises par les commandes, par exemple `ls`.

Une commande échouée ne ferme pas la session. Chaque exécution produit une nouvelle entrée dans l’historique. Pour réutiliser une commande, recopiez-la depuis l’historique vers l’éditeur.

## Architecture

- `src/shell.rs` : session Bash, exécution, flux de sortie et tests du moteur.
- `src/config.rs` : préférences XDG et valeurs par défaut.
- `src/ui.rs` : fenêtre GTK 4, onglets, éditeur et explorateur.
- `src/main.rs` : point d’entrée de l’application graphique.

## Limites et prochaines étapes

Les applications qui n’activent pas le mode d’écran alternatif peuvent rester dans l’historique de sortie classique. La coloration syntaxique shell, les opérations de fichiers mutantes, la distribution Linux et la mise en paquet restent à réaliser.
# EditShell
