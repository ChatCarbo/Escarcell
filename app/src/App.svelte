<script>
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { api } from './lib/api.js';
  import Tableau from './pages/Tableau.svelte';
  import Livrets from './pages/Livrets.svelte';
  import Portefeuille from './pages/Portefeuille.svelte';
  import Parametres from './pages/Parametres.svelte';

  const PAGES = [
    { id: 'tableau', libelle: 'Tableau de bord' },
    { id: 'livrets', libelle: 'Livrets' },
    { id: 'pea', libelle: 'PEA' },
  ];
  let version = $state('');

  let page = $state('tableau');

  let maj = $state(null); // nouvelle version disponible
  let majFermee = $state(false);

  onMount(async () => {
    version = await getVersion().catch(() => '');
    try {
      if (await api.verificationAuto(null)) {
        const r = await api.verifierMiseAJour();
        if (r.disponible) maj = r;
      }
    } catch {
      // Hors ligne ou GitHub indisponible : on réessaiera au prochain démarrage.
    }
  });
</script>

<div class="application">
  <nav class="barre">
    <div class="marque">
      <svg viewBox="0 0 128 128" width="28" height="28" aria-hidden="true">
        <path d="M64 8L112 25.5V62C112 94 91.8 117.2 64 125C36.2 117.2 16 94 16 62V25.5L64 8Z" fill="#0B1F3A" />
        <rect x="37" y="70" width="14" height="28" rx="3" fill="#2F6BEA" />
        <rect x="57" y="55" width="14" height="43" rx="3" fill="#FFFFFF" />
        <rect x="77" y="38" width="14" height="60" rx="3" fill="#E8413A" />
      </svg>
      <span>€scarcell</span>
    </div>
    {#each PAGES as p (p.id)}
      <button class:actif={page === p.id} onclick={() => (page = p.id)}>{p.libelle}</button>
    {/each}
    <p class="mention">Simulations à titre informatif. Ne remplace ni un conseiller (CIF) ni l'administration fiscale.</p>
    {#if version}<p class="mention version">Version {version}</p>{/if}
  </nav>

  <div class="colonne">
  <header class="barre-haut">
    <button class="reglages" class:actif={page === 'parametres'} onclick={() => (page = 'parametres')} title="Paramètres">
      <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"><path fill="currentColor" d="M19.4 13a7.6 7.6 0 0 0 0-2l2.1-1.6-2-3.5-2.5 1a7.5 7.5 0 0 0-1.7-1L15 3h-4l-.4 2.9a7.5 7.5 0 0 0-1.7 1l-2.5-1-2 3.5L6.6 11a7.6 7.6 0 0 0 0 2l-2.1 1.6 2 3.5 2.5-1a7.5 7.5 0 0 0 1.7 1L11 21h4l.4-2.9a7.5 7.5 0 0 0 1.7-1l2.5 1 2-3.5L19.4 13ZM13 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7Z" transform="translate(-1 0)"/></svg>
      Paramètres
    </button>
  </header>
  <main>
    {#if maj && !majFermee}
      <p class="message ok bandeau-maj">
        €scarcell {maj.derniere_version} est disponible (vous avez la {maj.version_actuelle}).
        <a href={maj.url} target="_blank" rel="noreferrer">Voir la nouvelle version</a>
        <button class="discret" onclick={() => (majFermee = true)} aria-label="Fermer">✕</button>
      </p>
    {/if}
    {#if page === 'tableau'}
      <Tableau />
    {:else if page === 'livrets'}
      <Livrets />
    {:else if page === 'pea'}
      <Portefeuille />
    {:else if page === 'parametres'}
      <Parametres />
    {/if}
  </main>
  </div>
</div>
