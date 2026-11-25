<script>
  import { onMount } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { api } from '../lib/api.js';
  import { pct, pctVersMicro } from '../lib/format.js';

  let emplacement = $state('');
  let message = $state(null); // { classe, texte }
  let confirmation = $state('');
  let restaurationEnAttente = $state(null);

  let retenues = $state([]);
  let editionRetenue = $state(null); // { pays, valeur }
  let majAuto = $state(false);
  let maj = $state(null);
  let verification = $state(false);

  onMount(async () => {
    try {
      emplacement = await api.emplacementDonnees();
      retenues = await api.retenues();
      majAuto = await api.verificationAuto(null);
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  });

  async function enregistrerRetenue() {
    const t = pctVersMicro(editionRetenue.valeur);
    if (t === null) return (message = { classe: 'alerte', texte: 'Taux invalide.' });
    try {
      retenues = await api.definirRetenue(editionRetenue.pays, t);
      editionRetenue = null;
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function retenueParDefaut(pays) {
    try {
      retenues = await api.definirRetenue(pays, null);
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function basculerMajAuto() {
    try {
      majAuto = await api.verificationAuto(!majAuto);
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function verifier() {
    verification = true;
    try {
      maj = await api.verifierMiseAJour();
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    } finally {
      verification = false;
    }
  }

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
  <p>Toutes vos données (livrets, lignes du PEA, dividendes, assemblées générales, réglages) tiennent dans un seul fichier. Enregistrez-en une copie où vous voulez : clé USB, disque externe, dossier synchronisé.</p>
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

<section class="carte">
  <h3>Retenues à la source sur les dividendes</h3>
  <p>
    Taux prélevés par le pays de la société avant le versement. Dans un PEA, cette retenue est perdue. Les retenues proposées sont indicatives :
    vérifiez-les sur vos relevés. 
  </p>
  <table>
    <thead><tr><th>Pays</th><th class="nombre">Retenue</th><th class="nombre">Par défaut</th><th></th></tr></thead>
    <tbody>
      {#each retenues as r (r.pays)}
        <tr>
          <td>{r.nom}</td>
          <td class="nombre">
            {#if editionRetenue?.pays === r.pays}
              <input class="court" bind:value={editionRetenue.valeur} inputmode="decimal" /> %
            {:else}
              {pct(r.taux_micro, 3)}
            {/if}
          </td>
          <td class="nombre discret">{pct(r.defaut_micro, 3)}</td>
          <td class="actions">
            {#if editionRetenue?.pays === r.pays}
              <button class="discret" onclick={enregistrerRetenue}>Enregistrer</button>
              <button class="discret" onclick={() => (editionRetenue = null)}>Annuler</button>
            {:else}
              <button class="discret" onclick={() => (editionRetenue = { pays: r.pays, valeur: (r.taux_micro / 1e4).toLocaleString('fr-FR') })}>Modifier</button>
              {#if r.taux_micro !== r.defaut_micro}<button class="discret" onclick={() => retenueParDefaut(r.pays)}>Rétablir</button>{/if}
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>

<section class="carte">
  <h3>Nouvelles versions</h3>
  <label class="case"><input type="checkbox" checked={majAuto} onchange={basculerMajAuto} /> Vérifier au démarrage si une nouvelle version est publiée sur GitHub (désactivé par défaut)</label>
  <p><button onclick={verifier} disabled={verification}>{verification ? 'Vérification…' : 'Vérifier maintenant'}</button></p>
  {#if maj}
    {#if maj.disponible}
      <p class="message ok">Escarcelle {maj.derniere_version} est disponible (vous avez la {maj.version_actuelle}).
        Téléchargez-la depuis la page <a href={maj.url} target="_blank" rel="noreferrer">de la version</a>.</p>
    {:else}
      <p class="message">Vous avez la dernière version ({maj.version_actuelle}).</p>
    {/if}
  {/if}
  <p class="discret petit">
    La vérification envoie une requête à api.github.com : GitHub voit votre adresse IP, comme pour toute page web, mais aucune
    donnée d'Escarcelle n'est transmise. Rien n'est téléchargé ni installé automatiquement.
  </p>
</section>

<section class="carte zone-danger">
  <h3>Tout effacer</h3>
  <p>Supprime définitivement tous les livrets, virements, lignes du PEA, cours, dividendes, AG et réglages. Cette action est irréversible : faites une sauvegarde avant.</p>
  <label>Tapez EFFACER pour confirmer<input bind:value={confirmation} placeholder="EFFACER" /></label>
  <button class="danger" disabled={confirmation.trim() !== 'EFFACER'} onclick={effacer}>Tout effacer</button>
</section>
