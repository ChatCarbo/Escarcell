<p align="center">
  <img src="docs/images/logo.svg" alt="Logo Escarcelle" width="120">
</p>

<h1 align="center">Escarcelle</h1>

<p align="center">
  <strong>Logiciel libre, gratuit et local pour suivre ses livrets réglementés et son PEA.</strong><br>
  Vos données restent sur votre ordinateur : pas de compte, pas de serveur, pas de connexion à votre banque.
</p>

> Version 0.1 en test.

## Ce que fait Escarcelle

- **Livrets réglementés** (Livret A, LDDS, LEP) : virements, soldes, plafonds, intérêts calculés par quinzaines, et conseils pour verser et retirer au bon moment, avec une frise colorée.
- **PEA** : recherche d'actions et d'ETF européens, saisie des lignes (quantité, PRU), cours automatiques, plus ou moins-values.
- **Courbes et analyse technique** : moyennes mobiles, bandes de Bollinger, RSI, MACD, supports et résistances, signaux de rebond ou de cassure expliqués en clair.
- **Tableau de bord** avec répartition de l'épargne.
- **Sauvegarde, restauration** et effacement des données.

## Captures d'écran

### Tableau de bord
![Tableau de bord : patrimoine suivi, livrets, intérêts estimés, plus-value du PEA et répartition par enveloppe](docs/images/tableau-de-bord.png)

### Livrets réglementés
![Livrets : soldes, plafonds, nouveau virement et frise des quinzaines indiquant quand verser](docs/images/livrets.png)

### PEA
![PEA : lignes saisies avec quantité, PRU, cours, plus-value et courbe du titre](docs/images/pea.png)

### Analyse technique
![Courbe sur 5 ans avec moyennes mobiles, supports et résistances, ligne du PRU, RSI, MACD et signaux expliqués](docs/images/analyse-technique.png)

### Paramètres
![Paramètres : sauvegarder, restaurer une sauvegarde, tout effacer](docs/images/parametres.png)

## Documentation

- [Guide d'utilisation](docs/GUIDE.md)
- [Spécification](docs/SPEC.md)
- [Compiler et lancer](docs/COMPILER.md)

## Structure

```
crates/escarcelle-core    Calculs (livrets, analyse technique, PEA, impôt…), sans dépendance
crates/escarcelle-quotes  Chaîne de sources de cours
app/                   Application de bureau : Tauri v2 (Rust) + Svelte, stockage SQLite
catalogue/             Frais et conditions des établissements (versions suivantes)
docs/                  Documentation
```

## Développement

```sh
cargo test                 # cœur de calcul : 76 tests, sans réseau
cd app && npm install && npm run tauri dev
```

Principes : montants en centimes, aucun flottant pour l'argent, chaque règle couverte par un test.

## Avertissement

Escarcelle fournit des simulations à titre informatif. Il ne remplace ni un conseiller en investissements financiers (CIF) ni l'administration fiscale. L'analyse technique décrit le passé et ne constitue pas un conseil d'achat ou de vente.

## Licence

Copyright © 2026 ChatCarbo

Escarcelle est un logiciel libre distribué sous licence [GNU GPL v3.0 ou ultérieure](LICENSE). Vous pouvez l'utiliser, l'étudier, le modifier et le redistribuer. Toute version modifiée que vous distribuez doit rester sous la même licence, avec son code source.

Les cours de bourse affichés proviennent de Yahoo Finance et ne sont pas couverts par cette licence.
