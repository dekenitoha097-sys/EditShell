# Projet : Shell Linux interactif avec édition libre, développé en Rust et GTK 4

## 1. Rôle et mission

Tu es un ingénieur logiciel senior spécialisé dans :

- Rust et la programmation système.
- Linux et les systèmes Unix.
- GTK 4 et Libadwaita.
- Les shells Unix et la gestion des processus.
- Les interfaces graphiques natives.
- La conception d'applications de bureau modernes.
- L'ergonomie des outils destinés aux développeurs.

Ta mission est de concevoir et de développer un shell innovant destiné exclusivement à Linux.

Cette application doit combiner trois éléments :

1. Le fonctionnement d'un véritable shell Linux.
2. La liberté d'édition d'un éditeur de texte moderne.
3. Un explorateur de fichiers interactif intégré à l'interface.

L'application doit posséder une interface moderne, élégante, cohérente avec l'environnement GNOME et suffisamment aboutie pour être utilisée quotidiennement.

**La priorité absolue est de créer un véritable shell dans lequel l'utilisateur peut écrire, modifier et exécuter des commandes librement, comme dans un éditeur de texte, sans perdre les fonctionnalités d'un shell traditionnel.**

Ne développe pas une simple maquette visuelle. Le résultat doit être une application fonctionnelle, modulaire, performante et maintenable.

---

# 2. Vision du produit

Les shells traditionnels comme Bash, Zsh et Fish reposent principalement sur une ligne de saisie et un affichage textuel.

Mon projet conserve la puissance de ces outils, mais transforme complètement l'expérience utilisateur.

L'utilisateur doit pouvoir écrire ses commandes naturellement, déplacer librement son curseur, cliquer au milieu d'une ancienne commande et modifier le texte comme dans un éditeur de code.

Les résultats doivent rester visibles et associés à leurs commandes.

L'application doit également intégrer un explorateur de fichiers interactif permettant de naviguer dans l'arborescence Linux et d'effectuer des opérations sur les fichiers.

L'interface doit s'inspirer de la simplicité et de l'intégration native de GNOME Terminal, tout en proposant une expérience visuelle plus proche d'un éditeur de code moderne.

Il ne faut cependant pas reproduire simplement GNOME Terminal.

**L'innovation centrale est la transformation de la zone de commande traditionnelle en un espace d'édition libre, tout en conservant un véritable moteur de shell.**

---

# 3. Stack technique obligatoire

Le projet doit être développé en privilégiant les technologies natives de Linux.

## 3.1. Langage principal : Rust

Utiliser Rust pour :

- La logique applicative.
- Le moteur de gestion des commandes.
- La gestion des processus.
- La communication avec le shell sous-jacent.
- L'explorateur de fichiers.
- La gestion de l'historique.
- Les services internes.
- La gestion des paramètres.
- Les interactions entre les composants.

Rust doit être le langage principal du projet.

Ne remplace pas Rust par Python, JavaScript, TypeScript ou C++ pour implémenter le cœur de l'application.

Des composants natifs externes peuvent être utilisés lorsque cela est justifié, mais la logique principale doit rester en Rust.

## 3.2. Interface graphique : GTK 4

Utiliser GTK 4 pour construire l'interface graphique.

L'application doit être une véritable application de bureau native Linux.

L'interface ne doit pas être développée avec Electron, React dans un navigateur embarqué ou une technologie web utilisée comme substitut à GTK.

Privilégier les composants GTK natifs chaque fois que cela est possible.

Documentation : https://www.gtk.org/

## 3.3. Libadwaita

Utiliser Libadwaita lorsque cela apporte une réelle valeur à l'interface.

Libadwaita doit notamment contribuer à :

- L'apparence générale de l'application.
- La barre de titre.
- Les préférences.
- Les boîtes de dialogue.
- Les boutons et contrôles.
- Les vues de navigation.
- Les espacements et la cohérence visuelle.
- L'intégration avec GNOME.
- L'adaptation aux thèmes clair et sombre.

L'application doit respecter les conventions de conception des applications GNOME sans être limitée à une apparence générique.

Documentation : https://gnome.pages.gitlab.gnome.org/libadwaita/

## 3.4. Bibliothèques Rust

Privilégier les bibliothèques suivantes lorsque leur utilisation est appropriée et compatible avec les versions retenues.

| Besoin | Technologie privilégiée |
|---|---|
| Langage principal | Rust |
| Interface graphique | GTK 4 |
| Composants GNOME | Libadwaita |
| Bindings Rust pour GTK | gtk4-rs |
| Asynchronisme | Tokio, si nécessaire |
| Sérialisation | Serde |
| Configuration | TOML ou JSON |
| Journaux applicatifs | tracing |
| Tests | Cargo test |
| Gestion des erreurs | thiserror et anyhow selon le contexte |
| Intégration système | APIs Linux et bibliothèques Rust adaptées |

Documentation des bindings Rust : https://gtk-rs.org/

Ces technologies constituent une base de travail et non une obligation d'ajouter chaque bibliothèque.

Évite les dépendances inutiles.

Avant de choisir les versions des bibliothèques, vérifie leur compatibilité avec la version de GTK installée et les versions de Rust disponibles.

N'utilise pas de versions arbitrairement anciennes.

## 3.5. Moteur de shell

Pour la première version, privilégier Bash comme moteur d'exécution sous-jacent.

Le moteur doit fonctionner dans une véritable session persistante lorsque cela est nécessaire.

Il doit permettre de conserver correctement :

- Le répertoire courant.
- Les variables d'environnement.
- L'état de la session.
- Les commandes exécutées.
- Les processus actifs.
- Les codes de retour.

Étudier l'utilisation d'un pseudo-terminal Unix lorsque cela est nécessaire pour les programmes interactifs.

Il faut distinguer le moteur qui exécute les commandes de l'interface graphique qui permet de les écrire et d'en consulter les résultats.

Ne développe pas immédiatement un interpréteur Bash personnalisé.

L'objectif initial est de créer une excellente expérience utilisateur en s'appuyant sur un moteur de shell existant.

L'éventuel développement d'un interpréteur propriétaire pourra être étudié plus tard.

## 3.6. Terminal natif et programmes interactifs

Étudier la possibilité d'utiliser VTE pour les cas où un véritable terminal d'émulation est nécessaire.

Documentation : https://gitlab.gnome.org/GNOME/vte

VTE ne doit pas remplacer l'éditeur de texte principal.

Le composant VTE peut être utile pour les programmes interactifs qui nécessitent un terminal classique, tandis que l'interface principale reste centrée sur l'édition libre.

Évaluer soigneusement l'intégration de VTE avec GTK 4 et Rust avant de retenir cette solution.

Ne suppose pas qu'une bibliothèque est compatible avec GTK 4 simplement parce qu'elle est utilisée par GNOME Terminal.

Si VTE n'est pas adapté à un besoin précis, proposer une alternative native et documentée.

---

# 4. Plateforme cible : Linux uniquement

L'application est exclusivement destinée à Linux.

La première plateforme cible est Ubuntu avec GNOME.

Le projet doit néanmoins être conçu de manière à pouvoir fonctionner sur d'autres distributions Linux modernes lorsque leurs dépendances sont compatibles.

Ne cherche pas à assurer la compatibilité avec Windows ou macOS.

Ne développe pas de couche multiplateforme inutile.

Privilégier :

- Les APIs natives de Linux.
- Les mécanismes standard des shells Unix.
- Les conventions GNOME.
- Les chemins et permissions du système de fichiers Linux.
- Les mécanismes standards de gestion des processus.
- Les fonctionnalités disponibles dans GTK 4 et Libadwaita.

L'application ne doit pas nécessiter les privilèges root pour son utilisation normale.

Documenter clairement les dépendances système nécessaires à la compilation et à l'exécution.

---

# 5. Interface graphique moderne

L'interface est une composante essentielle du projet.

Elle doit être moderne, propre, élégante, cohérente et agréable à utiliser quotidiennement.

L'inspiration générale est GNOME Terminal, GNOME Console et les éditeurs de code modernes, sans reproduire exactement leur interface.

Il faut trouver un équilibre entre la simplicité d'un terminal et la richesse d'un environnement de développement.

## 5.1. Principes visuels

L'interface doit privilégier :

- Une hiérarchie visuelle claire.
- Des espacements cohérents.
- Une typographie lisible.
- Une police monospace de qualité pour les commandes.
- Des icônes cohérentes.
- Des animations discrètes.
- Des séparateurs subtils.
- Des panneaux redimensionnables.
- Des menus bien organisés.
- Des états visuels compréhensibles.
- Un affichage sans encombrement inutile.

Évite les effets visuels excessifs, les dégradés omniprésents, les bordures trop marquées et les interfaces qui ressemblent à des tableaux de bord web.

L'application doit conserver une identité visuelle propre.

Elle doit paraître suffisamment professionnelle pour être utilisée par des développeurs Linux.

## 5.2. Fenêtre principale

Prévoir une fenêtre principale composée des éléments suivants :

1. Une barre de titre moderne.
2. Une barre d'outils.
3. Une zone d'onglets.
4. Une zone principale contenant l'éditeur-shell.
5. Un explorateur de fichiers intégré.
6. Une barre d'état.
7. Des menus et des paramètres accessibles.

La disposition doit rester claire même lorsque plusieurs onglets sont ouverts.

L'explorateur doit pouvoir être masqué, redimensionné ou réorganisé sans perturber la zone de commande.

L'interface doit s'adapter aux dimensions de la fenêtre.

## 5.3. Barre de titre et barre d'outils

La barre supérieure doit permettre d'accéder facilement aux fonctions importantes.

Prévoir notamment :

- La création d'un nouvel onglet.
- L'accès aux onglets existants.
- L'ouverture des paramètres.
- L'affichage ou le masquage de l'explorateur.
- L'accès aux actions principales.
- Le menu général de l'application.

Ne surcharge pas la barre d'outils.

Les commandes les plus fréquentes doivent rester accessibles par des raccourcis clavier.

## 5.4. Onglets

L'application doit prendre en charge plusieurs onglets.

Chaque onglet doit disposer de sa propre session de travail.

Il doit pouvoir posséder :

- Une session de shell indépendante.
- Son propre historique.
- Son répertoire courant.
- Ses processus.
- Son contenu éditable.
- Ses résultats.
- Son état d'exécution.

Les onglets doivent permettre de passer rapidement d'un contexte de travail à un autre.

Prévoir :

- La création d'un onglet.
- La fermeture d'un onglet.
- Le changement d'onglet.
- Le renommage d'un onglet.
- L'affichage du répertoire courant dans son titre.
- Des raccourcis clavier.
- Une indication visuelle lorsqu'une commande est en cours d'exécution.

La fermeture d'un onglet contenant des processus actifs doit être gérée proprement.

Ne tue pas silencieusement des processus sans appliquer une politique de fermeture explicite.

L'architecture doit permettre d'envisager ultérieurement plusieurs panneaux ou des sessions divisées.

Cette fonctionnalité peut être ajoutée après la stabilisation de la première version.

## 5.5. Explorateur latéral

Prévoir un explorateur de fichiers intégré à la fenêtre principale.

Il doit être possible de l'afficher ou de le masquer.

Il doit également être possible de modifier sa largeur.

L'explorateur doit présenter une véritable arborescence de fichiers et de dossiers.

Les interactions doivent inclure :

- Le développement et le repli des dossiers.
- La sélection des fichiers.
- La navigation.
- Le menu contextuel.
- La création de fichiers.
- La création de dossiers.
- Le renommage.
- La copie et le déplacement.
- La suppression.
- La copie du chemin.
- L'insertion du chemin dans la zone de commande.
- L'actualisation de l'affichage.

L'explorateur doit respecter les permissions Linux.

Les opérations dangereuses doivent être clairement identifiées.

Il doit être possible de masquer l'explorateur pour disposer d'une zone de travail plus large.

## 5.6. Zone principale d'édition

La zone principale est le cœur du projet.

Elle doit fonctionner comme un espace de texte éditable et comme un shell.

L'utilisateur écrit ses commandes au clavier.

Il peut cliquer au milieu du texte pour modifier une commande, corriger un argument, sélectionner du texte ou insérer du contenu.

Il ne doit pas être obligé de sélectionner des commandes prédéfinies.

La saisie doit rester naturelle et familière.

Les résultats des commandes doivent être présentés de façon lisible et rester associés à leurs exécutions.

L'interface doit distinguer visuellement :

- Les commandes en cours d'édition.
- Les commandes en attente.
- Les commandes en cours d'exécution.
- Les commandes terminées.
- Les commandes ayant échoué.
- Les résultats.
- Les erreurs.

La distinction doit rester discrète et professionnelle.

Évite de transformer chaque ligne en carte volumineuse.

L'expérience principale doit rester celle d'un espace de travail fluide, éditable et proche d'un document.

## 5.7. Barre d'état

Prévoir une barre d'état affichant les informations utiles, par exemple :

- Le répertoire courant.
- Le shell utilisé.
- L'état de la session.
- Le nombre de processus actifs, si pertinent.
- L'état de la commande en cours.
- Les informations de contexte nécessaires.

Ne surcharge pas cette barre avec des informations techniques rarement utiles.

---

# 6. Système d'édition libre

L'édition libre est la principale innovation du projet.

Elle ne doit pas être sacrifiée pour simplifier l'interface.

## 6.1. Fonctionnement

L'utilisateur écrit une commande comme dans un terminal normal.

Exemple :

```bash
cd ~/Documents
mkdir mon_projet
cd mon_projet
touch main.rs
```

Il doit pouvoir revenir au milieu de la deuxième commande et la modifier.

Il doit également pouvoir sélectionner plusieurs caractères, utiliser les raccourcis clavier, copier, coller et naviguer librement.

L'édition ne doit jamais déclencher une commande automatiquement.

Une modification du texte ne doit pas être confondue avec une exécution.

## 6.2. Historique et résultats

Chaque exécution doit être identifiable.

Le système doit conserver :

- Le texte exécuté.
- La date de l'exécution.
- Le résultat.
- Les erreurs.
- Le code de retour.
- L'identifiant de l'exécution.

Une commande modifiée puis réexécutée doit créer une nouvelle entrée d'exécution.

L'ancienne exécution doit rester consultable.

Il faut éviter qu'une modification visuelle d'une commande change rétroactivement les résultats affichés.

## 6.3. Commandes multilignes

L'application doit prendre en charge les commandes qui s'étendent sur plusieurs lignes.

Exemple :

```bash
for fichier in *.txt; do
    echo "$fichier"
done
```

Le comportement de la touche Entrée doit être défini de manière cohérente.

Il faut distinguer la poursuite d'une commande incomplète, l'insertion d'une nouvelle ligne et l'exécution d'une commande complète.

Les règles doivent être documentées.

## 6.4. Coloration syntaxique

Prévoir une coloration syntaxique adaptée aux commandes shell.

Elle doit pouvoir différencier :

- Les commandes.
- Les options.
- Les chaînes de caractères.
- Les variables.
- Les redirections.
- Les opérateurs.
- Les commentaires.

La coloration doit rester correcte pendant l'édition.

Elle ne doit pas bloquer la saisie ni dégrader les performances sur de longues sessions.

La coloration syntaxique est une aide visuelle : elle ne doit pas servir à déterminer seule si une commande peut être exécutée.

---

# 7. Moteur de shell

L'application doit exécuter de véritables commandes Linux.

La première version peut s'appuyer sur Bash.

Le moteur doit conserver une session cohérente entre les commandes.

Il faut gérer correctement :

- Le répertoire courant.
- Les variables d'environnement.
- Les commandes internes.
- Les programmes externes.
- Les pipes.
- Les redirections.
- Les scripts.
- Les tâches en arrière-plan.
- Les processus interactifs.
- Les signaux.
- Les erreurs.
- Les codes de retour.
- Les flux de sortie.
- Les interruptions avec Ctrl+C.

Une commande longue ne doit pas bloquer l'interface graphique.

Les lectures et écritures liées aux processus doivent être gérées sans bloquer la boucle principale de GTK.

Étudier soigneusement la séparation entre les tâches d'arrière-plan, les événements GTK et la gestion des processus.

Ne suppose pas que toutes les commandes peuvent être exécutées de la même manière.

Certaines nécessitent un terminal interactif, d'autres se prêtent à une exécution avec des flux redirigés.

L'architecture doit prendre en compte ces différences.

---

# 8. Résultats interactifs

L'application doit pouvoir afficher certaines sorties sous une forme plus riche qu'un texte brut.

Cependant, cette fonctionnalité ne doit pas compromettre le fonctionnement du shell.

Exemples :

| Commande | Affichage possible |
|---|---|
| `ls` | Liste de fichiers |
| `tree` | Arbre interactif |
| `ps aux` | Tableau de processus |
| `df -h` | Tableau des partitions |
| `free -h` | Informations sur la mémoire |
| `git status` | Vue enrichie de l'état Git |

La sortie textuelle originale doit rester accessible.

Les commandes doivent continuer à fonctionner normalement dans les pipes et les redirections.

Commencer par quelques intégrations explicites.

Ne développe pas un système complexe de détection automatique avant que le moteur de shell et l'interface soient stables.

---

# 9. Thèmes et personnalisation

L'application doit proposer une véritable page de paramètres.

La personnalisation ne doit pas être limitée à un simple bouton qui inverse les couleurs.

## 9.1. Thèmes

Prévoir au minimum :

- Un thème clair.
- Un thème sombre.
- Un mode automatique qui suit les préférences du système.

Si cela reste raisonnable, permettre ultérieurement des thèmes supplémentaires.

L'application doit conserver une apparence cohérente dans toutes ses vues.

Le thème doit s'appliquer à la fenêtre, aux onglets, à l'explorateur, aux résultats et aux paramètres.

## 9.2. Personnalisation de l'éditeur

Prévoir des paramètres pour :

- La police monospace.
- La taille du texte.
- L'espacement des lignes.
- La coloration syntaxique.
- L'affichage de certaines informations.
- Le comportement de l'éditeur, lorsque cela est pertinent.

Les réglages doivent être persistants.

Ils doivent être conservés après la fermeture et le redémarrage de l'application.

## 9.3. Préférences générales

Prévoir des paramètres pour :

- Le thème.
- La police.
- Les raccourcis clavier.
- Le comportement des onglets.
- L'affichage de l'explorateur.
- Les préférences de démarrage.
- Le comportement de la touche Entrée.
- L'historique.
- Les confirmations d'actions destructrices.

N'ajoute pas des options qui n'ont aucun effet réel.

Chaque paramètre affiché doit être relié à une fonctionnalité effectivement implémentée.

## 9.4. Persistance

Utiliser un format de configuration adapté, par exemple TOML ou JSON.

Stocker la configuration dans un emplacement approprié aux applications Linux, en respectant les conventions XDG.

Prévoir des valeurs par défaut raisonnables et un comportement robuste lorsque le fichier de configuration est absent ou invalide.

---

# 10. Architecture logicielle

Le projet doit être modulaire.

Séparer clairement les responsabilités.

Une architecture initiale possible est :

```text
editshell/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── assets/
│   ├── icons/
│   └── styles/
├── data/
│   ├── desktop/
│   └── metainfo/
├── src/
│   ├── main.rs
│   ├── application.rs
│   ├── config/
│   │   ├── mod.rs
│   │   └── settings.rs
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── window.rs
│   │   ├── header_bar.rs
│   │   ├── tab_view.rs
│   │   ├── shell_editor.rs
│   │   ├── output_view.rs
│   │   ├── file_explorer.rs
│   │   ├── preferences.rs
│   │   └── dialogs.rs
│   ├── shell/
│   │   ├── mod.rs
│   │   ├── session.rs
│   │   ├── executor.rs
│   │   ├── process.rs
│   │   └── history.rs
│   ├── filesystem/
│   │   ├── mod.rs
│   │   ├── explorer.rs
│   │   └── operations.rs
│   ├── rendering/
│   │   ├── mod.rs
│   │   └── output_renderer.rs
│   └── utils/
│       └── mod.rs
└── tests/
```

Cette arborescence est une proposition de départ.

Tu peux la modifier si une autre structure est techniquement plus adaptée.

Ne crée pas des fichiers vides uniquement pour respecter cette structure.

Chaque module doit avoir une responsabilité clairement définie.

---

# 11. Sécurité et robustesse

L'application manipule de véritables commandes système et doit être conçue avec soin.

Respecter les règles suivantes :

- Ne jamais exécuter une commande simplement parce que le texte a changé.
- Ne pas lancer l'application en root par défaut.
- Respecter les permissions Linux.
- Éviter les injections de commandes dans les opérations internes de l'application.
- Ne pas construire des commandes shell à partir de chemins non échappés.
- Préférer les APIs système directes pour les opérations de fichiers.
- Ne pas supprimer silencieusement des fichiers.
- Gérer proprement les interruptions.
- Nettoyer les processus à la fermeture.
- Éviter les blocages de l'interface.
- Limiter les consommations de mémoire inutiles.
- Prévoir les sorties volumineuses.
- Gérer les erreurs de manière explicite.

L'application doit rester utilisable même si une commande échoue, si un processus se bloque ou si un fichier devient inaccessible.

---

# 12. Phases de développement

Développe le projet progressivement.

Ne cherche pas à implémenter toutes les fonctionnalités en une seule fois.

## Phase 1 : étude technique

Avant de coder :

1. Vérifier les versions compatibles de Rust, GTK 4, Libadwaita et des bibliothèques retenues.
2. Définir l'architecture.
3. Comparer les solutions d'édition de texte.
4. Définir la stratégie de gestion du shell.
5. Définir le modèle de données pour les commandes et leurs résultats.
6. Définir la structure des onglets.
7. Préparer un plan de tests.
8. Identifier les risques techniques.

Présenter les décisions importantes avant de passer à l'implémentation.

## Phase 2 : première application GTK

Créer une application Rust/GTK 4 fonctionnelle.

Elle doit comprendre :

- Une fenêtre principale.
- Une barre de titre.
- Une barre d'outils.
- Une zone d'onglets.
- Une zone de travail.
- Un panneau latéral.
- Une barre d'état.
- Une page de paramètres initiale.

L'objectif est d'établir une base d'interface propre.

Ne passe pas trop de temps sur les détails visuels avant que les composants essentiels fonctionnent.

## Phase 3 : éditeur-shell fonctionnel

Implémenter :

- L'édition libre.
- Le positionnement du curseur.
- La sélection.
- Les raccourcis.
- L'exécution des commandes.
- La récupération des sorties.
- La persistance du répertoire courant.
- Les erreurs et les codes de retour.

Tester les fonctionnalités avant d'ajouter des éléments décoratifs.

## Phase 4 : historique et résultats

Ajouter :

- La conservation des commandes exécutées.
- La possibilité de modifier une ancienne commande.
- La distinction entre les différentes exécutions.
- L'association des résultats.
- La gestion des commandes multilignes.

## Phase 5 : explorateur de fichiers

Implémenter :

- L'arborescence interactive.
- La navigation.
- Les menus contextuels.
- Les opérations sur les fichiers.
- L'insertion des chemins.
- La synchronisation avec le shell.

## Phase 6 : onglets et sessions

Ajouter :

- La création et la fermeture des onglets.
- Les sessions indépendantes.
- La gestion des processus.
- Les raccourcis.
- Les indicateurs d'activité.
- La fermeture propre des sessions.

## Phase 7 : thèmes et paramètres

Ajouter :

- Le thème clair.
- Le thème sombre.
- Le mode automatique.
- Les préférences d'édition.
- Les paramètres persistants.
- Les raccourcis personnalisables, si cette fonctionnalité est prévue.

## Phase 8 : robustesse et optimisation

Tester :

- Les commandes interactives.
- Les processus longs.
- Les commandes multilignes.
- Les pipes.
- Les redirections.
- Les interruptions.
- Les sorties volumineuses.
- Les permissions.
- La fermeture des onglets.
- Les erreurs de fichiers.
- Les performances de l'interface.

## Phase 9 : affichages enrichis

Ajouter progressivement des rendus interactifs pour les commandes les plus pertinentes.

Cette phase doit intervenir après la stabilisation du cœur du projet.

---

# 13. Critères de validation

Le projet ne doit pas être considéré comme fonctionnel uniquement parce que sa fenêtre s'affiche.

Les tests suivants doivent être réalisés.

### Test 1 : édition libre

Écrire une commande, cliquer au milieu, modifier un argument et vérifier que le texte peut être édité naturellement.

### Test 2 : exécution

Exécuter :

```bash
pwd
```

Vérifier que la sortie correspond au répertoire courant.

### Test 3 : session persistante

Exécuter :

```bash
cd /tmp
pwd
```

La deuxième commande doit afficher `/tmp`.

### Test 4 : commandes successives

Exécuter :

```bash
mkdir -p test_shell
cd test_shell
touch fichier.txt
ls -la
```

Vérifier le résultat et la persistance du contexte.

### Test 5 : historique éditable

Exécuter une commande, revenir la modifier, puis l'exécuter à nouveau.

Vérifier que les deux exécutions restent distinctes.

### Test 6 : explorateur

Créer un fichier, vérifier son apparition dans l'arborescence, sélectionner le fichier et insérer son chemin dans une commande.

### Test 7 : onglets

Ouvrir deux onglets et vérifier que leurs sessions de shell sont indépendantes.

### Test 8 : thèmes

Changer de thème et vérifier que le choix est appliqué à l'ensemble de l'interface et conservé après redémarrage.

### Test 9 : commandes multilignes

Exécuter une boucle shell valide sans déclencher prématurément son exécution.

### Test 10 : réactivité

Lancer une commande longue et vérifier que l'interface reste réactive.

### Test 11 : interruption

Interrompre une commande avec Ctrl+C et vérifier que l'application reste utilisable.

### Test 12 : compatibilité

Exécuter :

```bash
printf "alpha\nbeta\n" | grep beta
```

Vérifier que le pipeline fonctionne normalement.

---

# 14. Documentation et installation

Le projet doit inclure un README expliquant :

- La vision du projet.
- Les fonctionnalités.
- La stack technique.
- L'architecture.
- Les dépendances.
- L'installation sous Ubuntu.
- La compilation.
- Le lancement.
- Les tests.
- Les limitations.
- Les prochaines étapes.

Fournir des commandes de compilation exactes et adaptées aux dépendances effectivement utilisées.

Prévoir à terme une intégration de bureau Linux correcte, avec un fichier `.desktop`, une icône et les métadonnées nécessaires.

Étudier les possibilités de distribution adaptées à Linux, notamment les paquets natifs ou Flatpak, sans en faire une priorité avant que l'application soit fonctionnelle.

---

# 15. Consignes de développement pour l'agent

Commence par examiner l'environnement et le dépôt existant.

Si le projet existe déjà, analyse son architecture avant de modifier le code.

S'il n'existe pas, crée une structure initiale propre.

Respecte les consignes suivantes :

- Utiliser Rust comme langage principal.
- Utiliser GTK 4 pour l'interface.
- Utiliser Libadwaita lorsque cela est pertinent.
- Cibler exclusivement Linux.
- Ne pas utiliser Electron.
- Ne pas remplacer l'interface native par une application web.
- Ne pas simuler les commandes.
- Ne pas sacrifier l'édition libre pour simplifier l'interface.
- Ne pas lancer automatiquement les commandes pendant leur édition.
- Ne pas développer toutes les fonctionnalités simultanément.
- Compiler et tester régulièrement.
- Documenter les choix techniques.
- Corriger les erreurs avant de continuer.
- Éviter les dépendances inutiles.
- Ne pas annoncer une fonctionnalité comme terminée sans l'avoir vérifiée.

Si tu disposes d'un accès à un terminal de développement, utilise-le pour inspecter l'environnement, compiler le projet et lancer les tests.

Si une dépendance n'est pas disponible, identifie précisément le problème et propose une solution adaptée à Linux.

Ne modifie pas des configurations système ou n'installe pas de paquets avec des privilèges élevés sans nécessité.

---

# 16. Première tâche à exécuter

Commence par réaliser une étude technique du projet.

Ne génère pas immédiatement des centaines de lignes de code.

Présente :

1. La stack finale recommandée.
2. Les versions compatibles des technologies.
3. L'architecture générale.
4. La méthode de gestion du shell.
5. Le choix du composant d'édition de texte.
6. La stratégie d'intégration de l'explorateur.
7. La conception des onglets.
8. La stratégie de gestion des thèmes et des paramètres.
9. Les difficultés techniques majeures.
10. Le plan de développement du premier prototype.

Justifie les décisions importantes en tenant compte des contraintes de GTK 4, de Rust et de Linux.

Une fois l'étude terminée, commence l'implémentation de la première phase fonctionnelle.

**Objectif final : créer un shell Linux natif, moderne et réellement utilisable, qui combine la puissance d'un terminal classique, la liberté d'un éditeur de texte et un explorateur de fichiers interactif, avec une interface GTK 4 élégante et des onglets indépendants.**