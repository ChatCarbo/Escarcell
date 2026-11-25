# Guide d'utilisation d'Escarcelle

Escarcelle vous aide à suivre vos **livrets réglementés** (Livret A, LDDS, LEP), et votre **PEA**, directement sur votre ordinateur. Vos données ne quittent jamais votre machine : pas de compte à créer, pas de banque à connecter. Escarcelle lit seulement des cours publics sur Internet, sans rien transmettre de vos données.

---

## Sommaire

1. [Installer et lancer Escarcelle](#1-installer-et-lancer-escarcelle)
2. [Découvrir l'écran](#2-découvrir-lécran)
3. [Suivre ses livrets](#3-suivre-ses-livrets)
4. [Choisir le bon jour pour verser ou retirer](#4-choisir-le-bon-jour-pour-verser-ou-retirer)
5. [Suivre son PEA](#5-suivre-son-pea)
6. [Dividendes, impôts et assemblées générales](#6-dividendes-impôts-et-assemblées-générales)
7. [Lire la courbe et l'analyse technique](#7-lire-la-courbe-et-lanalyse-technique)
8. [Paramètres : sauvegarde, retenues, nouvelles versions](#8-paramètres--sauvegarde-retenues-nouvelles-versions)
9. [Questions fréquentes](#9-questions-fréquentes)

---

## 1. Installer et lancer Escarcelle

### Avec l'installateur

Téléchargez `Escarcelle_x.y.z_x64-setup.exe` depuis la page **Releases** du dépôt GitHub, lancez-le et suivez les étapes. Escarcelle apparaît ensuite dans le menu Démarrer.

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

Le numéro de version d'Escarcelle est affiché en bas du menu.

Si vous activez la vérification des nouvelles versions dans **Paramètres** (désactivée par défaut), un bandeau vert vous signale au démarrage qu'une nouvelle version est publiée, avec un lien vers la page de téléchargement.

Tous les montants sont en euros, au format français (1 234,56 €). Les gains apparaissent en vert, les pertes en rouge.

---

## 3. Suivre ses livrets

### Déclarer ses livrets

Dans **Livrets**, trois cartes s'affichent : LEP, Livret A, LDDS. Pour chaque livret que vous possédez, cliquez sur **« J'ai un … »**.

### Saisir un virement

1. Choisissez **Versement** ou **Retrait**.
2. Choisissez le livret.
3. Indiquez la **date de valeur** (celle qui figure sur votre relevé, pas forcément le jour où vous avez fait le virement). Escarcelle connaît les taux depuis le **1er février 2020** : pour un livret plus ancien, saisissez comme premier versement son solde à cette date (il figure sur votre relevé de début 2020).
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

> Escarcelle suit le **PEA**. Le compte-titres et le PEA-PME ne sont plus proposés depuis la version 0.3 ; si vous aviez saisi des lignes dans ces enveloppes avec la 0.2, elles sont supprimées à la première ouverture (faites une sauvegarde avec la 0.2 avant de mettre à jour si vous voulez les garder).

Escarcelle ne vous demande pas l'historique de vos achats : vous saisissez simplement ce que vous détenez, comme sur l'écran de votre banque.

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

---

## 6. Dividendes, impôts et assemblées générales

Cliquez sur le **nom d'une ligne** : sa fiche s'ouvre au-dessus de la courbe.

### Les dividendes

| Donnée | Signification |
| :--- | :--- |
| Dividende annuel / action | Total versé ces 12 derniers mois, et rythme (annuel, semestriel, trimestriel) |
| Rendement | Dividende annuel ÷ cours actuel |
| Croissance | Hausse moyenne du dividende par an ces dernières années |
| Prochain détachement | Date estimée d'après celle de l'an dernier : à confirmer sur le site de la société |

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

Sous les graphiques, Escarcelle liste les signaux détectés, chacun avec son explication :

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

### Sauvegarder

Cliquez sur **Sauvegarder Escarcelle…** et choisissez où enregistrer le fichier (`escarcelle-2026-11-23.db`) : clé USB, disque externe, dossier synchronisé… Une sauvegarde par mois est un bon rythme, et une avant chaque mise à jour d'Escarcelle.

### Restaurer

Cliquez sur **Choisir une sauvegarde…**, sélectionnez le fichier, puis confirmez. **Toutes les données actuelles sont remplacées** par celles de la sauvegarde. Escarcelle vérifie d'abord qu'il s'agit bien d'une sauvegarde Escarcelle.

### Tout effacer

Tapez **EFFACER** en majuscules, puis cliquez sur **Tout effacer**. Livrets, virements, lignes du PEA, cours, dividendes, AG et réglages sont supprimés définitivement. Faites une sauvegarde avant.

### Retenues à la source

Le tableau liste le taux retenu par chaque pays sur les dividendes. Les valeurs proposées sont indicatives : si votre relevé montre un autre taux (par exemple parce que votre banque applique le taux réduit d'une convention fiscale), cliquez sur **Modifier**. **Rétablir** revient à la valeur proposée.

### Nouvelles versions

La case **Vérifier au démarrage** est **décochée par défaut** : Escarcelle ne contacte pas GitHub sans votre accord. Cochez-la pour être prévenu des nouvelles versions, ou cliquez de temps en temps sur **Vérifier maintenant**. Escarcelle ne fait que lire la liste des versions publiées : aucune de vos données n'est envoyée, et rien n'est téléchargé ni installé sans vous.

> **Connexions à Internet.** Escarcelle communique avec l'extérieur pour lire des données publiques, sans jamais transmettre vos données : Yahoo Finance pour les cours et les dividendes (seul le nom, l'ISIN ou le symbole du titre est envoyé, jamais vos quantités ou montants), et GitHub uniquement pour la vérification des versions. Les livrets et le tableau de bord fonctionnent hors ligne.

### Où sont mes données ?

L'emplacement exact est affiché dans **Paramètres**. Sous Windows : `%APPDATA%\fr.chatcarbo.escarcelle\escarcelle.db`.

---

### Exporter

- **Exporter en CSV** (tableau de bord, livrets, PEA) : un fichier qui s'ouvre directement dans Excel ou LibreOffice, avec toutes les colonnes et un total.
- **Exporter en image** (courbe d'un titre, répartition du tableau de bord) : une image PNG nette, prête à coller dans un document ou un message.

Une fenêtre vous demande où enregistrer le fichier.

---

## 9. Questions fréquentes

**Les intérêts calculés ne correspondent pas exactement à mon relevé.**
Vérifiez que les dates saisies sont bien les **dates de valeur** du relevé. Si vous avez démarré avec un versement unique égal à votre solde, les intérêts de la première année sont sous-estimés (les quinzaines antérieures ne sont pas connues d'Escarcelle).

**Escarcelle refuse un virement daté d'avant 2020.**
Les taux des livrets sont connus depuis le 1er février 2020. Saisissez votre solde à cette date comme premier versement, daté du 1er février 2020, puis les virements suivants.

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
Les taux sont révisés au 1er février et au 1er août. Ils sont inclus dans Escarcelle : mettez à jour le logiciel après chaque révision.

**Escarcelle peut-il passer des ordres ou se connecter à ma banque ?**
Non. Escarcelle est un outil de suivi : il ne se connecte à aucune banque et ne passe aucun ordre.

---

*Escarcelle est un logiciel libre (licence GPL-3.0), développé par ChatCarbo. Simulations à titre informatif : ne remplace ni un conseiller en investissements financiers ni l'administration fiscale.*
