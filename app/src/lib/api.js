import { invoke } from '@tauri-apps/api/core';

// Tauri v2 : les noms d'arguments des commandes passent en camelCase côté JavaScript ;
// les champs des structures restent en snake_case.
export const api = {
  etablissements: () => invoke('etablissements'),
  comptes: () => invoke('comptes'),
  ajouterCompte: (compte) => invoke('ajouter_compte', { compte }),
  operations: (compteId) => invoke('operations', { compteId }),
  ajouterOperation: (operation) => invoke('ajouter_operation', { operation }),
  supprimerOperation: (id) => invoke('supprimer_operation', { id }),
  enregistrerCours: (isin, dateCours, coursMicro, source) =>
    invoke('enregistrer_cours', { isin, dateCours, coursMicro, source }),
  tableauDeBord: () => invoke('tableau_de_bord'),
  impot: (revenuCents, foyer) => invoke('impot', { revenuCents, foyer }),
  economiePer: (revenuCents, foyer, versementCents, plafondCents) =>
    invoke('economie_per', { revenuCents, foyer, versementCents, plafondCents }),
  comparerPer: (hypotheses) => invoke('comparer_per', { hypotheses }),
  // Livrets
  livrets: () => invoke('livrets'),
  ouvrirLivret: (typeLivret) => invoke('ouvrir_livret', { typeLivret }),
  conseilLivret: (typeLivret, sens, date, montantCents) => invoke('conseil_livret', { typeLivret, sens, date, montantCents }),
  repartirLivrets: (sens, montantCents) => invoke('repartir_livrets', { sens, montantCents }),
  // Impôt, PER, dons
  simulationFiscale: (saisie) => invoke('simulation_fiscale', { saisie }),
  // PEA : suivi de lignes
  positions: () => invoke('positions'),
  ajouterPosition: (saisie) => invoke('ajouter_position', { saisie }),
  modifierPosition: (id, quantiteMicro, pruMicro) => invoke('modifier_position', { id, quantiteMicro, pruMicro }),
  supprimerPosition: (id) => invoke('supprimer_position', { id }),
  actualiserCours: () => invoke('actualiser_cours'),
  saisirCours: (symbole, coursMicro) => invoke('saisir_cours', { symbole, coursMicro }),
  chercherTitres: (requete) => invoke('chercher_titres', { requete }),
  analyseTitre: (symbole, periode) => invoke('analyse_titre', { symbole, periode }),
  // Réglages
  emplacementDonnees: () => invoke('emplacement_donnees'),
  sauvegarder: (chemin) => invoke('sauvegarder', { chemin }),
  restaurer: (chemin) => invoke('restaurer', { chemin }),
  toutEffacer: (confirmation) => invoke('tout_effacer', { confirmation }),
};

export const TYPES = {
  LIVRET_A: 'Livret A',
  LDDS: 'LDDS',
  LEP: 'LEP',
  PEA: 'PEA',
  PEA_PME: 'PEA-PME',
  PER: 'PER',
};
