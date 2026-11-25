# Campagne de vérification de la 0.3

La 0.3 n'ajoute aucune fonction. Elle doit montrer qu'Escarcelle calcule juste, sur de vraies données.
Ce document décrit ce qui est déjà vérifié automatiquement, puis ce qui ne peut être vérifié que par
l'utilisateur, avec ses propres relevés.

## 1. Déjà vérifié par les tests (CI)

| Règle | Source officielle | Test |
| :--- | :--- | :--- |
| Dates de valeur des livrets (dépôt et retrait, avant et après le 15) | service-public.gouv.fr, fiche F2365 | `tests/reference.rs`, tests `f2365_*` |
| Intérêts ajoutés au capital au 31 décembre, puis rémunérés | F2365 | `f2365_interets_ajoutes_au_capital_au_31_decembre` |
| Taux des livrets depuis le 01/02/2020 (Livret A, LDDS, LEP) | barèmes IPP (service-public.fr), Banque de France | `params::tests`, `livrets_simulables_pour_toute_date_depuis_le_debut_des_taux` |
| Clé de contrôle des ISIN | norme ISO 6166 | `identifiants::tests` |
| PEA : crédits d'impôt non restitués, retenue étrangère perdue | BOFiP BOI-RPPM-RCM-40-50-30 | `pea_aucun_impot_francais_et_retenue_non_restituable` |

## 2. À vérifier avec vos relevés

Pour chaque point : faites la comparaison, notez le résultat dans le tableau, et ouvrez une issue
(modèle « Problème ») dès qu'un écart dépasse 1 centime, avec la capture du relevé (montants
personnels masqués si vous le souhaitez).

### 2.1 Intérêts d'un livret sur une année complète

1. Prenez le relevé annuel d'un Livret A, LDDS ou LEP (intérêts crédités au 31 décembre d'une année passée).
2. Dans Escarcelle, saisissez **tous** les virements de cette année avec leurs **dates de valeur** (pas les dates d'opération), plus un versement au 1er janvier égal au solde du 31 décembre précédent.
3. Comparez les intérêts calculés par Escarcelle aux intérêts du relevé.

| Livret | Année | Intérêts du relevé | Intérêts Escarcelle | Écart | Issue |
| :--- | :--- | ---: | ---: | ---: | :--- |
| | | | | | |

Causes d'écart attendues : date d'opération saisie au lieu de la date de valeur ; banque qui arrondit
par quinzaine au lieu d'une fois par an ; changement de taux en cours d'année (1er février, 1er août).

### 2.2 Dividende d'une action étrangère dans le PEA

Le net du relevé doit être : brut − retenue du pays. Si votre banque applique un autre taux, corrigez la
retenue du pays dans Paramètres et ouvrez une issue pour mettre à jour la valeur par défaut.

| Titre | Pays | Brut relevé | Net relevé | Net Escarcelle | Écart | Issue |
| :--- | :--- | ---: | ---: | ---: | ---: | :--- |
| | | | | | | |

### 2.3 Mise à jour d'une base 0.2

Si vous avez utilisé la 0.2 : faites une sauvegarde avec la 0.2, installez la 0.3, puis vérifiez que les livrets
et les lignes du PEA sont intacts. Les lignes du compte-titres et du PEA-PME sont supprimées (c'est voulu).

## 3. Utilisation réelle (2 à 3 semaines)

Utilisez Escarcelle comme seul outil de suivi pendant deux à trois semaines. Ouvrez une issue pour
**chaque** problème, même petit : calcul, affichage, texte peu clair, lenteur, plantage.

Règles de la 0.3 :
- une issue = un problème, avec les étapes pour le reproduire ;
- chaque correction de calcul s'accompagne d'un test qui échouait avant la correction ;
- aucune nouvelle fonction : une idée de fonction devient une issue avec l'étiquette `idée`, pour plus tard ;
- la 0.3 est publiée quand il n'y a plus d'issue ouverte avec l'étiquette `bug` et que les tableaux du §2 sont remplis.

## 4. Issues à ouvrir pour suivre la campagne

Avec GitHub CLI (`gh auth login` une fois), depuis le dossier du projet :

```powershell
gh label create "0.3" --color 0E8A16 --description "Stabilisation 0.3"
gh label create "idée" --color C5DEF5 --description "Pour une version ultérieure"
gh issue create --label "0.3" --title "Vérifier les intérêts d'un livret sur une année complète" --body "Voir docs/VERIFICATION.md §2.1"
gh issue create --label "0.3" --title "Vérifier un dividende étranger dans le PEA" --body "Voir docs/VERIFICATION.md §2.2"
gh issue create --label "0.3" --title "Vérifier la mise à jour d'une base 0.2" --body "Voir docs/VERIFICATION.md §2.3"
gh issue create --label "0.3" --title "Utilisation réelle pendant 3 semaines" --body "Une issue par problème rencontré. Voir docs/VERIFICATION.md §3"
gh issue create --label "0.3" --title "Vérifier les retenues à la source par défaut sur les relevés" --body "Les retenues proposées (Paramètres) sont indicatives : les confirmer avec de vrais avis de dividende, pays par pays."
```
