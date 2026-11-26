<script>
  // Aperçu et import en un clic d'un export de portefeuille (Moning, BoursoBank, tableur…).
  import { api } from './api.js';
  import { microVersTexte } from './format.js';

  let { chemin, analyse, fini, fermer } = $props();

  let supprimerAbsentes = $state(false);
  let enCours = $state(false);
  let erreur = $state(null);
  let detail = $state(false);

  const c = $derived(analyse.compteurs);
  const aEcrire = $derived(c.nouvelles + c.mises_a_jour + (supprimerAbsentes ? analyse.absentes.length : 0));
  const STATUTS = {
    nouvelle: { texte: 'Nouvelle', classe: 'positif' },
    mise_a_jour: { texte: 'Mise à jour', classe: 'attention' },
    identique: { texte: 'Inchangée', classe: '' },
    ignoree: { texte: 'Ignorée', classe: '' },
    erreur: { texte: 'Erreur', classe: 'negatif' },
  };
  // Les lignes inchangées ou ignorées ne s'affichent que sur demande.
  const visibles = $derived(analyse.lignes.filter((l) => detail || !['identique', 'ignoree'].includes(l.statut)));
  const pluriel = (n, mot) => `${n} ${mot}${n > 1 ? 's' : ''}`;

  async function importer() {
    enCours = true;
    erreur = null;
    try {
      const r = await api.importerPositions(chemin, supprimerAbsentes);
      fini(r);
    } catch (e) {
      erreur = String(e);
    } finally {
      enCours = false;
    }
  }
</script>

<section class="carte import-pea">
  <div class="livret-tete">
    <h3>Importer {analyse.fichier}</h3>
    <button class="discret" onclick={fermer} aria-label="Fermer">✕</button>
  </div>
  <p>
    Export {analyse.format === 'tableau' ? 'de portefeuille' : analyse.format} reconnu :
    <b>{pluriel(c.nouvelles, 'nouvelle ligne')}</b>, <b>{pluriel(c.mises_a_jour, 'mise à jour')}</b>{#if c.identiques}, {pluriel(c.identiques, 'ligne inchangée')}{/if}{#if c.erreurs}, <span class="negatif">{pluriel(c.erreurs, 'erreur')}</span>{/if}.
  </p>

  {#if visibles.length}
    <div class="defilement">
      <table class="apercu-import">
        <thead><tr><th>Titre</th><th>Statut</th><th class="nombre">Quantité</th><th class="nombre">PRU</th><th>Détail</th></tr></thead>
        <tbody>
          {#each visibles as l (l.numero)}
            <tr>
              <td class="titre-import">
                {l.nom ?? l.isin ?? l.symbole ?? `Ligne ${l.numero}`}
                {#if l.isin || l.symbole}<div class="discret petit">{[l.symbole, l.isin].filter(Boolean).join(' · ')}</div>{/if}
              </td>
              <td><span class="badge {STATUTS[l.statut].classe}">{STATUTS[l.statut].texte}</span></td>
              <td class="nombre">{l.quantite_micro === null ? '' : microVersTexte(l.quantite_micro)}</td>
              <td class="nombre">{l.pru_micro === null ? '' : `${microVersTexte(l.pru_micro)} €`}</td>
              <td class="messages-import">{#each l.messages as m}<div class="petit" class:negatif={l.statut === 'erreur'}>{m}</div>{/each}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
  {#if c.identiques + c.ignorees > 0}
    <p><button class="discret" onclick={() => (detail = !detail)}>{detail ? 'Masquer' : 'Afficher'} les lignes inchangées ou ignorées</button></p>
  {/if}

  {#if analyse.absentes.length}
    <div class="champs absentes-import">
      <label class="case">
        <input type="checkbox" bind:checked={supprimerAbsentes} />
        <span>Supprimer aussi {analyse.absentes.length > 1 ? `les ${analyse.absentes.length} lignes` : 'la ligne'} du PEA absente{analyse.absentes.length > 1 ? 's' : ''} de ce fichier : {analyse.absentes.join(', ')}</span>
      </label>
    </div>
  {/if}

  {#if erreur}<p class="message alerte">{erreur}</p>{/if}
  <div class="actions">
    <button class="principal" onclick={importer} disabled={enCours || aEcrire === 0}>
      {enCours ? 'Import…' : aEcrire === 0 ? 'PEA déjà à jour' : 'Importer'}
    </button>
    <button onclick={fermer}>Annuler</button>
  </div>
  <p class="discret petit">Une copie de sécurité de vos données est faite juste avant l'import : en cas de besoin, elle se restaure dans Paramètres.</p>
</section>
