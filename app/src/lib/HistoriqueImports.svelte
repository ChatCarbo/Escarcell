<script>
  // Imports précédents (photos du portefeuille et historiques d'opérations), annulables en bloc.
  import { api } from './api.js';

  let { imports, annule } = $props();

  let aAnnuler = $state(null);
  let erreur = $state(null);
  const nomFichier = (p) => String(p ?? '').split(/[\\/]/).pop();

  // Annulation en deux clics (les boîtes de dialogue du navigateur ne sont pas fiables dans Tauri).
  async function annuler(id) {
    if (aAnnuler !== id) return (aAnnuler = id);
    aAnnuler = null;
    erreur = null;
    try {
      await api.annulerImport(id);
      annule();
    } catch (e) {
      erreur = String(e);
    }
  }
</script>

{#if erreur}<p class="message alerte">{erreur}</p>{/if}
<div class="defilement">
  <table>
    <thead><tr><th>Date (UTC)</th><th>Fichier</th><th>Contenu</th><th class="nombre">Lignes</th><th></th></tr></thead>
    <tbody>
      {#each imports as h (h.id)}
        <tr>
          <td>{h.date}</td>
          <td class="titre-import">{nomFichier(h.fichier)}</td>
          <td>{h.genre === 'POSITIONS' ? 'Lignes du PEA' : `Opérations (${h.operations} restantes)`}</td>
          <td class="nombre">{h.lignes}</td>
          <td class="actions">
            <button class="discret" class:negatif={aAnnuler === h.id} onclick={() => annuler(h.id)}>
              {aAnnuler === h.id ? "Confirmer l'annulation ?" : 'Annuler cet import'}
            </button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>
