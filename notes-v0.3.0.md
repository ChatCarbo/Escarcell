## Escarcelle 0.3.0 — stabilisation

Version de stabilisation : moins de fonctions, des calculs plus justes et mieux testés.

### ⚠️ À lire avant de mettre à jour depuis la 0.2

Le **compte-titres (CTO)** et le **PEA-PME** sont retirés. À la première ouverture, les lignes de ces deux enveloppes sont **supprimées** de la base. Si vous voulez les garder, faites une sauvegarde avec la 0.2 avant d'installer la 0.3 (Paramètres → Sauvegarder).

Les livrets et le PEA ne sont pas touchés.

### Livrets réglementés

- **Solde de départ** : saisissez le solde d'un relevé (intérêts compris) au lieu de ressaisir des années de virements. Il peut dépasser le plafond, et un solde au 31 décembre ne rapporte pas une seconde fois la fin d'année.
- **Taux historiques** du Livret A, du LDDS et du LEP depuis le 1er février 2020 (sources : barèmes IPP, Banque de France).
- **Plafond et solde contrôlés à la date de valeur** : un versement daté de 2021 se juge sur le solde de 2021, pas sur celui d'aujourd'hui. Un retrait et un versement du même jour sont acceptés au plafond, quel que soit l'ordre de saisie.
- Un virement antidaté ou une suppression qui rendrait impossible une opération existante est refusé, avec une explication.
- **Conseil de répartition** expliqué ; à taux égal, retrait sur le LDDS avant le Livret A ; alerte si le livret choisi est déjà plein.
- Corrections : message « Quand retirer ? » faux sans montant saisi ; date du jour prise en heure UTC (la veille entre minuit et 1 h).

### PEA

- **Variation sur 1 an** calculée sur la séance d'il y a un an (et non sur la fin du mois).
- Titres cotés en euros uniquement ; symboles et saisies vérifiés avant tout appel réseau.
- Retenue à la source de la Grèce (5 %) ajoutée.

### Stabilité

- Aucune commande ne bloque la base pendant une requête réseau ; un plantage ne bloque plus les commandes suivantes.
- Toutes les saisies sont revérifiées par l'application ; toutes les erreurs sont affichées.
- Sauvegarde écrite dans un fichier temporaire puis renommée : un échec ne détruit plus l'ancienne sauvegarde.
- Plus de code mort ; vérification des nouvelles versions désactivée par défaut.

### Installation

Windows : téléchargez `Escarcelle_0.3.0_x64-setup.exe` ci-dessous et lancez-le. Windows SmartScreen peut afficher un avertissement (l'installateur n'est pas signé) : « Informations complémentaires » → « Exécuter quand même ».

Vos données restent sur votre ordinateur : `%APPDATA%\fr.chatcarbo.escarcelle\escarcelle.db`.

Un problème ? Ouvrez une issue avec le modèle « Problème ».
