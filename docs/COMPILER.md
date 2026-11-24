# Compiler et lancer Escarcelle

## 1. Le cœur de calcul (aucune connexion nécessaire)

```sh
cargo test                                               # 94 tests
cargo run -p escarcelle-core --example comparer_per      # comparaison des 5 PER
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

Tests propres à l'application (catalogue embarqué, base SQLite) :

```sh
cd app && cargo test
```

## État de ce code

Le dossier `app/` a été écrit sans pouvoir être compilé (pas d'accès réseau pour télécharger
tauri, rusqlite, serde et Svelte). Il faut s'attendre à quelques erreurs de compilation à corriger
au premier lancement : envoie-les telles quelles, elles se corrigent vite.

Les données sont enregistrées dans le dossier de données de l'application
(sous Windows : `%APPDATA%\fr.chatcarbo.escarcelle\escarcelle.db`). Une sauvegarde = une copie de ce fichier.
