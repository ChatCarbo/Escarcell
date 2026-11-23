# Guide d'utilisation d'Escarcelle

Escarcelle vous aide à suivre vos **livrets réglementés** (Livret A, LDDS, LEP) et votre **PEA**, directement sur votre ordinateur. Vos données ne quittent jamais votre machine : pas de compte à créer, pas de banque à connecter.

---

## Sommaire

1. [Installer et lancer Escarcelle](#1-installer-et-lancer-escarcelle)
2. [Découvrir l'écran](#2-découvrir-lécran)
3. [Suivre ses livrets](#3-suivre-ses-livrets)
4. [Choisir le bon jour pour verser ou retirer](#4-choisir-le-bon-jour-pour-verser-ou-retirer)
5. [Suivre son PEA](#5-suivre-son-pea)
6. [Lire la courbe et l'analyse technique](#6-lire-la-courbe-et-lanalyse-technique)
7. [Sauvegarder, restaurer, tout effacer](#7-sauvegarder-restaurer-tout-effacer)
8. [Questions fréquentes](#8-questions-fréquentes)

---

## 1. Installer et lancer Escarcelle

### Avec l'installateur

Lancez `Escarcelle_0.1.0_x64-setup.exe` (ou le `.msi`) et suivez les étapes. Escarcelle apparaît ensuite dans le menu Démarrer.

### Depuis le code source (Windows)

Installez une seule fois :
- **Rust** : https://rustup.rs
- **Node.js** (version LTS) : `winget install OpenJS.NodeJS.LTS`
- **Visual Studio Build Tools**, avec l'option « Développement Desktop en C++ »

Fermez et rouvrez PowerShell, puis dans le dossier `escarcelle\app` :

```powershell
npm install
npm run tauri icon src-tauri/icons/logo.svg   # une seule fois
npm run tauri dev                             # lance Escarcelle
npm run tauri build                           # crée l'installateur
```

La première compilation prend plusieurs minutes. L'installateur est créé dans `app\src-tauri\target\release\bundle\`.

---

## 2. Découvrir l'écran

À gauche, le menu :

| Menu | À quoi ça sert |
| :--- | :--- |
| **Tableau de bord** | Vue d'ensemble : patrimoine suivi, intérêts de l'année, plus-value du PEA, répartition |
| **Livrets** | Vos virements sur Livret A, LDDS et LEP, et les conseils de dates |
| **PEA** | Vos lignes d'actions et d'ETF, leurs cours et leurs courbes |
| **Paramètres** | Sauvegarde, restauration, effacement |

Tous les montants sont en euros, au format français (1 234,56 €). Les gains apparaissent en vert, les pertes en rouge.

---

## 3. Suivre ses livrets

### Déclarer ses livrets

Dans **Livrets**, trois cartes s'affichent : LEP, Livret A, LDDS. Pour chaque livret que vous possédez, cliquez sur **« J'ai un … »**.

### Saisir un virement

1. Choisissez **Versement** ou **Retrait**.
2. Choisissez le livret.
3. Indiquez la **date de valeur** (celle qui figure sur votre relevé, pas forcément le jour où vous avez fait le virement).
4. Indiquez le montant, puis **Enregistrer**.

Escarcelle refuse un versement qui dépasserait le plafond (22 950 € pour le Livret A, 12 000 € pour le LDDS, 10 000 € pour le LEP) et un retrait supérieur au solde.

> **Astuce pour démarrer** : inutile de ressaisir des années d'historique. Faites un seul versement du montant de votre solde actuel, daté du jour. Les intérêts de l'année en cours seront un peu sous-estimés la première année, puis tout sera exact.

### Ce que vous voyez

Chaque carte affiche le **solde**, une **jauge** de remplissage du plafond et les **intérêts estimés** de l'année. Les intérêts sont ajoutés au solde chaque 31 décembre, comme le fait votre banque.

### Où verser, où retirer ?

Quand vous tapez un montant, Escarcelle propose une répartition :
- pour un **versement** : d'abord le LEP (le mieux rémunéré), puis le Livret A et le LDDS, sans dépasser les plafonds ;
- pour un **retrait** : d'abord le livret le moins rémunéré.

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

Pendant la saisie, Escarcelle dessine les jours autour de la date choisie :

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

Escarcelle ne vous demande pas l'historique de vos achats : vous saisissez simplement ce que vous détenez, comme sur l'écran de votre banque.

### Ajouter une ligne

1. Dans **Ajouter une ligne**, tapez le nom, le mnémonique ou le code ISIN du titre : « LVMH », « MC », « FR0000121014 », « MSCI World »…
2. La liste se remplit pendant que vous tapez. Chaque résultat indique la place de cotation et l'**éligibilité au PEA** :
   - **probable** : société européenne ;
   - **à vérifier** : ETF ou titre coté en Allemagne ;
   - **non** : société hors d'Europe.
3. Cliquez sur le bon titre (de préférence celui coté à **Paris**, en tête de liste).
4. Saisissez la **quantité** et le **PRU** (prix de revient unitaire) tels qu'affichés par votre banque, puis **Ajouter**.

> L'éligibilité affichée est indicative. En cas de doute, vérifiez auprès de votre banque avant d'acheter.

### Le tableau des lignes

| Colonne | Signification |
| :--- | :--- |
| Quantité, PRU | Ce que vous avez saisi |
| Cours | Dernier cours connu, avec son heure |
| Valeur | Quantité × cours |
| +/− value | Valeur − (quantité × PRU), en euros et en % |

En haut : la valeur totale, le montant investi, la plus ou moins-value et la performance globale. En bas : un camembert de répartition de vos lignes.

### Les cours

- Ils se mettent à jour à l'ouverture de la page, puis **toutes les 5 minutes** tant que la page reste ouverte. Le bouton **Actualiser les cours** force une mise à jour.
- Ils viennent de Yahoo Finance et ont en général **environ 15 minutes de retard**.
- Si un titre n'a pas de cours, cliquez sur **Cours** (ou **saisir**) pour l'entrer vous-même.

### Modifier après un achat ou une vente

Cliquez sur **Modifier**, mettez à jour la quantité et le PRU affichés par votre banque, puis **Enregistrer**. Pour une ligne vendue entièrement, cliquez sur ✕ puis **Confirmer ?**.

---

## 6. Lire la courbe et l'analyse technique

Cliquez sur le **nom d'une ligne** (ou sur un résultat de recherche) pour ouvrir sa courbe.

### La courbe

- Choisissez la période : 3 mois, 6 mois, 1 an, 2 ans ou 5 ans.
- La ligne **orange pointillée** est votre PRU : au-dessus, vous êtes gagnant.
- Passez la souris sur la courbe pour voir la date, le cours et le RSI.

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

Sous les graphiques, Escarcelle liste les signaux détectés, chacun avec son explication :

| Signal | Ce qu'il veut dire |
| :--- | :--- |
| **Cassure de résistance** | Le cours a franchi un plafond qui le bloquait. Plus solide si les volumes augmentent. |
| **Cassure de support** | Le cours est passé sous un plancher qui le soutenait. |
| **Rebond possible sur support** | Le cours repart à la hausse juste au-dessus d'un plancher. |
| **Croisement doré / de la mort** | La moyenne 50 jours passe au-dessus / en dessous de la 200 jours. |
| **Surachat / survente** | Le titre a beaucoup monté / baissé récemment. |
| **Compression des bandes** | Un mouvement fort se prépare, sans indication de sens. |

Une **synthèse** résume l'ensemble : plutôt à la hausse, plutôt à la baisse, ou pas de tendance nette.

> ⚠️ **À lire avant de s'en servir.** L'analyse technique décrit le passé du cours. Elle ne prédit pas l'avenir de façon fiable, et beaucoup de signaux se révèlent faux, surtout sur quelques jours. Ce n'est pas un conseil d'achat ou de vente. Pour un investissement à long terme dans un PEA, la diversification et la durée comptent bien plus que le moment d'achat.

---

## 7. Sauvegarder, restaurer, tout effacer

Tout se passe dans **Paramètres**.

### Sauvegarder

Cliquez sur **Sauvegarder Escarcelle…** et choisissez où enregistrer le fichier (`escarcelle-2026-11-23.db`) : clé USB, disque externe, dossier synchronisé… Une sauvegarde par mois est un bon rythme, et une avant chaque mise à jour d'Escarcelle.

### Restaurer

Cliquez sur **Choisir une sauvegarde…**, sélectionnez le fichier, puis confirmez. **Toutes les données actuelles sont remplacées** par celles de la sauvegarde. Escarcelle vérifie d'abord qu'il s'agit bien d'une sauvegarde Escarcelle.

### Tout effacer

Tapez **EFFACER** en majuscules, puis cliquez sur **Tout effacer**. Livrets, virements, lignes du PEA et cours enregistrés sont supprimés définitivement. Faites une sauvegarde avant.

### Où sont mes données ?

L'emplacement exact est affiché dans **Paramètres**. Sous Windows : `%APPDATA%\fr.chatcarbo.escarcelle\escarcelle.db`.

---

## 8. Questions fréquentes

**Les intérêts calculés ne correspondent pas exactement à mon relevé.**
Vérifiez que les dates saisies sont bien les **dates de valeur** du relevé. Si vous avez démarré avec un versement unique égal à votre solde, les intérêts de la première année sont sous-estimés (les quinzaines antérieures ne sont pas connues d'Escarcelle).

**Un titre est introuvable dans la recherche.**
Essayez son code ISIN, ou son nom en anglais. Escarcelle n'affiche que les titres cotés **en euros** : les actions cotées à Stockholm, Copenhague ou Oslo ne sont pas encore gérées.

**Le cours ne se met pas à jour.**
Vérifiez votre connexion Internet. La source peut aussi être temporairement indisponible : le message apparaît en haut de la page PEA. En attendant, saisissez le cours à la main avec le bouton **Cours**.

**Le graphique affiche « historique trop court ».**
Le titre est coté depuis moins de 30 séances, ou la source ne fournit pas son historique.

**Les taux des livrets ont changé.**
Les taux sont révisés au 1er février et au 1er août. Ils sont inclus dans Escarcelle : mettez à jour le logiciel après chaque révision.

**Escarcelle peut-il passer des ordres ou se connecter à ma banque ?**
Non. Escarcelle est un outil de suivi : il ne se connecte à aucune banque et ne passe aucun ordre.

---

*Escarcelle est un logiciel libre (licence GPL-3.0), développé par ChatCarbo. Simulations à titre informatif : ne remplace ni un conseiller en investissements financiers ni l'administration fiscale.*
