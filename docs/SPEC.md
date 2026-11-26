# Escarcelle — Spécification

> Version du document : 13 — 25/11/2026 — version 0.4 en cours (lot 1 : sauvegarde chiffrée ; livrets : intérêts cumulés et maximaux)
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
| **PEA** | Lignes (titre, quantité, PRU), recherche de titres, cours automatiques, plus ou moins-values, dividendes nets, assemblées générales, courbe et analyse technique |
| **Paramètres** (bouton en haut à droite de la fenêtre) | Sauvegarder, restaurer, tout effacer, retenues à la source par pays, vérification des nouvelles versions |

**Compte-titres et PEA-PME** : retirés en 0.3 pour stabiliser l'application. Leur code (titres en devises, flat tax, crédit d'impôt conventionnel, liste Euronext du PEA-PME) a été supprimé et reste consultable dans l'historique git (étiquette `v0.2.0`). À l'ouverture d'une base de la 0.2, les lignes de ces enveloppes sont supprimées (§8.3).

**Exports** : chaque écran de chiffres s'exporte en **CSV** (tableau de bord, historique des livrets, lignes du PEA) ; les graphiques s'exportent en **image PNG** (courbe d'un titre avec ses indicateurs, répartition du tableau de bord). Le numéro de version est affiché sous le menu.

**Hors V1** (voir §9) : impôt et dons, PER, assurance-vie, import de relevés, comptes courants, livrets non réglementés. Aucun code n'est conservé pour ces fonctions : le code écrit pour l'impôt, les dons et le PER a été retiré en préparant la 0.3 et reste consultable dans l'historique git (étiquette `v0.2.0`).

---

## 3. Tableau de bord

| Indicateur | Calcul |
| :--- | :--- |
| Patrimoine suivi | Somme des soldes des livrets + valeur du PEA |
| Livrets réglementés | Somme des soldes des livrets |
| Intérêts estimés cette année | Intérêts calculés par quinzaines jusqu'au 31 décembre, en supposant qu'aucun autre virement n'aura lieu |
| Plus-value latente | Valeur des lignes du PEA − (quantité × PRU) |
| Dividendes nets par an | Somme des dividendes annuels nets du PEA (§5.6) |

Camembert par enveloppe (Livret A, LDDS, LEP, PEA), couleurs fixes. Bloc **« À venir (45 jours) »** : détachements de dividendes estimés et assemblées générales (§6).

---

## 4. Livrets réglementés

### 4.1 Règles appliquées

| Règle | Détail |
| :--- | :--- |
| Livrets gérés | Un Livret A, un LDDS et un LEP au plus (un par personne) |
| Plafonds de versement | Livret A 22 950 € · LDDS 12 000 € · LEP 10 000 € |
| Plafond | Un versement est refusé si `solde à sa date de valeur + versement > plafond`. Pour des virements du même jour, l'ordre de saisie ne compte pas : un versement tient compte des retraits du jour, un retrait des versements du jour. Le solde d'un jour comprend les virements jusqu'à ce jour et les intérêts capitalisés aux 31 décembre précédents : un versement daté de 2021 se juge sur le solde de 2021, pas sur celui d'aujourd'hui. Seuls les intérêts capitalisés peuvent faire dépasser le plafond. |
| Taux | Fixés par l'État, révisables au 1er février et au 1er août. Au 01/08/2026 : Livret A et LDDS 1,70 %, LEP 2,50 %. Historique embarqué depuis le 01/02/2020 (§8.2). |
| Fiscalité | Intérêts exonérés d'impôt et de prélèvements sociaux |
| Quinzaines | L'année compte 24 quinzaines : du 1er au 15 et du 16 à la fin du mois. |
| Versement | Rapporte à partir du **1er jour de la quinzaine suivante** |
| Retrait | Cesse de rapporter dès le **1er jour de la quinzaine en cours** |
| Changement de taux | Chaque quinzaine est payée au taux en vigueur à son premier jour |
| Intérêts | Calculés par quinzaine, arrondis au centime une fois par an, ajoutés au solde le 31 décembre |
| Solde de départ | Solde lu sur un relevé, intérêts compris (type d'opération `VALORISATION`). Un seul par livret, daté au plus tôt du 01/02/2020 ; aucune opération ne peut être datée avant lui. Daté d'un 1er ou d'un 16, il rapporte dès cette quinzaine ; sinon à partir de la suivante, comme un versement (un solde au 31 décembre comprend déjà les intérêts de l'année et ne doit pas rapporter une seconde fois la quinzaine du 16 au 31 décembre). Il n'est pas soumis au plafond (un livret ancien peut le dépasser grâce aux intérêts capitalisés). Il remplace l'historique antérieur : pas besoin des taux ni des virements d'avant. **Historique normalisé** : le solde de départ qui fait foi est le plus ancien (à date égale, le plus petit montant, jamais l'ordre de saisie) ; les opérations datées avant lui et les autres soldes de départ (par exemple dans une base restaurée) sont écartés du solde comme des intérêts, et signalés comme anomalies. |

Calcul : `intérêts(quinzaine) = max(0, solde rémunéré) × taux annuel / 24`.

Chaque carte de livret affiche les **intérêts maximaux de l'année** (livret resté au plafond du 1er janvier au 31 décembre : en 2026, Livret A 367,20 €, LDDS 192,00 €, LEP 251,67 €), les **intérêts cumulés** depuis la première opération (intérêts capitalisés + estimation de l'année en cours) et, dépliable, le **détail par année** : taux moyen de l'année (moyenne des taux des 24 quinzaines), intérêts, solde au 31 décembre.

### 4.2 Saisie

L'utilisateur indique qu'il possède un livret (« J'ai un Livret A »), puis saisit ses virements : sens (versement ou retrait), livret, date de valeur, montant. Contrôles (côté application, à la date de valeur) : montant positif, plafond pour un versement, solde pour un retrait, aucune opération déjà saisie rendue impossible par un virement antidaté ou par une suppression, date entre le 01/02/2020 (début de l'historique des taux) et un an après aujourd'hui. Pour un livret plus ancien, on commence par un **solde de départ** : en pratique, le solde au 31 décembre du dernier relevé annuel, daté du 1er janvier suivant. Historique par livret avec suppression en deux clics.

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

- **Versement** : remplir d'abord le livret le mieux rémunéré (LEP), puis les autres, dans la limite des plafonds ; à taux égal, le livret au plus grand plafond d'abord (Livret A avant LDDS) ; le montant qui dépasse tous les plafonds est signalé.
- **Retrait** : puiser d'abord dans le livret le moins rémunéré ; à taux égal, le livret au plus petit plafond d'abord (LDDS avant Livret A), pour garder plein celui qui peut contenir le plus.

---

## 5. PEA

### 5.1 Principe

L'utilisateur saisit ce qu'il détient : **titre, quantité, PRU** (le prix de revient affiché par sa banque, frais de courtage inclus). Escarcelle récupère les cours et les dividendes, puis calcule les gains. Il n'y a ni établissement ni journal d'opérations.

| Règle | Détail |
| :--- | :--- |
| Titres | Sociétés de l'Union européenne et de l'EEE, cotées en euros ; un titre coté dans une autre devise est refusé |
| Plus-value | Pas d'impôt tant que l'argent reste dans le plan |
| Dividendes français | Encaissés sans impôt |
| Dividendes étrangers | Retenue à la source du pays, **perdue** (aucun impôt français sur lequel l'imputer) |
| Plafond | 150 000 € de versements (non contrôlé : Escarcelle suit des lignes, pas les versements) |
| Saisie | Quantité positive (dix millions de titres au plus), PRU entre 0 et 1 000 000 € |

### 5.2 Recherche de titres

Recherche par nom, mnémonique ou ISIN, résultats au fil de la frappe (après 300 ms sans frappe).

| Critère | Règle |
| :--- | :--- |
| Types | Actions et ETF |
| Places | Cotations en euros : Paris, Amsterdam, Bruxelles, Lisbonne, Milan, Madrid, Xetra, Francfort, Helsinki, Vienne, Dublin, Athènes. Paris est affiché en premier. |
| Éligibilité PEA « probable » | Action dont l'ISIN désigne un pays de l'Espace économique européen, ou action cotée sur une place de la zone euro hors Allemagne |
| « à vérifier » | ETF (l'éligibilité dépend de sa composition) et cotations allemandes (beaucoup de sociétés étrangères y sont cotées) |
| « non » | ISIN d'un émetteur hors EEE |

L'éligibilité est **indicative** (affichée seulement sur l'écran PEA) ; l'utilisateur la vérifie auprès de sa banque.

Symbole de cotation : lettres, chiffres, point, tiret, `^`, `=` ou `_`, 20 caractères au plus ; tout autre symbole est refusé avant d'interroger Yahoo Finance.

### 5.3 Valorisation

| Valeur | Calcul |
| :--- | :--- |
| Investi | quantité × PRU |
| Valeur | quantité × dernier cours (ou × PRU si aucun cours n'est connu) |
| Plus ou moins-value | valeur − investi, en euros et en % |
| Variation sur 1 an | dernier cours / clôture de la séance du même jour un an plus tôt (ou de la séance précédente) − 1, d'après l'historique journalier |

Totaux en haut de page, camembert de répartition des lignes. Modification de la quantité, du PRU et du pays de la société, suppression en deux clics.

### 5.4 Cours

- **Source V1 : Yahoo Finance**, sans clé, depuis la machine de l'utilisateur, pour son seul affichage (aucune rediffusion).
- Yahoo Finance fournit aussi l'historique et les dividendes. Il n'y a pas de source de secours.
- Un cours absent, nul, négatif ou non numérique est ignoré. Un cours sans heure de cotation est daté du moment de la récupération.
- Mise à jour à l'ouverture de la page PEA, puis toutes les 5 minutes tant qu'elle est ouverte, ou à la demande (« Actualiser les cours »).
- Les dividendes et le cours d'il y a un an sont récupérés au plus une fois par jour et par titre : une requête mensuelle sur 10 ans pour les dividendes, une requête journalière sur 2 ans pour le cours d'il y a un an.
- Cours en général **différés d'environ 15 minutes**. L'heure du cours est toujours affichée.
- Seules les cotations en euros sont acceptées.
- Si un titre n'est pas trouvé : saisie manuelle du cours.
- Si Yahoo ne répond pas, le dernier cours enregistré reste affiché, et la saisie manuelle reste possible.

### 5.5 Dividendes

**Source** : historique des détachements fourni par Yahoo Finance (date de détachement, montant par action).

| Donnée | Calcul |
| :--- | :--- |
| Dividende annuel par action | Somme des dividendes détachés sur les 12 derniers mois |
| Fréquence | Nombre de détachements sur 12 mois (annuel, semestriel, trimestriel…) |
| Rendement | Dividende annuel / dernier cours |
| Croissance | Taux annuel moyen entre la première et la dernière année civile complète disponibles (6 ans au plus) |
| Prochain détachement | Chaque détachement des 12 derniers mois décalé d'un an ; le plus proche dans le futur. **Estimation** à confirmer auprès de la société |
| Dividende brut de la ligne | Dividende annuel par action × quantité |

### 5.6 Retenues à la source et dividende net

Le **pays de la société** est déduit, dans cet ordre : du choix de l'utilisateur (« Modifier »), des deux premières lettres de l'ISIN, de la place de cotation (approximation : une société néerlandaise peut être cotée à Paris).

Retenues proposées par défaut (taux appliqués à un résident français, au 1er janvier 2025), **modifiables dans Paramètres** :

| Pays | Retenue | Pays | Retenue |
| :--- | :--- | :--- | :--- |
| France | 0 % | Italie | 26 % |
| Allemagne | 26,375 % | Luxembourg | 15 % |
| Autriche | 27,5 % | Pays-Bas | 15 % |
| Belgique | 30 % | Norvège | 25 % |
| Danemark | 27 % | Pologne | 19 % |
| Espagne | 19 % | Portugal | 25 % |
| Finlande | 20 % | Tchéquie | 15 % |
| Irlande | 25 % | Suède | 30 % |
| Grèce | 5 % | | |

Pays inconnu : retenue comptée à 0 et signalée sur la fiche de la ligne.

**Dividende net** : net = brut − retenue. Aucun impôt français tant que l'argent reste dans le plan ; la retenue n'est pas restituée (BOFiP BOI-RPPM-RCM-40-50-30 : les crédits d'impôt conventionnels ne sont restitués que si les revenus sont imposables, ce qui n'est pas le cas dans un PEA). Vérifié par `tests/fiscalite.rs`.

| Société | Retenue | Net pour 100 € bruts |
| :--- | ---: | ---: |
| Française | 0 % | 100,00 € |
| Néerlandaise | 15 % | 85,00 € |
| Allemande | 26,375 % | 73,62 € |
| Belge | 30 % | 70,00 € |

**Fiche de la ligne** (en cliquant sur son nom) : dividende annuel par action, fréquence, rendement, croissance, prochain détachement estimé, décomposition brut → retenue → net, assemblées générales, puis la courbe.

### 5.7 Courbe et analyse technique

Ouverte en cliquant sur une ligne ou sur un résultat de recherche.

**Courbe** : sur 3 mois, 6 mois, 1 an, 2 ans ou 5 ans, en bougies **journalières ou hebdomadaires**. Quatre représentations : **ligne** (clôtures), **chandeliers japonais**, **barres OHLC**, **Heikin Ashi** (bougies lissées : clôture = moyenne de l'ouverture, du plus haut, du plus bas et de la clôture ; ouverture = milieu de la bougie précédente). Ligne du PRU de l'utilisateur ; marqueur « D » à chaque détachement de dividende ; au survol, date, cours, RSI et, en bougies, ouverture, plus haut et plus bas. Volumes en histogramme sous la courbe (vert en séance de hausse, rouge en séance de baisse), à l'échelle du 98e centile pour qu'une séance exceptionnelle n'écrase pas les autres (les barres plus hautes sont écrêtées). Les indicateurs sont calculés sur un historique plus long que la période affichée (2, 5 ou 10 ans), pour que la moyenne 200 soit disponible. En hebdomadaire, les moyennes s'entendent en semaines.

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
| Structure de tendance | Deux derniers sommets et deux derniers creux (± 5 séances) : tous deux plus hauts = hausse ; plus bas = baisse ; sinon marché sans direction (range) | Haussier / baissier / neutre |
| Confirmation hebdomadaire | La structure hebdomadaire (± 3 semaines), confirmée par la position du cours par rapport à la moyenne 40 semaines, va dans le même sens que la structure journalière ; signal « en désaccord » dans le cas contraire | Haussier / baissier / neutre |
| Double sommet / double fond | Sur 120 séances : deux sommets (creux) à moins de 3 % l'un de l'autre, séparés d'au moins 10 séances, avec un creux (sommet) intermédiaire d'au moins 5 % ; « confirmé » quand le cours franchit ce niveau intermédiaire, « possible » sinon | Baissier / haussier |
| Doji | Corps ≤ 10 % de l'amplitude de la séance | Neutre (hésitation) |
| Marteau | Après 5 séances de baisse : mèche basse ≥ 2 × corps, mèche haute ≤ corps | Haussier |
| Étoile filante | Après 5 séances de hausse : mèche haute ≥ 2 × corps, mèche basse ≤ corps | Baissier |
| Avalement haussier / baissier | Le corps du jour englobe le corps de la veille, de couleur opposée, après une baisse / une hausse | Haussier / baissier |

Ne sont pas détectés, car trop sujets aux fausses détections : tête-épaules, triangles, drapeaux et biseaux. Les graphiques Renko et Point et Figure ne sont pas proposés.

**Synthèse** : « plutôt à la hausse » si les signaux haussiers dépassent les baissiers d'au moins 2, « plutôt à la baisse » dans le cas inverse, sinon « pas de tendance technique nette ».

**Avertissement affiché sous chaque analyse** : l'analyse technique décrit le comportement passé du cours, ne prédit pas l'avenir de façon fiable, beaucoup de signaux se révèlent faux ; ce n'est pas un conseil d'achat ou de vente.

---

## 6. Alertes et nouvelles versions

### 6.1 Alertes

Affichées sur le tableau de bord, pour les 45 prochains jours, triées par date :

| Alerte | Origine |
| :--- | :--- |
| Détachement de dividende | Date estimée (§5.5), pour chaque ligne du PEA |
| Assemblée générale | Date saisie par l'utilisateur sur la fiche de la ligne (avec une note facultative) |

Source conseillée pour les dates d'AG des sociétés françaises : l'avis de réunion publié au **BALO** (Bulletin des annonces légales obligatoires, journal-officiel.gouv.fr) et l'espace « Actionnaires » du site de la société. La récupération automatique depuis les données ouvertes du BALO est prévue (§9).

### 6.2 Nouvelles versions

Si l'utilisateur l'active dans Paramètres (**désactivé par défaut**, pour qu'Escarcelle ne contacte aucun serveur sans son accord), Escarcelle demande au démarrage à GitHub la liste des versions publiées du dépôt (`repository` de `app/src-tauri/Cargo.toml`) et compare la plus récente, préversions comprises, à la version installée. Si elle est plus récente, un bandeau propose d'ouvrir la page de la version. Aucune donnée d'Escarcelle n'est envoyée (GitHub voit seulement l'adresse IP, comme pour toute page web) ; rien n'est téléchargé ni installé automatiquement. Bouton « Vérifier maintenant » dans Paramètres.

### 6.3 Connexions réseau

| Service | Déclencheur | Contenu de la requête |
| :--- | :--- | :--- |
| Yahoo Finance | Page PEA : recherche, cours, dividendes, courbes | Texte recherché, ISIN ou symbole du titre ; jamais de quantité, de PRU ni de montant |
| GitHub (api.github.com) | Vérification des versions, si activée, ou bouton « Vérifier maintenant » | Liste des versions publiées |

Aucune autre connexion. Escarcelle communique donc avec l'extérieur pour lire des données publiques, mais **ne transmet aucune donnée de l'utilisateur**. Les pages Livrets et Tableau de bord fonctionnent hors ligne (le tableau de bord utilise les derniers cours enregistrés).

---

## 7. Paramètres

| Action | Fonctionnement |
| :--- | :--- |
| Sauvegarde chiffrée (0.4) | Fichier `escarcelle-AAAA-MM-JJ-HHMM.escarcelle` chiffré par mot de passe (§7.2) ; mot de passe de 12 caractères minimum, saisi deux fois, avec indicateur de robustesse et case « j'ai compris qu'un oubli rend la sauvegarde illisible » |
| Sauvegarde non chiffrée | Copie cohérente de toute la base (`escarcelle-AAAA-MM-JJ.db`), toujours proposée en lien secondaire |
| Restaurer | Fichier `.escarcelle` (mot de passe demandé) ou `.db`. Affiche la date, l'appareil et la version de la sauvegarde, demande confirmation, fait une **copie de sécurité** de la base actuelle (`escarcelle-avant-restauration-AAAAMMJJ-HHMMSS.db` dans le dossier de données, 5 dernières conservées), remplace les données, met le schéma à jour si la sauvegarde est ancienne et **refuse** une sauvegarde venant d'une version plus récente |
| Écritures | Toute sauvegarde est écrite dans un fichier temporaire puis renommée : un échec ne détruit pas une ancienne sauvegarde du même nom |
| Tout effacer | Supprime toutes les données après saisie du mot EFFACER ; le schéma est conservé |
| Retenues à la source | Tableau des pays : taux en vigueur, taux par défaut, modification et retour à la valeur par défaut |
| Nouvelles versions | Vérification au démarrage (case à cocher) et « Vérifier maintenant » |

### 7.2 Format de la sauvegarde chiffrée (`.escarcelle`)

| Partie | Contenu | Chiffré |
| :--- | :--- | :--- |
| En-tête (57 octets) | « ESCA », version du format (1), paramètres Argon2id m (Kio), t, p en u32 petit-boutiste, sel (16 octets), nonce (24 octets) | Non, mais authentifié (données associées) |
| Métadonnées | `cree_le` (UTC), `version_escarcelle`, `version_schema`, `appareil`, une par ligne `clé=valeur` | Oui |
| Données | Copie cohérente de la base SQLite (`VACUUM INTO`) | Oui |

- Clé de 32 octets dérivée par **Argon2id** (64 Mio, 3 itérations, 1 fil pour les nouvelles sauvegardes ; paramètres lus dans l'en-tête, bornés à 1 Gio, 10 itérations, 8 fils pour qu'un en-tête modifié ne bloque pas la machine).
- Chiffrement authentifié **XChaCha20-Poly1305** : un octet modifié, n'importe où, est détecté.
- Bibliothèques RustCrypto (`argon2`, `chacha20poly1305`), aléa du système (`getrandom`), clé et données en clair effacées de la mémoire après usage (`zeroize`).
- Mot de passe faux et fichier altéré donnent le même message, sans dire lequel.
- Le format est vérifié par un **vecteur de référence produit par une implémentation indépendante** (Python, bibliothèque `cryptography`), en plus des tests d'aller-retour.
- Écart à la spécification 0.4 : les données ne sont pas compressées (une base Escarcelle pèse quelques centaines de Kio) ; la sauvegarde automatique n'est pas encore faite (question ouverte : mémoriser le mot de passe dans le coffre Windows).

### 7.1 Exports

| Export | Contenu |
| :--- | :--- |
| Tableau de bord, CSV | Comptes, type, valeur, intérêts estimés ou plus-value latente, total |
| Tableau de bord, image | Camembert de répartition, avec titre, date et montants |
| Livrets, CSV | Historique de tous les virements (livret, date de valeur, opération, montant signé) |
| PEA, CSV | Une ligne par titre : identifiants, pays, quantité, PRU, cours, valeur, investi, plus-value, variation sur 1 an, dividende par action, rendement, dividende brut, retenue, dividende net, prochain détachement ; ligne de total |
| Courbe d'un titre, image | Cours, volumes, RSI et MACD tels qu'affichés, avec titre et mention de la source |

Format CSV : séparateur « ; », virgule décimale, UTF-8 avec BOM, pour une ouverture directe dans Excel ou LibreOffice en français. Images PNG en double résolution, aux couleurs du thème. L'emplacement est choisi par l'utilisateur ; seules les extensions `.csv` et `.png` sont acceptées par la commande d'enregistrement.

L'emplacement du fichier de données est affiché (sous Windows : `%APPDATA%\fr.chatcarbo.escarcelle\escarcelle.db`).

---

## 8. Données et architecture

### 8.1 Unités

| Grandeur | Unité interne |
| :--- | :--- |
| Montants | centimes (`i64`) |
| Taux | millionièmes (1,70 % = 17 000) |
| Cours, PRU, quantités, dividendes par action | millionièmes |
| Indicateurs techniques | nombres à virgule (`f64`) : ce sont des statistiques, pas des montants |

Les conversions depuis la saisie (« 1 234,56 ») se font une seule fois, à l'entrée.

### 8.2 Paramètres réglementaires embarqués

| Paramètre | Valeurs (date d'effet) |
| :--- | :--- |
| Livret A et LDDS | 0,50 % (01/02/2020) · 1,00 % (01/02/2022) · 2,00 % (01/08/2022) · 3,00 % (01/02/2023) · 2,40 % (01/02/2025) · 1,70 % (01/08/2025) · 1,50 % (01/02/2026) · 1,70 % (01/08/2026) |
| LEP | 1,00 % (01/02/2020) · 2,20 % (01/02/2022) · 4,60 % (01/08/2022) · 6,10 % (01/02/2023) · 6,00 % (01/08/2023) · 5,00 % (01/02/2024) · 4,00 % (01/08/2024) · 3,50 % (01/02/2025) · 2,70 % (01/08/2025) · 2,50 % (01/02/2026) |
| Plafonds | 22 950 € · 12 000 € · 10 000 € |
| Retenues à la source | Tableau du §5.6 |

À mettre à jour à chaque révision des taux (1er février, 1er août) dans `crates/escarcelle-core/src/params.rs` ; les retenues à la source sont dans `dividendes.rs`. La mise à jour par fichier signé est prévue (§9).

### 8.3 Stockage

Un seul fichier SQLite, schéma versionné et migré automatiquement à l'ouverture (et après une restauration). La migration 7 (version 0.3) efface les cours d'il y a un an déjà enregistrés, pour qu'ils soient recalculés sur l'historique journalier. La migration 6 (version 0.3) supprime les lignes du compte-titres et du PEA-PME, la table `pea_pme`, les taux de change, les retenues des pays hors PEA, ainsi que les dividendes, informations et événements des titres qui ne sont plus détenus.

| Table | Contenu |
| :--- | :--- |
| `comptes` | Livrets détenus (type, date d'ouverture) |
| `operations` | Virements des livrets (date, sens, montant en centimes) |
| `positions` | Lignes du PEA (ISIN, symbole de cotation, nom, quantité, PRU, pays) ; les colonnes `enveloppe` (« PEA ») et `devise` (« EUR ») restent pour la compatibilité des sauvegardes |
| `cours` | Dernier cours par titre et par jour (cours, heure, source) |
| `dividendes` | Détachements par titre (date, montant par action) |
| `infos_titres` | Cours d'il y a un an et date de la dernière récupération des dividendes |
| `evenements` | Assemblées générales et autres événements saisis (titre, date, note) |
| `parametres` | Réglages : retenues par pays (`retenue.XX`), vérification des versions (`maj.auto`) |

### 8.4 Organisation du code

```
crates/escarcelle-core     Calculs, sans dépendance ni accès réseau :
                        date, money, price, rate, params, livret, conseil_livret,
                        analyse, chandeliers, dividendes, identifiants
app/src-tauri           Application Tauri v2 : stockage SQLite, cours et dividendes Yahoo,
                        PEA, alertes, vérification des versions
app/src                 Interface Svelte 5
docs/                   Spécification, guide d'utilisation, compilation
.github/                Intégration continue, modèle d'issue
```

Commandes de l'interface V1 : `tableau_de_bord`, `livrets`, `ouvrir_livret`, `conseil_livret`, `repartir_livrets`, `ajouter_operation`, `supprimer_operation`, `positions`, `chercher_titres`, `ajouter_position`, `modifier_position`, `supprimer_position`, `actualiser_cours`, `saisir_cours`, `analyse_titre`, `retenues`, `definir_retenue`, `evenements`, `ajouter_evenement`, `supprimer_evenement`, `verifier_mise_a_jour`, `verification_auto`, `emplacement_donnees`, `sauvegarder`, `restaurer`, `tout_effacer`, `enregistrer_export`.

### 8.5 Qualité et stabilité

- `cargo test` à la racine : 91 tests du cœur de calcul, sans réseau (livrets, quinzaines, conseils, analyse technique, chandeliers et figures, dividendes, ISIN). Deux fichiers reprennent des sources officielles citées en tête : `tests/reference.rs` (fiche F2365 de service-public.fr) et `tests/fiscalite.rs` (BOFiP sur le PEA).
- `tests/proprietes.rs` : **600 scénarios de livret générés automatiquement** (graine fixe, donc reproductibles : dates sur les jours charnières 1, 2, 14, 15, 16, 17, fin de mois, 31 décembre ; montants de 1 centime à 30 000 € ; versements, retraits, soldes de départ multiples ; taux officiels ou inventés, changeant même en milieu de quinzaine). Pour chacun :
  - le moteur doit donner exactement le même résultat qu'un **oracle indépendant** (soldes recalculés de zéro à chaque quinzaine au lieu d'être cumulés) ;
  - propriétés : ordre de saisie sans effet, déplacement dans la même quinzaine sans effet, découpage d'un versement sans effet, intérêts jamais négatifs, verser plus ou à un meilleur taux ne rapporte jamais moins, passage d'année (solde au 31/12 = solde précédent + intérêts + mouvements de l'année), contrôle du plafond stable ;
  - chaque **changement officiel de taux** depuis 2020 (Livret A, LDDS, LEP) : la quinzaine qui commence le jour du changement est payée au nouveau taux, la précédente à l'ancien ; capitalisation sur trois ans.
  - La suite a été vérifiée par **mutation** : six erreurs volontaires dans le moteur (versement compté dès sa quinzaine, taux pris en fin de quinzaine, solde de départ décalé, mauvais solde de départ retenu, arrondi par troncature, solde négatif rémunéré) sont toutes détectées.
- `tests/robustesse.rs` : entrées vides, courtes, constantes ou mal formées (analyse technique, chandeliers, dividendes, ISIN, dates) et simulation des livrets pour toute date depuis le début de l'historique des taux ; aucune fonction ne doit planter, et les indicateurs gardent la longueur de l'historique.
- `cargo test` dans `app/src-tauri` : 23 tests — historique normalisé des livrets, sauvegarde chiffrée (aller-retour, altération d'un octet, mauvais mot de passe, en-tête démesuré, vecteur de référence indépendant), intérêts année par année, solde de départ, contrôle du plafond et du solde à la date de valeur, base SQLite (lignes, dividendes, événements), migration d'une base 0.2 avec CTO et PEA-PME, sauvegarde et restauration, ISIN, symboles, places et pays, dates, numéros de version.
- Intégration continue (GitHub Actions) à chaque envoi : `cargo fmt --check`, `cargo clippy -D warnings`, tests du cœur sous Linux et avec Rust 1.75 (version minimale), build de l'interface et tests de l'application sous Windows.
- Règles de robustesse de l'application :
  - aucune commande ne garde le verrou de la base pendant une requête réseau (vérifié par le compilateur : les commandes asynchrones doivent être `Send`) ;
  - si une commande plante en tenant le verrou, il est récupéré au lieu de bloquer toutes les commandes suivantes ;
  - toute saisie est revérifiée côté Rust (montants, quantités, dates, pays, symboles), même quand l'interface l'a déjà contrôlée ;
  - limites métier contre les débordements d'entiers : 10 000 000 € par opération de livret, 10 millions de titres et 1 000 000 € par titre dans le PEA ; l'arrondi est borné au lieu de déborder ;
  - arrondi des intérêts au centime le plus proche, une fois par an (testé : demi-centime, arrondi annuel contre arrondi par quinzaine, changement de taux au début d'une quinzaine) ;
  - chaque appel de l'interface affiche son erreur au lieu de l'ignorer ; les réponses d'une recherche, d'un conseil ou d'une courbe dépassées par une demande plus récente sont ignorées ;
  - les listes affichées ont des clés uniques (une clé en double fait planter un composant Svelte).
- Règle : pas de code mort. Une fonction qui n'est pas dans le menu n'a pas de code dans le dépôt.
- Vérifications à faire sur de vrais relevés : `docs/VERIFICATION.md`.

---

## 9. Feuille de route

| Version | Contenu | État du code |
| :--- | :--- | :--- |
| **V1** (0.2.0) | Tableau de bord, Livrets, PEA et CTO (lignes, cours y compris en devises, dividendes, AG, analyse technique), exports CSV et PNG, Paramètres | Publiée |
| **0.3 — stabilisation** | CTO et PEA-PME supprimés ; historique des taux des livrets depuis 2020 ; contrôles de saisie côté Rust ; **aucune nouvelle fonction** : code mort supprimé ; vérification des versions désactivée par défaut ; intégration continue ; fiscalité (flat tax, crédit d'impôt, retenues) testée contre des sources officielles (BOFiP, impots.gouv.fr, conventions fiscales) ; intérêts des livrets comparés à de vrais relevés ; deux à trois semaines d'utilisation réelle, une issue GitHub par problème, chaque issue corrigée avec un test | En cours |
| 0.4 | Livrets : **intérêt maximal de l'année en cours** sur chaque carte = maximum théorique d'un livret resté au plafond du 1er janvier au 31 décembre, soit la somme, sur les 24 quinzaines, de plafond × taux de la quinzaine / 24, arrondie au centime une fois. En 2026 : Livret A 367,20 € (22 950 € à 1,60 % en moyenne), LDDS 192,00 €, LEP 251,67 € | Demandé, reporté après la stabilisation |
| 0.4 | PEA : **simulation d'un retrait et de son impôt**. À saisir : date d'ouverture du plan et total des versements (aujourd'hui, Escarcelle suit des lignes, pas les versements). Calcul : gain contenu dans le retrait au prorata (gain total × retrait / valeur du plan) ; avant 5 ans, retrait = clôture (sauf exceptions) et gain soumis à l'impôt sur le revenu et aux prélèvements sociaux ; après 5 ans, prélèvements sociaux seuls et retraits partiels possibles sans clôture. Taux et règles à vérifier sur les sources officielles (BOFiP, service-public.fr) au moment du développement, chacun couvert par un test | Demandé |
| V1.1 | Dates d'AG récupérées depuis les données ouvertes du BALO ; mise à jour des taux par fichier signé ; écran Aide intégré | — |
| V1.2 | Alertes de cours (seuils) ; notifications système ; éventuellement PEA-PME et compte-titres, réécrits (ancien code dans l'historique, étiquette `v0.2.0`) | — |
| V2 | **Impôt et dons** : TMI, décote, plafonnement, réduction pour dons ; **PER** : intérêt d'un versement selon la TMI | À réécrire (ancien code dans l'historique, étiquette `v0.2.0`) |
| V3 | Assurance-vie, import CSV des relevés | — |

---

## 10. Sources

- Taux de l'épargne réglementée au 01/08/2026 : https://www.service-public.gouv.fr/particuliers/actualites/A15818
- Plafonds de versement, règles des quinzaines : https://www.service-public.gouv.fr/particuliers/vosdroits/F2365
- Éligibilité au PEA (titres de sociétés de l'UE et de l'EEE) : https://www.service-public.gouv.fr/particuliers/vosdroits/F2385
- Retenues à la source des pays éligibles au PEA au 01/01/2025 (valeurs par défaut, indicatives) : https://www.professioncgp.com/article/juridique-et-fiscal/impot-sur-le-revenu/declaration-de-revenus-quelle-est-la-fiscalite-du-pea.html
- PEA et crédits d'impôt conventionnels : BOI-RPPM-RCM-40-50-30, https://bofip.impots.gouv.fr/bofip/1560-PGP.html
- Historique des taux du Livret A : barèmes IPP, https://www.ipp.eu/baremes-ipp/taxation-du-capital/taxation_capital.epargne.livret_a/table
- Historique des taux du LEP : Banque de France, rapports annuels sur l'épargne réglementée (https://www.banque-france.fr) ; vie-publique.fr, rapport 2022 sur l'épargne réglementée
- Retenue à la source en Grèce (5 % depuis le 01/01/2020) : Clearstream, fiche marché Grèce
- Avis de réunion des assemblées générales (BALO) : https://www.journal-officiel.gouv.fr
