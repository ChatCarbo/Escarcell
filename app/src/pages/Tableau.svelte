<script>
  import { onMount } from 'svelte';
  import { api, TYPES } from '../lib/api.js';
  import { euros } from '../lib/format.js';
  import { exporterCsv, exporterImage, eurosCsv } from '../lib/export.js';
  import Donut from '../lib/Donut.svelte';

  let tdb = $state(null);
  let erreur = $state('');

  let racine = $state(null);
  let messageExport = $state('');

  async function exporterComptes() {
    messageExport = '';
    const lignes = tdb.comptes.map((c) => [c.nom, TYPES[c.type] ?? c.type, eurosCsv(c.valeur_cents), eurosCsv(c.complement_cents)]);
    lignes.push(['Total', '', eurosCsv(tdb.total_cents), '']);
    try {
      const chemin = await exporterCsv('escarcelle-tableau-de-bord', ['Compte', 'Type', 'Valeur (€)', 'Intérêts estimés / plus-value latente (€)'], lignes);
      if (chemin) messageExport = `Export enregistré : ${chemin}`;
    } catch (e) {
      erreur = String(e);
    }
  }

  async function exporterRepartition() {
    messageExport = '';
    try {
      const chemin = await exporterImage(
        'escarcelle-repartition',
        `Répartition de l'épargne au ${new Date(tdb.date + 'T00:00:00').toLocaleDateString('fr-FR')} : ${euros(tdb.total_cents)}`,
        [racine.querySelector('.donut svg')],
        tdb.par_enveloppe.filter((p) => p.montant_cents > 0).map((p) => `${p.libelle} ${euros(p.montant_cents)}`).join(' · '),
      );
      if (chemin) messageExport = `Image enregistrée : ${chemin}`;
    } catch (e) {
      erreur = String(e);
    }
  }

  onMount(async () => {
    try {
      tdb = await api.tableauDeBord();
    } catch (e) {
      erreur = String(e);
    }
  });
</script>

<header class="entete">
  <h1>Tableau de bord</h1>
  {#if tdb}<span class="discret">au {new Date(tdb.date + 'T00:00:00').toLocaleDateString('fr-FR')}</span>{/if}
  {#if tdb}
    <div class="boutons">
      <button onclick={exporterComptes} disabled={!tdb.comptes.length}>Exporter en CSV</button>
      <button onclick={exporterRepartition} disabled={!tdb.total_cents}>Exporter la répartition en image</button>
    </div>
  {/if}
</header>
{#if messageExport}<p class="discret petit">{messageExport}</p>{/if}

{#if erreur}<p class="erreur">{erreur}</p>{/if}

{#if tdb}
  <div class="kpis">
    <div class="kpi"><span>Patrimoine suivi</span><strong>{euros(tdb.total_cents)}</strong></div>
    <div class="kpi"><span>Livrets réglementés</span><strong>{euros(tdb.livrets_cents)}</strong></div>
    <div class="kpi"><span>Intérêts estimés cette année</span><strong>{euros(tdb.interets_estimes_cents)}</strong></div>
    <div class="kpi">
      <span>Plus-value latente du PEA</span>
      <strong class:positif={tdb.pv_latente_cents > 0} class:negatif={tdb.pv_latente_cents < 0}>{euros(tdb.pv_latente_cents)}</strong>
    </div>
    <div class="kpi" title="Somme des dividendes détachés ces 12 derniers mois, après retenues étrangères. Une hausse annoncée mais pas encore détachée n'est pas comptée.">
      <span>Dividendes nets des 12 derniers mois</span><strong class="positif">{euros(tdb.dividendes_nets_cents)}</strong>
    </div>
  </div>

  {#if tdb.alertes.length}
    <section class="carte">
      <h3>À venir (45 jours)</h3>
      {#each tdb.alertes as a}
        <div class="alerte-ligne">
          <span class="badge {a.type === 'dividende' ? 'positif' : 'attention'}">{a.type === 'dividende' ? 'Dividende' : a.type === 'ag' ? 'AG' : 'Événement'}</span>
          <div>
            <b>{new Date(a.date + 'T00:00:00').toLocaleDateString('fr-FR', { day: 'numeric', month: 'long' })}</b> · {a.titre}
            {#if a.enveloppe}<span class="discret petit">({a.enveloppe})</span>{/if}
            <br /><span class="discret">{a.message}</span>
          </div>
        </div>
      {/each}
    </section>
  {/if}

  <div bind:this={racine}><Donut titre="Répartition par enveloppe" parts={tdb.par_enveloppe} /></div>

  <section class="carte">
    <h3>Comptes</h3>
    {#if tdb.comptes.length === 0}
      <p class="vide">Aucun compte. Commencez par la page Livrets ou PEA.</p>
    {:else}
      <table>
        <thead><tr><th>Compte</th><th>Type</th><th class="nombre">Valeur</th><th class="nombre">Intérêts estimés / PV latente</th></tr></thead>
        <tbody>
          {#each tdb.comptes as c (`${c.type}-${c.id}`)}
            <tr>
              <td>{c.nom}</td>
              <td>{TYPES[c.type] ?? c.type}</td>
              <td class="nombre">{euros(c.valeur_cents)}</td>
              <td class="nombre">{c.complement_cents ? euros(c.complement_cents) : '—'}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

{/if}
