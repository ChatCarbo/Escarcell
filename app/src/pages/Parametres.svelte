<script>
  import { onMount } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { api } from '../lib/api.js';

  let emplacement = $state('');
  let message = $state(null); // { classe, texte }
  let confirmation = $state('');
  let restaurationEnAttente = $state(null);

  onMount(async () => (emplacement = await api.emplacementDonnees()));

  const aujourdhui = () => new Date().toISOString().slice(0, 10);
  const FILTRES = [{ name: 'Sauvegarde Escarcelle', extensions: ['db'] }];

  async function sauvegarder() {
    message = null;
    const chemin = await save({ defaultPath: `escarcelle-${aujourdhui()}.db`, filters: FILTRES });
    if (!chemin) return;
    try {
      await api.sauvegarder(chemin);
      message = { classe: 'ok', texte: `Sauvegarde enregistrée : ${chemin}` };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function choisirRestauration() {
    message = null;
    const chemin = await open({ multiple: false, directory: false, filters: FILTRES });
    if (chemin) restaurationEnAttente = chemin;
  }

  async function restaurer() {
    try {
      await api.restaurer(restaurationEnAttente);
      message = { classe: 'ok', texte: 'Données restaurées. Les autres pages afficheront les données de la sauvegarde.' };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    } finally {
      restaurationEnAttente = null;
    }
  }

  async function effacer() {
    try {
      await api.toutEffacer(confirmation);
      confirmation = '';
      message = { classe: 'ok', texte: 'Toutes les données ont été effacées.' };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }
</script>

<header class="entete"><h1>Paramètres</h1></header>
{#if message}<p class="message {message.classe}">{message.texte}</p>{/if}

<section class="carte">
  <h3>Sauvegarder</h3>
  <p>Toutes vos données (livrets, lignes du PEA, historique des cours) tiennent dans un seul fichier. Enregistrez-en une copie où vous voulez : clé USB, disque externe, dossier synchronisé.</p>
  <button class="principal" onclick={sauvegarder}>Sauvegarder Escarcelle…</button>
  <p class="discret petit">Fichier de données actuel : {emplacement}</p>
</section>

<section class="carte">
  <h3>Restaurer une sauvegarde</h3>
  <p>Remplace toutes les données actuelles par celles d'une sauvegarde.</p>
  {#if restaurationEnAttente}
    <p class="message attention">Remplacer toutes les données actuelles par <b>{restaurationEnAttente}</b> ? Pensez à sauvegarder d'abord si besoin.</p>
    <button class="principal" onclick={restaurer}>Oui, restaurer</button>
    <button onclick={() => (restaurationEnAttente = null)}>Annuler</button>
  {:else}
    <button onclick={choisirRestauration}>Choisir une sauvegarde…</button>
  {/if}
</section>

<section class="carte zone-danger">
  <h3>Tout effacer</h3>
  <p>Supprime définitivement tous les livrets, virements, lignes du PEA et cours enregistrés. Cette action est irréversible : faites une sauvegarde avant.</p>
  <label>Tapez EFFACER pour confirmer<input bind:value={confirmation} placeholder="EFFACER" /></label>
  <button class="danger" disabled={confirmation.trim() !== 'EFFACER'} onclick={effacer}>Tout effacer</button>
</section>
