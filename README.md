<p align="center">
  <img src="docs/images/logo.svg" alt="Logo Escarcelle" width="120">
</p>

<h1 align="center">Escarcelle</h1>

<p align="center">
  <strong>Logiciel libre, gratuit et local pour suivre ses livrets réglementés et son PEA.</strong><br>
  Vos données restent sur votre ordinateur : pas de compte, pas de serveur Escarcelle, pas de connexion à votre banque.
</p>

<p align="center">
  <a href="https://github.com/ChatCarbo/escarcelle/actions/workflows/ci.yml"><img src="https://github.com/ChatCarbo/escarcelle/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
</p>

> Version 0.3 : stabilisation. Escarcelle suit les livrets réglementés et le PEA ; le compte-titres et le PEA-PME de la 0.2 ont été retirés.

## Ce que fait Escarcelle

- **Livrets réglementés** (Livret A, LDDS, LEP) : virements, soldes, plafonds, intérêts calculés par quinzaines, et conseils pour verser et retirer au bon moment, avec une frise colorée.
- **PEA** : recherche d'actions et d'ETF européens, saisie des lignes (quantité, PRU), cours automatiques, plus ou moins-values.
- **Dividendes** : dividende annuel, rendement, croissance, prochain détachement estimé, et dividende net après retenue à la source du pays.
- **Courbes et analyse technique** : ligne, chandeliers japonais, barres, Heikin Ashi, en journalier ou hebdomadaire ; volumes, moyennes mobiles, Bollinger, RSI, MACD, supports et résistances ; signaux expliqués en clair (cassures, figures, motifs de chandeliers, structure de tendance, confirmation hebdomadaire).
- **Alertes** : détachements de dividendes et assemblées générales des 45 prochains jours.
- **Tableau de bord** avec répartition de l'épargne et dividendes nets annuels.
- **Exports** en CSV (Excel) et en image PNG.
- **Sauvegarde, restauration**, effacement des données, et avis de nouvelle version au démarrage.

## Captures d'écran

### Tableau de bord
![Tableau de bord : patrimoine suivi, livrets, intérêts estimés, plus-value du PEA et répartition par enveloppe](docs/images/tableau-de-bord.png)

### Livrets réglementés
![Livrets : soldes, plafonds, nouveau virement et frise des quinzaines indiquant quand verser](docs/images/livrets.png)

### PEA
![PEA : lignes, plus-values, dividendes bruts et nets, rendement, croissance et fiche du titre](docs/images/pea.png)

### Analyse technique
![Courbe sur 5 ans avec moyennes mobiles, résistances, ligne du PRU, détachements de dividendes, volumes, RSI et MACD](docs/images/analyse-technique.png)

## Confidentialité et connexions réseau

Vos livrets, vos lignes, vos montants et vos réglages sont enregistrés dans un seul fichier sur votre ordinateur. Ils ne sont envoyés nulle part.

Escarcelle communique avec l'extérieur pour **lire** des données publiques (cours, dividendes), mais **ne transmet aucune de vos données**. Il se connecte seulement aux services suivants :

| Service | Quand | Ce qui est envoyé |
| :--- | :--- | :--- |
| Yahoo Finance | Sur la page PEA : recherche d'un titre, cours, dividendes, courbes | Le nom, l'ISIN ou le symbole du titre recherché ou affiché (par exemple `MC.PA`). Jamais vos quantités, PRU ni montants. |
| GitHub | Seulement si vous activez « Vérifier les nouvelles versions » dans Paramètres (**désactivé par défaut**), ou si vous cliquez sur « Vérifier maintenant » | Une demande de la liste des versions publiées. Aucune donnée d'Escarcelle. |

Comme pour toute connexion à Internet, ces services voient votre adresse IP. Les pages Livrets et Tableau de bord fonctionnent entièrement hors ligne.

## Documentation

- [Guide d'utilisation](docs/GUIDE.md)
- [Spécification](docs/SPEC.md)
- [Compiler et lancer](docs/COMPILER.md)
- [Campagne de vérification de la 0.3](docs/VERIFICATION.md)

## Structure

```
crates/escarcelle-core    Calculs (livrets, analyse technique, dividendes du PEA…), sans dépendance
app/                      Application de bureau : Tauri v2 (Rust) + Svelte, stockage SQLite
docs/                     Documentation
.github/                  Intégration continue (format, lint, tests) et modèle d'issue
```

## Développement

```sh
cargo fmt --check && cargo clippy --all-targets -- -D warnings
cargo test                       # cœur de calcul : 72 tests, sans réseau
cd app/src-tauri && cargo test   # application : 14 tests (livrets, base, migration, cours, versions)
cd .. && npm install && npm run tauri dev
```

Ces vérifications tournent automatiquement sur GitHub à chaque envoi (intégration continue).

Principes : montants en centimes, aucun flottant pour l'argent, chaque règle couverte par un test, aucun code mort. Les règles sont testées contre les textes officiels (BOFiP, service-public.fr) cités dans `tests/fiscalite.rs` et `tests/reference.rs`, et `tests/robustesse.rs` vérifie qu'aucun calcul ne plante sur des données vides ou mal formées.

Un problème ? Ouvrez une [issue](https://github.com/ChatCarbo/escarcelle/issues/new?template=probleme.md), avec si possible la source du bon résultat (relevé, service-public.fr, BOFiP).

## Avertissement

Escarcelle fournit des simulations à titre informatif. Il ne remplace ni un conseiller en investissements financiers (CIF) ni l'administration fiscale. L'analyse technique décrit le passé et ne constitue pas un conseil d'achat ou de vente.

## Licence

Copyright © 2026 ChatCarbo

Escarcelle est un logiciel libre distribué sous licence [GNU GPL v3.0 ou ultérieure](LICENSE). Vous pouvez l'utiliser, l'étudier, le modifier et le redistribuer. Toute version modifiée que vous distribuez doit rester sous la même licence, avec son code source.

Les cours de bourse affichés proviennent de Yahoo Finance et ne sont pas couverts par cette licence.
