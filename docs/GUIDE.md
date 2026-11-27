# Guide d'utilisation d'€scarcell

€scarcell vous aide à suivre vos **livrets réglementés** (Livret A, LDDS, LEP), et votre **PEA**, directement sur votre ordinateur. Vos données ne quittent jamais votre machine : pas de compte à créer, pas de banque à connecter. €scarcell lit seulement des cours publics sur Internet, sans rien transmettre de vos données.

---

## Sommaire

1. [Installer et lancer €scarcell](#1-installer-et-lancer-escarcell)
2. [Découvrir l'écran](#2-découvrir-lécran)
3. [Suivre ses livrets](#3-suivre-ses-livrets)
4. [Choisir le bon jour pour verser ou retirer](#4-choisir-le-bon-jour-pour-verser-ou-retirer)
5. [Suivre son PEA](#5-suivre-son-pea)
6. [Dividendes, impôts et assemblées générales](#6-dividendes-impôts-et-assemblées-générales)
7. [Lire la courbe et l'analyse technique](#7-lire-la-courbe-et-lanalyse-technique)
8. [Paramètres : sauvegarde, retenues, nouvelles versions](#8-paramètres--sauvegarde-retenues-nouvelles-versions)
9. [Questions fréquentes](#9-questions-fréquentes)

---

## 1. Installer et lancer €scarcell

### Avec l'installateur

Téléchargez `€scarcell_x.y.z_x64-setup.exe` depuis la page **Releases** du dépôt GitHub, lancez-le et suivez les étapes. €scarcell apparaît ensuite dans le menu Démarrer.

### Depuis le code source (Windows)

Installez une seule fois :
- **Rust** : https://rustup.rs
- **Node.js** (version LTS) : `winget install OpenJS.NodeJS.LTS`
- **Visual Studio Build Tools**, avec l'option « Développement Desktop en C++ »

Fermez et rouvrez PowerShell, puis dans le dossier `escarcell\app` :

```powershell
npm install
npm run tauri icon src-tauri/icons/logo.svg   # une seule fois
npm run tauri dev                             # lance €scarcell
npm run tauri build                           # crée l'installateur
```

La première compilation prend plusieurs minutes. L'installateur est créé dans `app\target\release\bundle\nsis\`.

---

## 2. Découvrir l'écran

À gauche, le menu :

| Menu | À quoi ça sert |
| :--- | :--- |
| **Tableau de bord** | Vue d'ensemble : patrimoine suivi, intérêts de l'année, plus-values, dividendes nets, répartition, événements à venir |
| **Livrets** | Vos virements sur Livret A, LDDS et LEP, et les conseils de dates |
| **PEA** | Vos lignes d'actions et d'ETF, leurs cours, leurs dividendes et leurs courbes |
| **Paramètres** (bouton ⚙ en haut à droite) | Sauvegarde, restauration, effacement, retenues à la source, nouvelles versions |

Le numéro de version d'€scarcell est affiché en bas du menu.

Si vous activez la vérification des nouvelles versions dans **Paramètres** (désactivée par défaut), un bandeau vert vous signale au démarrage qu'une nouvelle version est publiée, avec un lien vers la page de téléchargement.

Tous les montants sont en euros, au format français (1 234,56 €). Les gains apparaissent en vert, les pertes en rouge.

---

## 3. Suivre ses livrets

### Déclarer ses livrets

Dans **Livrets**, trois cartes s'affichent : LEP, Livret A, LDDS. Pour chaque livret que vous possédez, cliquez sur **« J'ai un … »**.

### Saisir un virement

1. Choisissez **Versement** ou **Retrait**.
2. Choisissez le livret.
3. Indiquez la **date de valeur** (celle qui figure sur votre relevé, pas forcément le jour où vous avez fait le virement). €scarcell connaît les taux depuis le **1er février 2020** : pour un livret plus ancien, commencez par un **solde de départ** (voir ci-dessous).
4. Indiquez le montant, puis **Enregistrer**.

€scarcell refuse un versement qui dépasserait le plafond (22 950 € pour le Livret A, 12 000 € pour le LDDS, 10 000 € pour le LEP) et un retrait supérieur au solde. Le contrôle se fait **à la date de valeur** : un versement daté de 2021 est comparé au solde de 2021, sans les intérêts gagnés depuis.

> **Pour démarrer : le solde de départ.** Inutile de ressaisir des années de virements. Choisissez **Solde de départ**, puis indiquez le solde de votre dernier relevé annuel, **intérêts compris** (par exemple le solde au 31 décembre 2025), daté du **1er janvier** suivant (daté du 31 décembre, c'est pareil). Ce solde rapporte dès le 1er janvier, et il peut dépasser le plafond si les intérêts l'ont fait grossir. Saisissez ensuite les virements faits depuis. Un seul solde de départ par livret ; pour le changer, supprimez-le dans l'historique.

### Ce que vous voyez

Chaque carte affiche le **solde**, une **jauge** de remplissage du plafond, les **intérêts estimés** de l'année et les **intérêts cumulés** depuis votre première opération. **Détail par année** déplie, pour chaque année, le taux moyen appliqué, les intérêts et le solde au 31 décembre. Les intérêts sont ajoutés au solde chaque 31 décembre, comme le fait votre banque.

### Où verser, où retirer ?

Quand vous tapez un montant, €scarcell propose une répartition :
- pour un **versement** : d'abord le LEP (le mieux rémunéré), puis le Livret A, puis le LDDS, sans dépasser les plafonds ;
- pour un **retrait** : d'abord le livret le moins rémunéré ; à taux égal, le LDDS avant le Livret A (on garde plein le livret qui a le plus grand plafond).

Cette suggestion est calculée sur les soldes d'aujourd'hui, tous livrets confondus : elle ne dépend pas du livret choisi dans le formulaire.

### Corriger une erreur

Dans **Historique**, cliquez sur ✕ à côté du virement, puis sur **Confirmer ?**.

---

## 4. Choisir le bon jour pour verser ou retirer

Les livrets ne comptent pas les intérêts au jour près mais par **quinzaine** : du 1er au 15, et du 16 à la fin du mois.

### Les deux règles à retenir

🟢 **Versez avant le 15 ou avant la fin du mois.**
L'argent rapporte à partir du 16 ou du 1er suivant. Verser le 2 ou le 14 donne exactement le même résultat : inutile de se précipiter, mais ne laissez pas passer la fin de la quinzaine.

🔴 **Retirez le 1er ou le 16.**
Un retrait fait perdre toute la quinzaine en cours. Retirer le 14 coûte autant que retirer le 1er. En revanche, attendre le 16 fait gagner la quinzaine du 1er au 15.

### La frise colorée

Pendant la saisie, €scarcell dessine les jours autour de la date choisie :

| Couleur | Signification |
| :--- | :--- |
| 🟩 Vert | L'argent rapporte |
| 🟧 Orange | Versement en attente : il ne rapporte pas encore |
| 🟥 Rouge | Quinzaine perdue pour le montant retiré |
| ⬜ Gris | L'argent n'est pas sur le livret |
| ▼ | La date que vous avez choisie |
| ★ | La date limite (versement) ou la date conseillée (retrait) |

**Exemple : retrait de 2 400 € le 14 mars sur un Livret A à 1,70 %.**
Le message indique que ce montant ne rapporte plus depuis le 1er mars, et que si vous pouvez attendre le 16 mars, vous gagnez 1,70 €. Si vous avez besoin de l'argent tout de suite, retirer le 14 ou le 15 revient au même.

> **Attention aux dates de valeur** : un virement fait un vendredi 15 peut être daté du lundi 17 par votre banque. Prévoyez un ou deux jours ouvrés de marge.

---

## 5. Suivre son PEA

> €scarcell suit le **PEA**. Le compte-titres et le PEA-PME ne sont plus proposés depuis la version 0.3 ; si vous aviez saisi des lignes dans ces enveloppes avec la 0.2, elles sont supprimées à la première ouverture (faites une sauvegarde avec la 0.2 avant de mettre à jour si vous voulez les garder).

€scarcell ne vous demande pas l'historique de vos achats : vous saisissez simplement ce que vous détenez, comme sur l'écran de votre banque.

### Ajouter une ligne au PEA

1. Dans **Ajouter une ligne**, tapez le nom, le mnémonique ou le code ISIN du titre : « LVMH », « MC », « FR0000121014 », « MSCI World »…
2. La liste se remplit pendant que vous tapez. Chaque résultat indique l'**éligibilité au PEA** :
   - **probable** : société européenne ;
   - **à vérifier** : ETF ou titre coté en Allemagne ;
   - **non** : société hors d'Europe.
3. Cliquez sur le bon titre (de préférence celui coté à **Paris**, en tête de liste). Sa courbe s'affiche pour vous aider à le reconnaître.
4. Saisissez la **quantité** et le **PRU** (prix de revient unitaire) tels qu'affichés par votre banque, puis **Ajouter**. Le PRU de la banque inclut déjà les frais de courtage : rien d'autre à saisir.

> L'éligibilité affichée est indicative. En cas de doute, vérifiez auprès de votre banque avant d'acheter.

### Le tableau des lignes

| Colonne | Signification |
| :--- | :--- |
| Quantité, PRU | Ce que vous avez saisi |
| Cours | Dernier cours connu, avec son heure |
| Valeur | Quantité × cours |
| +/− value | Valeur − (quantité × PRU), en euros et en % |
| 1 an | Évolution du cours sur les 12 derniers mois |
| Rend. | Rendement du dividende : dividende annuel ÷ cours |

En haut, deux rangées de chiffres :
- valeur, montant investi, plus ou moins-value, impôt (aucun tant que l'argent reste dans le plan) ;
- dividendes annuels bruts et nets, rendement et croissance moyenne du dividende.

En bas : un camembert de répartition de vos lignes.

### Les cours

- Ils se mettent à jour à l'ouverture de la page, puis **toutes les 5 minutes** tant que la page reste ouverte. Le bouton **Actualiser les cours** force une mise à jour. Les dividendes sont récupérés en même temps, une fois par jour.
- Ils viennent de Yahoo Finance et ont en général **environ 15 minutes de retard**.
- Si un titre n'a pas de cours, cliquez sur **Cours** (ou **saisir**) pour l'entrer vous-même.

### Modifier après un achat ou une vente

Cliquez sur **Modifier**, mettez à jour la quantité et le PRU affichés par votre banque, puis **Enregistrer**. Le menu **Pays** permet de corriger le pays de la société s'il est mal deviné (voir ci-dessous). Pour une ligne vendue entièrement, cliquez sur ✕ puis **Confirmer ?**.

Une ligne marquée **importée** est calculée à partir des achats et ventes d'un import CSV (voir ci-dessous) : sa quantité et son PRU ne se modifient pas à la main, seul son pays se corrige. La supprimer supprime aussi ses achats et ventes importés.

### Importer son portefeuille

Sur la page **PEA**, cliquez sur **Importer…** et choisissez un fichier exporté de votre banque ou de votre application de suivi : export CSV de **Moning**, export des positions de **BoursoBank**, export Excel « portefeuille simple » de **Fortuneo**, classeur **Excel** (.xlsx, .xls) ou OpenDocument (.ods), ou tout tableau qui contient, pour chaque titre, sa **quantité**, son **prix de revient (PRU)** et son **ISIN**, son **symbole** ou son **nom**. Rien à régler : €scarcell reconnaît seul le format, la ligne d'en-tête (même après des lignes de titre), les colonnes en français ou en anglais, la virgule ou le point décimal.

Un aperçu indique, titre par titre, ce qui va se passer : **Nouvelle** ligne, **Mise à jour** (ancienne et nouvelle quantité, ancien et nouveau PRU), **Inchangée**, ou **Erreur** (ISIN dont la clé est fausse, titre coté dans une autre devise que l'euro…). Les lignes de total et de liquidités sont ignorées. Cliquez sur **Importer** : tout est enregistré en une fois, après une copie de sécurité de vos données.

- Un titre déjà présent est retrouvé par son ISIN ou son symbole : sa quantité et son PRU sont remplacés, son pays est gardé.
- Les lignes de votre PEA absentes du fichier ne sont pas touchées, sauf si vous cochez « Supprimer aussi… ».
- Fortuneo écrit le mnémonique entre parenthèses (« TOTALENERGIES (TTE) ») : il sert à trouver le cours. Une ligne d'actions à prime de fidélité (« AIR LIQUIDE PF28 », ISIN provisoire) est retrouvée d'après son nom ; l'aperçu vous demande de vérifier.
- Un export de **PEA-PME** est refusé quand son titre l'indique (€scarcell ne suit que le PEA). Ce n'est pas toujours le cas : n'importez que l'export de votre PEA.
- Un titre présent deux fois dans le fichier est regroupé (quantités additionnées, PRU moyen pondéré).
- Importer deux fois le même fichier ne change rien.
- Une **copie de sécurité** de vos données est faite avant chaque import, dans le dossier de données : en cas d'erreur, restaurez-la dans **Paramètres → Restaurer une sauvegarde** (fichier `escarcell-avant-import-…db`).

### Importer un historique d'opérations

Si le fichier choisi avec **Importer…** contient des **opérations datées** (colonnes date, compte, type : achats, ventes, dividendes, versements, retraits, et aussi les virements des livrets), €scarcell ouvre l'import détaillé. Le modèle de fichier se télécharge depuis cet écran.

1. **Colonnes** : reconnues seules si elles suivent le modèle ; sinon, associez chaque colonne à un champ et enregistrez cette correspondance sous un nom pour la prochaine fois.
2. **Aperçu** : *Valide* ; *Avertissement* (importée, à vérifier) ; *Erreur* (non importée : vente de plus de titres que vous n'en déteniez à cette date, retrait supérieur au solde…) ; *Doublon* (déjà enregistrée) ; *Non importée* (intérêts d'un livret).
3. **Importer** : les lignes retenues sont enregistrées toutes ensemble ou pas du tout.

Bon à savoir :

- Pour un achat ou une vente, le **montant est calculé** (quantité × prix) ; les **frais** s'ajoutent au PRU. Les lignes du PEA calculées ainsi sont marquées **importée** : leur quantité et leur PRU ne se modifient plus à la main.
- Les **intérêts des livrets ne sont pas importés** : €scarcell les recalcule et indique, année par année, s'ils sont conformes à ceux du fichier.
- Pour un livret, €scarcell connaît les taux depuis le 1er février 2020 : remplacez les opérations plus anciennes par une ligne `solde`.

---

## 6. Dividendes, impôts et assemblées générales

Cliquez sur le **nom d'une ligne** : sa fiche s'ouvre au-dessus de la courbe.

### Les dividendes

| Donnée | Signification |
| :--- | :--- |
| Dividende / action, 12 derniers mois | Somme des dividendes **détachés** ces 12 derniers mois (période affichée), et rythme (annuel, semestriel, trimestriel) |
| Rendement | Dividende des 12 derniers mois ÷ cours actuel |
| Croissance | Hausse moyenne par an du total des dividendes d'une année civile complète, entre les années affichées (6 ans au plus). Pour le PEA : moyenne des lignes, pondérée par leur dividende |
| Prochain détachement | Date estimée d'après celle de l'an dernier : à confirmer sur le site de la société |

€scarcell compte ce qui a **déjà été versé**. Une hausse annoncée n'apparaît qu'une fois le dividende détaché : une application qui projette les prochains versements (Moning, par exemple) peut afficher un montant plus élevé. Exemple fin novembre 2026 : TotalEnergies a détaché 0,85 € trois fois puis 0,90 € (3,45 € sur 12 mois), alors que les acomptes annoncés sont de 0,90 € (3,60 € par an).

### Du brut au net

La fiche détaille ce que vous touchez vraiment sur un an :

1. **Dividende brut** : dividende par action × quantité.
2. **Retenue à la source** : prélevée par le pays de la société (0 % en France, 26,375 % en Allemagne, 15 % aux Pays-Bas…). Le pays est deviné à partir de l'ISIN ou de la place de cotation ; corrigez-le avec **Modifier** si besoin (par exemple Stellantis ou Airbus, cotées à Paris mais néerlandaises).
3. **Dividende net encaissé** : brut − retenue. Aucun impôt français tant que l'argent reste dans le PEA.

| 100 € de dividende brut | PEA |
| :--- | ---: |
| Société française | 100,00 € |
| Société néerlandaise (15 %) | 85,00 € |
| Société allemande (26,375 %) | 73,62 € |

> Dans un PEA, la retenue étrangère est **perdue** : il n'y a pas d'impôt français sur lequel la déduire. C'est pourquoi les actions françaises sont souvent privilégiées dans ces plans pour toucher des dividendes.

### Le calendrier des dividendes

En bas de la page **PEA**, le calendrier rassemble ce que vos lignes vous ont rapporté et ce qu'elles devraient rapporter.

- **À venir** : les détachements des 12 prochains mois, mois par mois, avec le montant net attendu. *Annoncé* : date et montant publiés par la source. *Estimé* : même date que l'an dernier et montant du même coupon l'an dernier ; si le dernier dividende a changé, le même changement est appliqué (TotalEnergies : acomptes passés de 0,85 € à 0,90 € → les suivants sont estimés à 0,90 €). Un changement de plus de 50 % (dividende exceptionnel) n'est pas appliqué. Les ETF capitalisants n'ont pas de dividende et n'apparaissent pas.
- **Dividendes détachés à enregistrer** : de 15 à 90 jours après un détachement, s'il n'y a pas d'opération correspondante, €scarcell propose de l'enregistrer, avec le montant net prérempli (quantité détenue aujourd'hui × dividende, moins la retenue à la source). Corrigez-le d'après votre relevé, puis **Enregistrer** ; **Ignorer** ne le propose plus.
- **Perçus** : les dividendes enregistrés (ou importés avec un historique d'opérations) ; ✕ en supprime un.
- **Année** : histogramme des dividendes nets par mois. En vert, les **perçus** : dividendes enregistrés, et dividendes détachés ces 12 derniers mois pas encore enregistrés (calculés avec la quantité détenue aujourd'hui) ; en bleu, les annoncés ; en bleu clair, les estimés.
- **Synthèse** : dividendes nets des 12 derniers mois et des 12 prochains, rendement sur la valeur actuelle et sur le prix de revient (PRU), et les lignes qui rapportent le plus.

### Les assemblées générales

Sur la fiche d'une ligne, saisissez la **date de l'AG** (et une note si vous voulez : lieu, vote par correspondance…), puis **Ajouter l'AG**. Les sociétés françaises publient cette date au **BALO** (sur journal-officiel.gouv.fr) et dans l'espace « Actionnaires » de leur site, en général au printemps.

### Le bloc « À venir » du tableau de bord

Il liste, pour les 45 prochains jours, les **détachements de dividendes estimés** et les **assemblées générales** que vous avez saisies.

---

## 7. Lire la courbe et l'analyse technique

Cliquez sur le **nom d'une ligne** (ou sur un résultat de recherche) pour ouvrir sa courbe.

### La courbe

- Choisissez la période : 3 mois, 6 mois, 1 an, 2 ans ou 5 ans.
- Choisissez **Jour** ou **Semaine** : en hebdomadaire, chaque bougie résume une semaine ; c'est la vue des investisseurs à long terme, avec moins de « bruit ».
- Choisissez la représentation :
  - **Ligne** : les cours de clôture, le plus simple ;
  - **Chandeliers** : pour chaque séance, un corps entre l'ouverture et la clôture (vert si hausse, rouge si baisse) et des mèches jusqu'au plus haut et au plus bas ;
  - **Barres** : les mêmes informations, en traits (ouverture à gauche, clôture à droite) ;
  - **Heikin Ashi** : des chandeliers lissés, où les tendances apparaissent plus nettement (une suite de bougies vertes = hausse).
- La ligne **orange pointillée** est votre PRU : au-dessus, vous êtes gagnant.
- Les pastilles violettes **D** marquent les détachements de dividendes (le cours baisse mécaniquement ce jour-là du montant du dividende).
- Sous la courbe, les **volumes** : le nombre de titres échangés. Une hausse accompagnée de volumes en hausse est plus solide. Une séance exceptionnelle dépasse le haut de la zone : sa barre est coupée pour garder les autres lisibles.
- **Exporter en image** enregistre la courbe, les volumes, le RSI et le MACD dans un fichier PNG.
- Passez la souris sur la courbe pour voir la date, le cours, le RSI et, en bougies, l'ouverture, le plus haut et le plus bas.

### Les indicateurs, en clair

| Indicateur | Ce qu'il montre |
| :--- | :--- |
| **Moyennes mobiles** 20, 50, 200 jours | La tendance à court, moyen et long terme. Un cours au-dessus de sa moyenne 200 jours est en tendance haussière de fond. |
| **Bandes de Bollinger** | Une zone « normale » autour du cours. En sortir signale un excès ; des bandes très resserrées annoncent souvent un mouvement fort. |
| **Supports** (vert) et **résistances** (rouge) | Des prix où le cours a déjà rebondi ou buté plusieurs fois. Le nombre entre parenthèses indique combien de fois. |
| **RSI** | La vitesse des variations récentes, de 0 à 100. Au-dessus de 70, le titre a beaucoup monté (surachat) ; en dessous de 30, il a beaucoup baissé (survente). |
| **MACD** | L'élan du cours. Quand la ligne bleue passe au-dessus de l'orange, l'élan devient positif. |

Cochez ou décochez les cases au-dessus de la courbe pour afficher ce qui vous intéresse.

### Les signaux

Sous les graphiques, €scarcell liste les signaux détectés, chacun avec son explication :

| Signal | Ce qu'il veut dire |
| :--- | :--- |
| **Cassure de résistance** | Le cours a franchi un plafond qui le bloquait. Plus solide si les volumes augmentent. |
| **Cassure de support** | Le cours est passé sous un plancher qui le soutenait. |
| **Rebond possible sur support** | Le cours repart à la hausse juste au-dessus d'un plancher. |
| **Croisement doré / de la mort** | La moyenne 50 jours passe au-dessus / en dessous de la 200 jours. |
| **Surachat / survente** | Le titre a beaucoup monté / baissé récemment. |
| **Compression des bandes** | Un mouvement fort se prépare, sans indication de sens. |
| **Plus hauts et plus bas croissants / décroissants** | La définition même d'une tendance haussière / baissière. Sinon : marché sans direction (range), le cours oscille entre support et résistance. |
| **Tendance confirmée en hebdomadaire** | La tendance des derniers jours se voit aussi en semaines : signal plus fiable. « En désaccord » appelle à la prudence. |
| **Double sommet / double fond** | Le cours bute deux fois au même niveau : possible retournement, confirmé quand il franchit le creux (ou le sommet) entre les deux. |
| **Doji** | Ouverture et clôture presque égales : le marché hésite. |
| **Marteau / étoile filante** | Longue mèche après une baisse (une hausse) : possible retournement. |
| **Avalement haussier / baissier** | La bougie du jour « avale » celle de la veille : les acheteurs (vendeurs) reprennent la main. |

Une **synthèse** résume l'ensemble : plutôt à la hausse, plutôt à la baisse, ou pas de tendance nette.

> ⚠️ **À lire avant de s'en servir.** L'analyse technique décrit le passé du cours. Elle ne prédit pas l'avenir de façon fiable, et beaucoup de signaux se révèlent faux, surtout sur quelques jours. Ce n'est pas un conseil d'achat ou de vente. Pour un investissement à long terme dans un PEA, la diversification et la durée comptent bien plus que le moment d'achat.

---

## 8. Paramètres : sauvegarde, retenues, nouvelles versions

Tout se passe dans **Paramètres**, en haut à droite de la fenêtre.

### Sauvegarder (chiffré)

1. Dans **Sauvegarde chiffrée**, choisissez un mot de passe d'au moins 12 caractères (une phrase de quatre ou cinq mots est facile à retenir et solide), tapez-le deux fois.
2. Cochez **J'ai compris qu'un mot de passe oublié rend la sauvegarde illisible**.
3. Cliquez sur **Sauvegarder (chiffré)…** et choisissez l'emplacement : le fichier `escarcell-2026-11-25-2130.escarcell` peut aller sur une clé USB ou dans un dossier synchronisé (OneDrive, Google Drive…), son contenu est illisible sans le mot de passe.

> **Le mot de passe n'est conservé nulle part.** Si vous l'oubliez, personne ne peut ouvrir la sauvegarde. Notez-le dans un gestionnaire de mots de passe.

Une sauvegarde par mois est un bon rythme, et une avant chaque mise à jour d'€scarcell. Une copie **non chiffrée** (`.db`) reste possible avec le lien sous le bouton.

### Restaurer

Cliquez sur **Choisir une sauvegarde…** et sélectionnez un fichier `.escarcell` (ou `.escarcelle`, nom d'avant 0.4) (tapez son mot de passe, puis **Ouvrir**) ou `.db`. €scarcell affiche la date, l'ordinateur et la version de la sauvegarde ; confirmez avec **Oui, restaurer**. **Toutes les données actuelles sont remplacées**, mais €scarcell en fait d'abord une copie de sécurité dans son dossier de données (`escarcell-avant-restauration-….db`, les cinq dernières sont gardées). Une sauvegarde faite avec une version plus récente d'€scarcell est refusée : mettez d'abord €scarcell à jour.

### Tout effacer

Tapez **EFFACER** en majuscules, puis cliquez sur **Tout effacer**. Livrets, virements, lignes du PEA, cours, dividendes, AG et réglages sont supprimés définitivement. Faites une sauvegarde avant.

### Retenues à la source

Le tableau liste le taux retenu par chaque pays sur les dividendes. Les valeurs proposées sont indicatives : si votre relevé montre un autre taux (par exemple parce que votre banque applique le taux réduit d'une convention fiscale), cliquez sur **Modifier**. **Rétablir** revient à la valeur proposée.

### Nouvelles versions

La case **Vérifier au démarrage** est **décochée par défaut** : €scarcell ne contacte pas GitHub sans votre accord. Cochez-la pour être prévenu des nouvelles versions, ou cliquez de temps en temps sur **Vérifier maintenant**. €scarcell ne fait que lire la liste des versions publiées : aucune de vos données n'est envoyée, et rien n'est téléchargé ni installé sans vous.

> **Connexions à Internet.** €scarcell communique avec l'extérieur pour lire des données publiques, sans jamais transmettre vos données : Yahoo Finance pour les cours et les dividendes (seul le nom, l'ISIN ou le symbole du titre est envoyé, jamais vos quantités ou montants), et GitHub uniquement pour la vérification des versions. Les livrets et le tableau de bord fonctionnent hors ligne.

### Où sont mes données ?

L'emplacement exact est affiché dans **Paramètres**. Sous Windows : `%APPDATA%\fr.chatcarbo.escarcell\escarcell.db`.

Au premier lancement après le changement de nom (Escarcelle → €scarcell), la base d'Escarcelle (`%APPDATA%\fr.chatcarbo.escarcelle\escarcelle.db`) est **copiée** automatiquement : vous retrouvez vos données, et l'ancienne base reste en place. Les sauvegardes chiffrées `.escarcelle` faites avant se restaurent normalement.

---

### Exporter

- **Exporter en CSV** (tableau de bord, livrets, PEA) : un fichier qui s'ouvre directement dans Excel ou LibreOffice, avec toutes les colonnes et un total.
- **Exporter en image** (courbe d'un titre, répartition du tableau de bord) : une image PNG nette, prête à coller dans un document ou un message.

Une fenêtre vous demande où enregistrer le fichier.

---

## 9. Questions fréquentes

**Les intérêts calculés ne correspondent pas exactement à mon relevé.**
Vérifiez que les dates saisies sont bien les **dates de valeur** du relevé. Si vous avez démarré avec un versement unique égal à votre solde, les intérêts de la première année sont sous-estimés (les quinzaines antérieures ne sont pas connues d'€scarcell).

**€scarcell refuse un virement daté d'avant 2020.**
Les taux des livrets sont connus depuis le 1er février 2020. Saisissez un **solde de départ** (le solde d'un relevé, intérêts compris, daté du 1er février 2020 ou après), puis les virements suivants.

**Un titre est introuvable dans la recherche.**
Essayez son code ISIN, ou son nom en anglais. Seuls les titres cotés **en euros** apparaissent.

**Le dividende net ne correspond pas à mon relevé.**
Vérifiez le **pays** de la société (bouton **Modifier**) et le **taux de retenue** de ce pays dans **Paramètres**. Le dividende annuel est celui des 12 derniers mois : si la société l'a augmenté ou baissé depuis, le prochain sera différent.

**Aucun dividende n'apparaît.**
La société n'a rien versé ces 12 derniers mois, ou les dividendes n'ont pas encore été récupérés : cliquez sur **Actualiser les cours**. Les ETF capitalisants ne versent pas de dividendes.

**Le cours ne se met pas à jour.**
Vérifiez votre connexion Internet. Si Yahoo Finance ne répond pas, un message apparaît en haut de la page : en attendant, saisissez le cours à la main avec le bouton **Cours**.

**Le graphique affiche « historique trop court ».**
Le titre est coté depuis moins de 30 séances, ou la source ne fournit pas son historique.

**Les taux des livrets ont changé.**
Les taux sont révisés au 1er février et au 1er août. Ils sont inclus dans €scarcell : mettez à jour le logiciel après chaque révision.

**€scarcell peut-il passer des ordres ou se connecter à ma banque ?**
Non. €scarcell est un outil de suivi : il ne se connecte à aucune banque et ne passe aucun ordre.

---

*€scarcell est un logiciel libre (licence GPL-3.0), développé par ChatCarbo. Simulations à titre informatif : ne remplace ni un conseiller en investissements financiers ni l'administration fiscale.*
