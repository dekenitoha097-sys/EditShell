# EditShell

EditShell est un prototype Linux en Rust et GTK 4 qui associe un éditeur de commandes libre à de vraies sessions Bash et à un explorateur de fichiers intégré.

## État du prototype

- Texte de commande libre, modifiable à tout moment ; une édition ne lance jamais une commande.
- Exécution avec `Ctrl+Entrée` ou le bouton **Exécuter**.
- Session Bash persistante par onglet, avec répertoire courant et variables exportées conservés.
- Sortie et code de retour associés à chaque exécution.
- Explorateur de fichiers : navigation par double-clic, actualisation et insertion d’un chemin cité dans l’éditeur.
- Préférences persistantes pour le thème (système, clair, sombre) et la taille de police.
- Les commandes s’exécutent en arrière-plan par rapport à la boucle GTK.

Il s’agit d’un premier prototype : les programmes nécessitant un terminal interactif (éditeur plein écran, invite de mot de passe, etc.) ne sont pas encore pris en charge. La prochaine étape pour ces cas est une intégration PTY/VTE évaluée séparément. La fermeture d’un onglet en cours d’exécution n’est pas encore proposée.

## Dépendances

- Rust et Cargo (édition 2024).
- Bash.
- GTK 4 et ses fichiers de développement pour compiler l’interface.

Sur Ubuntu, installer les dépendances système avec :

```sh
sudo apt install build-essential pkg-config libgtk-4-dev
```

Le prototype utilise GTK 4 directement ; Libadwaita et VTE ne sont pas encore ajoutés, car leur intégration ne fait pas partie de cette première tranche fonctionnelle.

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
2. Lancez le contenu avec **Exécuter** ou `Ctrl+Entrée`.
3. La sortie, le code de retour et le répertoire courant sont affichés sous l’éditeur.
4. Le bouton `+` ouvre une session Bash indépendante.
5. L’explorateur latéral permet de parcourir les dossiers ; double-cliquez sur un dossier pour l’ouvrir ou sélectionnez un élément pour insérer son chemin shell-quoté.

Une commande échouée ne ferme pas la session. Une nouvelle exécution produit un nouvel enregistrement de sortie ; l’éditeur reste disponible afin de permettre la modification et la réexécution.

## Architecture

- `src/shell.rs` : session Bash, exécution, flux de sortie et tests du moteur.
- `src/config.rs` : préférences XDG et valeurs par défaut.
- `src/ui.rs` : fenêtre GTK 4, onglets, éditeur et explorateur.
- `src/main.rs` : point d’entrée de l’application graphique.

## Limites et prochaines étapes

Le moteur actuel utilise des pipes, pas un pseudo-terminal. Il convient aux commandes et scripts non interactifs (pipes, redirections, commandes multilignes et processus longs), mais pas aux applications terminales interactives. La coloration syntaxique shell, la confirmation lors de la fermeture d’un processus, les opérations de fichiers mutantes, la distribution Linux et la mise en paquet restent à réaliser.
# EditShell
