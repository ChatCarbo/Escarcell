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
  // PEA et CTO : suivi de lignes (enveloppe = 'PEA' ou 'CTO')
  positions: (enveloppe) => invoke('positions', { enveloppe }),
  ajouterPosition: (enveloppe, saisie) => invoke('ajouter_position', { enveloppe, saisie }),
  modifierPosition: (enveloppe, id, quantiteMicro, pruMicro, pays) =>
    invoke('modifier_position', { enveloppe, id, quantiteMicro, pruMicro, pays }),
  supprimerPosition: (enveloppe, id) => invoke('supprimer_position', { enveloppe, id }),
  actualiserCours: (enveloppe) => invoke('actualiser_cours', { enveloppe }),
  saisirCours: (enveloppe, symbole, coursMicro) => invoke('saisir_cours', { enveloppe, symbole, coursMicro }),
  chercherTitres: (requete, enveloppe) => invoke('chercher_titres', { requete, enveloppe }),
  analyseTitre: (symbole, periode, intervalle) => invoke('analyse_titre', { symbole, periode, intervalle }),
  retenues: () => invoke('retenues'),
  definirRetenue: (pays, tauxMicro) => invoke('definir_retenue', { pays, tauxMicro }),
  // Alertes et événements
  alertes: () => invoke('alertes'),
  evenements: () => invoke('evenements'),
  ajouterEvenement: (saisie) => invoke('ajouter_evenement', { saisie }),
  supprimerEvenement: (id) => invoke('supprimer_evenement', { id }),
  // Nouvelles versions
  verifierMiseAJour: () => invoke('verifier_mise_a_jour'),
  verificationAuto: (activer) => invoke('verification_auto', { activer }),
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
  CTO: 'Compte-titres',
  PER: 'PER',
};

/** Pays des sociétés (retenue à la source sur les dividendes). */
export const PAYS = {
  FR: 'France', DE: 'Allemagne', AT: 'Autriche', BE: 'Belgique', DK: 'Danemark', ES: 'Espagne', FI: 'Finlande',
  IE: 'Irlande', IT: 'Italie', LU: 'Luxembourg', NL: 'Pays-Bas', NO: 'Norvège', PL: 'Pologne', PT: 'Portugal',
  CZ: 'Tchéquie', SE: 'Suède', GR: 'Grèce', US: 'États-Unis', CH: 'Suisse', GB: 'Royaume-Uni',
  CA: 'Canada', JP: 'Japon', HK: 'Hong Kong', AU: 'Australie',
};

/** Symbole des devises affichées. */
export const SYMBOLES_DEVISE = { EUR: '€', USD: '$', GBP: '£', CHF: 'CHF', JPY: '¥', CAD: 'CA$', SEK: 'SEK', DKK: 'DKK', NOK: 'NOK', HKD: 'HK$', AUD: 'AU$' };
export const symboleDevise = (d) => SYMBOLES_DEVISE[d] ?? d;
