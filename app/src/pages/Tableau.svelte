<script>
  import { onMount } from 'svelte';
  import { api, TYPES } from '../lib/api.js';
  import { euros } from '../lib/format.js';
  import Donut from '../lib/Donut.svelte';

  let tdb = $state(null);
  let erreur = $state('');

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
  {#if tdb}<span class="discret">au {new Date(tdb.date).toLocaleDateString('fr-FR')}</span>{/if}
</header>

{#if erreur}<p class="erreur">{erreur}</p>{/if}

{#if tdb}
  <div class="kpis">
    <div class="kpi"><span>Patrimoine suivi</span><strong>{euros(tdb.total_cents)}</strong></div>
    <div class="kpi"><span>Livrets réglementés</span><strong>{euros(tdb.livrets_cents)}</strong></div>
    <div class="kpi"><span>Intérêts estimés cette année</span><strong>{euros(tdb.interets_estimes_cents)}</strong></div>
    <div class="kpi">
      <span>Plus-value latente PEA</span>
      <strong class:positif={tdb.pv_latente_cents > 0} class:negatif={tdb.pv_latente_cents < 0}>{euros(tdb.pv_latente_cents)}</strong>
    </div>
  </div>

  <Donut titre="Répartition par enveloppe" parts={tdb.par_enveloppe} />

  <section class="carte">
    <h3>Comptes</h3>
    {#if tdb.comptes.length === 0}
      <p class="vide">Aucun compte. Commencez par la page Livrets ou PEA.</p>
    {:else}
      <table>
        <thead><tr><th>Compte</th><th>Type</th><th class="nombre">Valeur</th><th class="nombre">Intérêts estimés / PV latente</th></tr></thead>
        <tbody>
          {#each tdb.comptes as c (c.id)}
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

  {#if tdb.alertes.length}
    <section class="alertes">
      {#each tdb.alertes as a}<p>{a}</p>{/each}
    </section>
  {/if}
{/if}
