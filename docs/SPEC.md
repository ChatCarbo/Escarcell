# Escarcelle — Spécification

> Version du document : 6 — 23/11/2026
> Auteur : ChatCarbo — Licence GPL-3.0-or-later
> Ce document décrit la **V1** telle qu'elle est développée, puis ce qui est prévu ensuite.

---

## 1. Présentation

Escarcelle est un logiciel de bureau **libre, gratuit et local** pour suivre son épargne en France.

| Principe | Conséquence |
| :--- | :--- |
| Local | Toutes les données restent dans un fichier sur l'ordinateur de l'utilisateur. Aucun compte, aucun serveur. |
| Simple | On saisit ce qu'on détient. Pas d'établissement bancaire à renseigner, pas d'agrégation bancaire. |
| Exact | Les montants sont calculés en nombres entiers (centimes), jamais en nombres à virgule flottante. |
| Explicable | Chaque calcul et chaque conseil est accompagné de son explication en clair. |
| Honnête | Escarcelle ne perçoit rien d'aucun établissement et ne donne pas de conseil en investissement. |

Mention affichée en permanence : *« Simulations à titre informatif. Ne remplace ni un conseiller (CIF) ni l'administration fiscale. »*

---

## 2. Périmètre de la V1

| Écran | Contenu |
| :--- | :--- |
| **Tableau de bord** | Patrimoine suivi, total des livrets, intérêts estimés de l'année, plus-value latente du PEA, camembert par enveloppe, liste des comptes, alertes |
| **Livrets** | Livret A, LDDS, LEP : virements, solde, plafond, intérêts, conseils de dates, répartition entre livrets |
| **PEA** | Lignes (titre, quantité, PRU), recherche de titres, cours automatiques, plus ou moins-values, courbe et analyse technique |
| **Paramètres** | Sauvegarder, restaurer, tout effacer |

**Hors V1** (code partiellement écrit, voir §8) : impôt et dons, PER, assurance-vie, compte-titres, import de relevés, comptes courants, livrets non réglementés.

---

## 3. Tableau de bord

| Indicateur | Calcul |
| :--- | :--- |
| Patrimoine suivi | Somme des soldes des livrets + valeur du PEA |
| Livrets réglementés | Somme des soldes des livrets |
| Intérêts estimés cette année | Intérêts calculés par quinzaines jusqu'au 31 décembre, en supposant qu'aucun autre virement n'aura lieu |
| Plus-value latente PEA | Valeur des lignes − (quantité × PRU) |

Camembert par enveloppe (Livret A, LDDS, LEP, PEA), au plus 6 parts, couleurs fixes. Alertes : cours manquant, opération incohérente.

---

## 4. Livrets réglementés

### 4.1 Règles appliquées

| Règle | Détail |
| :--- | :--- |
| Livrets gérés | Un Livret A, un LDDS et un LEP au plus (un par personne) |
| Plafonds de versement | Livret A 22 950 € · LDDS 12 000 € · LEP 10 000 € |
| Plafond | Un versement est refusé si `solde + versement > plafond`. Seuls les intérêts capitalisés peuvent faire dépasser le plafond. |
| Taux | Fixés par l'État, révisables au 1er février et au 1er août. Au 01/08/2026 : Livret A et LDDS 1,70 %, LEP 2,50 %. |
| Fiscalité | Intérêts exonérés d'impôt et de prélèvements sociaux |
| Quinzaines | L'année compte 24 quinzaines : du 1er au 15 et du 16 à la fin du mois. |
| Versement | Rapporte à partir du **1er jour de la quinzaine suivante** |
| Retrait | Cesse de rapporter dès le **1er jour de la quinzaine en cours** |
| Changement de taux | Chaque quinzaine est payée au taux en vigueur à son premier jour |
| Intérêts | Calculés par quinzaine, arrondis au centime une fois par an, ajoutés au solde le 31 décembre |

Calcul : `intérêts(quinzaine) = max(0, solde rémunéré) × taux annuel / 24`.

### 4.2 Saisie

L'utilisateur indique qu'il possède un livret (« J'ai un Livret A »), puis saisit ses virements : sens (versement ou retrait), livret, date de valeur, montant. Contrôles : plafond pour un versement, solde pour un retrait. Historique par livret avec suppression en deux clics.

### 4.3 Conseils de dates

Dès que l'utilisateur choisit une date et un montant, Escarcelle affiche un message et une frise colorée.

| Cas | Message | Calcul |
| :--- | :--- | :--- |
| Versement | « Versé le 10 mars, l'argent rapporte à partir du 16 mars. Vous pouvez attendre jusqu'au 15 mars sans rien perdre. » | Date limite = dernier jour de la quinzaine en cours (le 15 ou le dernier jour du mois) |
| Retrait en cours de quinzaine | « Retiré le 14 mars, ce montant ne rapporte plus depuis le 1er mars. Si vous pouvez attendre le 16 mars, vous gagnez 1,70 €. » | Gain = montant × taux / 24 |
| Retrait le 1er ou le 16 | « Bonne date : la quinzaine précédente est entièrement payée. » | — |

**Frise** : environ six semaines autour de la date choisie, un rectangle par jour.

| Couleur | Signification |
| :--- | :--- |
| Vert | L'argent rapporte |
| Orange | Versement en attente, ne rapporte pas encore |
| Rouge | Quinzaine perdue pour le montant retiré |
| Gris | Argent hors du livret |
| ▼ | Date choisie |
| ★ | Date limite (versement) ou date conseillée (retrait) |

Rappel affiché : les dates sont des **dates de valeur** ; un virement fait un vendredi 15 peut être daté du lundi 17, donc prévoir un ou deux jours ouvrés de marge.

### 4.4 Répartition entre livrets

- **Versement** : remplir d'abord le livret le mieux rémunéré (LEP), puis les autres, dans la limite des plafonds ; le montant qui dépasse tous les plafonds est signalé.
- **Retrait** : puiser d'abord dans le livret le moins rémunéré.

---

## 5. PEA

### 5.1 Principe

L'utilisateur saisit ce qu'il détient : **titre, quantité, PRU** (le prix de revient affiché par sa banque). Escarcelle récupère les cours et calcule les gains. Il n'y a ni établissement ni journal d'opérations en V1.

### 5.2 Recherche de titres

Recherche par nom, mnémonique ou ISIN, résultats au fil de la frappe (après 300 ms sans frappe).

| Critère | Règle |
| :--- | :--- |
| Types | Actions et ETF |
| Places | Cotations en euros : Paris, Amsterdam, Bruxelles, Lisbonne, Milan, Madrid, Xetra, Francfort, Helsinki, Vienne, Dublin, Athènes. Paris est affiché en premier. |
| Éligibilité PEA « probable » | Action dont l'ISIN désigne un pays de l'Espace économique européen, ou action cotée sur une place de la zone euro hors Allemagne |
| « à vérifier » | ETF (l'éligibilité dépend de sa composition) et cotations allemandes (beaucoup de sociétés étrangères y sont cotées) |
| « non » | ISIN d'un émetteur hors EEE |

L'éligibilité est **indicative** ; l'utilisateur la vérifie auprès de sa banque. Les cotations hors euro (Stockholm, Copenhague, Oslo) ne sont pas gérées en V1.

### 5.3 Valorisation

| Valeur | Calcul |
| :--- | :--- |
| Investi | quantité × PRU |
| Valeur | quantité × dernier cours (ou × PRU si aucun cours n'est connu) |
| Plus ou moins-value | valeur − investi, en euros et en % |

Totaux en haut de page, camembert de répartition des lignes. Modification de la quantité et du PRU, suppression en deux clics.

### 5.4 Cours

- **Source V1 : Yahoo Finance**, sans clé, depuis la machine de l'utilisateur, pour son seul affichage (aucune rediffusion).
- Mise à jour à l'ouverture de la page PEA, puis toutes les 5 minutes tant qu'elle est ouverte, ou à la demande (« Actualiser les cours »).
- Cours en général **différés d'environ 15 minutes**. L'heure du cours est toujours affichée.
- Seules les cotations en euros sont acceptées.
- Si un titre n'est pas trouvé : saisie manuelle du cours.
- Le module `escarcelle-quotes` prévoit une chaîne de sources avec bascule automatique ; Boursorama et Euronext seront ajoutées devant Yahoo.

### 5.5 Courbe et analyse technique

Ouverte en cliquant sur une ligne ou sur un résultat de recherche.

**Courbe** : cours de clôture quotidiens, sur 3 mois, 6 mois, 1 an, 2 ans ou 5 ans ; ligne du PRU de l'utilisateur ; au survol, date, cours et RSI. Les indicateurs sont calculés sur un historique plus long que la période affichée (2 ou 5 ans), pour que la moyenne 200 jours soit disponible.

**Indicateurs** (affichables ou masquables) :

| Indicateur | Paramètres | Lecture |
| :--- | :--- | :--- |
| Moyennes mobiles | 20, 50 et 200 séances | Tendance courte, moyenne et de fond |
| Bandes de Bollinger | 20 séances, ± 2 écarts-types | Excès de hausse ou de baisse, compression |
| Supports et résistances | Sommets et creux locaux (± 5 séances), regroupés à 2 %, au moins 2 contacts ; 3 plus proches de chaque côté | Niveaux où le cours a déjà rebondi ou buté |
| RSI | 14 séances (méthode de Wilder) | Au-dessus de 70 : surachat ; en dessous de 30 : survente |
| MACD | 12, 26, 9 | Croisement de la ligne de signal : changement d'élan |

**Signaux détectés** :

| Signal | Condition | Sens |
| :--- | :--- | :--- |
| Tendance de fond | Cours et moyenne 50 j au-dessus (ou au-dessous) de la moyenne 200 j | Haussier / baissier |
| Croisement doré / de la mort | La moyenne 50 j croise la 200 j dans les 10 dernières séances | Haussier / baissier |
| RSI en surachat / survente | RSI ≥ 70 / ≤ 30 | Baissier / haussier |
| Hors des bandes de Bollinger | Clôture au-dessus de la bande haute / sous la bande basse | Baissier / haussier |
| Compression des bandes | Bandes les plus resserrées depuis 6 mois | Neutre (mouvement fort probable, sens inconnu) |
| Croisement du MACD | Dans les 5 dernières séances | Haussier / baissier |
| Cassure de résistance / de support | Clôture à plus de 1 % au-delà d'un niveau qu'elle n'avait pas franchi 5 séances plus tôt ; **confirmée** si le volume moyen des 5 dernières séances dépasse 1,5 fois celui des 20 précédentes | Haussier / baissier |
| Rebond possible sur support | Cours à moins de 3 % au-dessus d'un support, RSI < 45, dernière séance en hausse | Haussier |

**Synthèse** : « plutôt à la hausse » si les signaux haussiers dépassent les baissiers d'au moins 2, « plutôt à la baisse » dans le cas inverse, sinon « pas de tendance technique nette ».

**Avertissement affiché sous chaque analyse** : l'analyse technique décrit le comportement passé du cours, ne prédit pas l'avenir de façon fiable, beaucoup de signaux se révèlent faux ; ce n'est pas un conseil d'achat ou de vente.

---

## 6. Paramètres

| Action | Fonctionnement |
| :--- | :--- |
| Sauvegarder | Copie cohérente de toute la base dans un fichier choisi par l'utilisateur (`escarcelle-AAAA-MM-JJ.db`) |
| Restaurer | Vérifie que le fichier est une sauvegarde Escarcelle, demande confirmation, remplace les données, met le schéma à jour si la sauvegarde est ancienne |
| Tout effacer | Supprime toutes les données après saisie du mot EFFACER ; le schéma est conservé |

L'emplacement du fichier de données est affiché (sous Windows : `%APPDATA%\fr.chatcarbo.escarcelle\escarcelle.db`).

---

## 7. Données et architecture

### 7.1 Unités

| Grandeur | Unité interne |
| :--- | :--- |
| Montants | centimes (`i64`) |
| Taux | millionièmes (1,70 % = 17 000) |
| Cours, PRU, quantités | millionièmes |
| Indicateurs techniques | nombres à virgule (`f64`) : ce sont des statistiques, pas des montants |

Les conversions depuis la saisie (« 1 234,56 ») se font une seule fois, à l'entrée.

### 7.2 Paramètres réglementaires embarqués

| Paramètre | Valeurs (date d'effet) |
| :--- | :--- |
| Livret A et LDDS | 1,70 % (01/08/2025) · 1,50 % (01/02/2026) · 1,70 % (01/08/2026) |
| LEP | 2,70 % (01/08/2025) · 2,50 % (01/02/2026) |
| Plafonds | 22 950 € · 12 000 € · 10 000 € |

À mettre à jour à chaque révision des taux (1er février, 1er août) dans `crates/escarcelle-core/src/params.rs`. La mise à jour par fichier signé est prévue (§8).

### 7.3 Stockage

Un seul fichier SQLite, schéma versionné et migré automatiquement à l'ouverture.

| Table | Contenu |
| :--- | :--- |
| `comptes` | Livrets détenus (type, date d'ouverture) |
| `operations` | Virements des livrets (date, sens, montant en centimes) |
| `positions` | Lignes du PEA (ISIN, symbole de cotation, nom, quantité, PRU) |
| `cours` | Dernier cours par titre et par jour (cours, heure, source) |
| `parametres` | Réglages |

### 7.4 Organisation du code

```
crates/escarcelle-core     Calculs, sans dépendance ni accès réseau :
                        livret, conseil_livret, analyse, pea, courtage,
                        impot, per, dons, comparateur, strategie, params
crates/escarcelle-quotes   Chaîne de sources de cours (bascule, pause, cohérence, fraîcheur)
app/src-tauri           Application Tauri v2 : stockage SQLite, cours Yahoo, commandes
app/src                 Interface Svelte 5
catalogue/              Frais et conditions des établissements (pour les versions suivantes)
docs/                   Spécification, guide d'utilisation, compilation
```

Commandes de l'interface V1 : `tableau_de_bord`, `livrets`, `ouvrir_livret`, `conseil_livret`, `repartir_livrets`, `ajouter_operation`, `supprimer_operation`, `positions`, `chercher_titres`, `ajouter_position`, `modifier_position`, `supprimer_position`, `actualiser_cours`, `saisir_cours`, `analyse_titre`, `emplacement_donnees`, `sauvegarder`, `restaurer`, `tout_effacer`.

### 7.5 Qualité

- `cargo test` à la racine : 76 tests du cœur de calcul (livrets, quinzaines, conseils, analyse technique, impôt, dons, PEA, PER, courtage, comparateur, cours), dont des cas officiels (service-public.fr, BOFiP, brochures Fortuneo et BoursoBank).
- `cargo test` dans `app/` : lecture du catalogue, base SQLite, sauvegarde et restauration, reconnaissance des ISIN.
- À faire avant publication : comparer les intérêts calculés avec un relevé de livret réel.

---

## 8. Feuille de route

| Version | Contenu | État du code |
| :--- | :--- | :--- |
| **V1** | Tableau de bord, Livrets, PEA (lignes, cours, analyse technique), Paramètres | En test |
| V1.1 | Sources de cours Boursorama et Euronext devant Yahoo ; mise à jour des taux par fichier signé ; écran Aide intégré | Chaîne de sources écrite |
| V2 | **Impôt et dons** : TMI, décote, plafonnement, réduction pour dons (75 % jusqu'à 2 000 €, 66 % dans la limite de 20 % du revenu) | Calculs et écran écrits, retirés du menu |
| V2 | **PER** : intérêt d'un versement selon la TMI actuelle et à la retraite, versement jusqu'au bas de tranche, comparateur des contrats | Calculs écrits |
| V3 | Assurance-vie, compte-titres, cotations hors euro, import CSV des relevés | Catalogue des frais prêt |

---

## 9. Sources officielles

- Taux de l'épargne réglementée au 01/08/2026 : https://www.service-public.gouv.fr/particuliers/actualites/A15818
- Plafonds de versement, règles des quinzaines : https://www.service-public.gouv.fr/particuliers/vosdroits/F2365
- Réduction d'impôt pour dons : https://www.economie.gouv.fr/particuliers/reduction-impot-don-associations
- Éligibilité au PEA (titres de sociétés de l'UE et de l'EEE) : https://www.service-public.gouv.fr/particuliers/vosdroits/F2385
