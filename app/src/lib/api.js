import { invoke } from '@tauri-apps/api/core';

// Tauri v2 : les noms d'arguments des commandes passent en camelCase côté JavaScript ;
// les champs des structures restent en snake_case.
export const api = {
  ajouterOperation: (operation) => invoke('ajouter_operation', { operation }),
  supprimerOperation: (id) => invoke('supprimer_operation', { id }),
  tableauDeBord: () => invoke('tableau_de_bord'),
  // Livrets
  livrets: () => invoke('livrets'),
  ouvrirLivret: (typeLivret) => invoke('ouvrir_livret', { typeLivret }),
  conseilLivret: (typeLivret, sens, date, montantCents) => invoke('conseil_livret', { typeLivret, sens, date, montantCents }),
  repartirLivrets: (sens, montantCents) => invoke('repartir_livrets', { sens, montantCents }),
  // PEA : suivi de lignes
  positions: () => invoke('positions'),
  ajouterPosition: (saisie) => invoke('ajouter_position', { saisie }),
  modifierPosition: (id, quantiteMicro, pruMicro, pays) => invoke('modifier_position', { id, quantiteMicro, pruMicro, pays }),
  supprimerPosition: (id) => invoke('supprimer_position', { id }),
  actualiserCours: () => invoke('actualiser_cours'),
  saisirCours: (symbole, coursMicro) => invoke('saisir_cours', { symbole, coursMicro }),
  chercherTitres: (requete) => invoke('chercher_titres', { requete }),
  analyseTitre: (symbole, periode, intervalle) => invoke('analyse_titre', { symbole, periode, intervalle }),
  retenues: () => invoke('retenues'),
  definirRetenue: (pays, tauxMicro) => invoke('definir_retenue', { pays, tauxMicro }),
  // Alertes et événements
  evenements: () => invoke('evenements'),
  ajouterEvenement: (saisie) => invoke('ajouter_evenement', { saisie }),
  supprimerEvenement: (id) => invoke('supprimer_evenement', { id }),
  // Nouvelles versions
  verifierMiseAJour: () => invoke('verifier_mise_a_jour'),
  verificationAuto: (activer) => invoke('verification_auto', { activer }),
  // Réglages
  emplacementDonnees: () => invoke('emplacement_donnees'),
  sauvegarder: (chemin) => invoke('sauvegarder', { chemin }),
  restaurer: (chemin, motDePasse) => invoke('restaurer', { chemin, motDePasse: motDePasse || null }),
  sauvegarderChiffre: (chemin, motDePasse) => invoke('sauvegarder_chiffre', { chemin, motDePasse }),
  lireSauvegarde: (chemin, motDePasse) => invoke('lire_sauvegarde', { chemin, motDePasse: motDePasse || null }),
  toutEffacer: (confirmation) => invoke('tout_effacer', { confirmation }),
  // Import CSV
  champsImport: () => invoke('champs_import'),
  analyserImport: (chemin, correspondance, inclureDoublons) =>
    invoke('analyser_import', { chemin, correspondance: correspondance ?? null, inclureDoublons: !!inclureDoublons }),
  importer: (chemin, correspondance, inclureDoublons) =>
    invoke('importer', { chemin, correspondance: correspondance ?? null, inclureDoublons: !!inclureDoublons }),
  imports: () => invoke('imports'),
  annulerImport: (id) => invoke('annuler_import', { id }),
  correspondances: () => invoke('correspondances'),
  enregistrerCorrespondance: (nom, colonnes) => invoke('enregistrer_correspondance', { nom, colonnes }),
  supprimerCorrespondance: (nom) => invoke('supprimer_correspondance', { nom }),
  enregistrerModele: (chemin) => invoke('enregistrer_modele', { chemin }),
  analyserFichier: (chemin) => invoke('analyser_fichier', { chemin }),
  importerPositions: (chemin, supprimerAbsentes) => invoke('importer_positions', { chemin, supprimerAbsentes: !!supprimerAbsentes }),
};

export const TYPES = {
  LIVRET_A: 'Livret A',
  LDDS: 'LDDS',
  LEP: 'LEP',
  PEA: 'PEA',
};

/** Pays des sociétés (retenue à la source sur les dividendes), dans l'ordre de l'écran Paramètres. */
export const PAYS = {
  FR: 'France', DE: 'Allemagne', AT: 'Autriche', BE: 'Belgique', DK: 'Danemark', ES: 'Espagne', FI: 'Finlande',
  IE: 'Irlande', IT: 'Italie', LU: 'Luxembourg', NL: 'Pays-Bas', NO: 'Norvège', PL: 'Pologne', PT: 'Portugal',
  CZ: 'Tchéquie', SE: 'Suède', GR: 'Grèce',
};
