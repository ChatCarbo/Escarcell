# Compiler et lancer Escarcelle

## 1. Le cœur de calcul (aucune connexion nécessaire)

```sh
cargo fmt --check                                        # mise en forme
cargo clippy --all-targets -- -D warnings                # analyse statique
cargo test                                               # 69 tests
```

## 2. L'application de bureau

Prérequis : Rust 1.77 ou plus, Node.js 20 ou plus, et les dépendances système de Tauri v2
(sous Windows : WebView2, présent par défaut sur Windows 10 et 11, et les « Build Tools » de Visual Studio).

```sh
cd app
npm install
npm run tauri icon src-tauri/icons/logo.svg   # génère les icônes à partir du logo (une seule fois)
npm run tauri dev                             # lance l'application en mode développement
npm run tauri build                           # produit l'installateur
```

Tests propres à l'application (base SQLite, cours, versions) :

```sh
cd app && cargo test
```

## Intégration continue

À chaque envoi sur `main` et à chaque pull request, GitHub Actions (`.github/workflows/ci.yml`) vérifie :
- le cœur sous Linux : mise en forme, clippy sans avertissement, tests ;
- le cœur avec Rust 1.75, la version minimale annoncée ;
- l'application sous Windows : build de l'interface Svelte, mise en forme, clippy, tests.

Pensez à envoyer `app/package-lock.json` sur GitHub : la CI installe alors exactement les mêmes versions de paquets que vous.

## Données

Les données sont enregistrées dans le dossier de données de l'application
(sous Windows : `%APPDATA%\fr.chatcarbo.escarcelle\escarcelle.db`). Une sauvegarde = une copie de ce fichier.
