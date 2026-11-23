<script>
  import { onMount } from 'svelte';
  import { api } from './lib/api.js';
  import Tableau from './pages/Tableau.svelte';
  import Livrets from './pages/Livrets.svelte';
  import Pea from './pages/Pea.svelte';
  import Parametres from './pages/Parametres.svelte';

  const PAGES = [
    { id: 'tableau', libelle: 'Tableau de bord' },
    { id: 'livrets', libelle: 'Livrets' },
    { id: 'pea', libelle: 'PEA' },
    { id: 'parametres', libelle: 'Paramètres' },
  ];

  let page = $state('tableau');
  let catalogue = $state(null);

  onMount(async () => {
    catalogue = await api.etablissements();
  });
</script>

<div class="application">
  <nav class="barre">
    <div class="marque">
      <svg viewBox="0 0 128 128" width="28" height="28" aria-hidden="true">
        <path d="M64 12L108 28V62C108 91.5 89.2 112.8 64 120C38.8 112.8 20 91.5 20 62V28L64 12Z" fill="#0F172A" />
        <rect x="42" y="68" width="10" height="24" rx="3" fill="#38BDF8" />
        <rect x="59" y="54" width="10" height="38" rx="3" fill="#34D399" />
        <rect x="76" y="40" width="10" height="52" rx="3" fill="#FBBF24" />
      </svg>
      <span>Escarcelle</span>
    </div>
    {#each PAGES as p (p.id)}
      <button class:actif={page === p.id} onclick={() => (page = p.id)}>{p.libelle}</button>
    {/each}
    <p class="mention">Simulations à titre informatif. Ne remplace ni un conseiller (CIF) ni l'administration fiscale.</p>
  </nav>

  <main>
    {#if page === 'tableau'}
      <Tableau />
    {:else if page === 'livrets'}
      <Livrets />
    {:else if page === 'pea'}
      <Pea />
    {:else if page === 'parametres'}
      <Parametres />
    {/if}
  </main>
</div>
