<script>
  import { onMount, onDestroy } from 'svelte';
  import { api } from '../lib/api.js';
  import { euros, pct, microVersTexte, versMicro } from '../lib/format.js';
  import Donut from '../lib/Donut.svelte';
  import Graphique from '../lib/Graphique.svelte';

  const PALETTE = ['#10B981', '#38BDF8', '#FBBF24', '#0284C7', '#34D399', '#F97316'];

  let pf = $state(null);
  let erreurs = $state([]);
  let chargement = $state(false);
  let saisie = $state({ quantite: '', pru: '' });
  let recherche = $state('');
  let resultats = $state([]);
  let choisi = $state(null); // titre sélectionné dans la recherche
  let enCours = $state(false);
  let detail = $state(null); // { symbole, nom, pru } affiché dans le graphique
  let delai;

  // Recherche au fil de la frappe (après 300 ms sans frappe).
  function chercher() {
    clearTimeout(delai);
    choisi = null;
    const q = recherche.trim();
    if (q.length < 2) return (resultats = []);
    delai = setTimeout(async () => {
      enCours = true;
      try {
        resultats = await api.chercherTitres(q);
      } catch (e) {
        erreurs = [String(e)];
      } finally {
        enCours = false;
      }
    }, 300);
  }

  function choisir(t) {
    choisi = t;
    resultats = [];
    recherche = `${t.nom} (${t.symbole})`;
    detail = { symbole: t.symbole, nom: t.nom, pru: null };
  }
  let edition = $state(null); // { id, quantite, pru }
  let coursManuel = $state(null); // { symbole, valeur }
  let minuterie;

  const fmt = (micro, max = 4) => (micro ?? null) === null ? '—' : (micro / 1e6).toLocaleString('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: max });
  const heure = (s) => (s ? new Date(s * 1000).toLocaleString('fr-FR', { dateStyle: 'short', timeStyle: 'short' }) : '');

  let parts = $derived(
    (pf?.lignes ?? []).map((l, i) => ({ cle: String(l.id), libelle: l.nom, montant_cents: l.valeur_cents, couleur: PALETTE[i % PALETTE.length] })),
  );

  async function actualiser() {
    chargement = true;
    try {
      const r = await api.actualiserCours();
      pf = r.portefeuille;
      erreurs = r.erreurs;
    } catch (e) {
      erreurs = [String(e)];
    } finally {
      chargement = false;
    }
  }

  onMount(async () => {
    pf = await api.positions();
    if (pf.lignes.length) actualiser();
    minuterie = setInterval(actualiser, 5 * 60 * 1000); // toutes les 5 minutes
  });
  onDestroy(() => clearInterval(minuterie));

  async function ajouter(ev) {
    ev.preventDefault();
    erreurs = [];
    const quantite_micro = versMicro(saisie.quantite);
    const pru_micro = versMicro(saisie.pru);
    if (!choisi) return (erreurs = ['Cherchez puis sélectionnez un titre dans la liste.']);
    if (!quantite_micro || pru_micro === null) return (erreurs = ['Renseignez la quantité et le PRU.']);
    chargement = true;
    try {
      pf = await api.ajouterPosition({ identifiant: choisi.isin ?? choisi.symbole, symbole: choisi.symbole, nom: choisi.nom, quantite_micro, pru_micro });
      saisie = { quantite: '', pru: '' };
      recherche = '';
      choisi = null;
    } catch (e) {
      erreurs = [String(e)];
    } finally {
      chargement = false;
    }
  }

  async function enregistrerEdition() {
    const q = versMicro(edition.quantite);
    const p = versMicro(edition.pru);
    if (!q || p === null) return (erreurs = ['Quantité et PRU invalides.']);
    pf = await api.modifierPosition(edition.id, q, p);
    edition = null;
  }

  // Suppression en deux clics (les boîtes de dialogue du navigateur ne sont pas fiables dans Tauri).
  let aSupprimer = $state(null);
  async function supprimer(l) {
    if (aSupprimer !== l.id) return (aSupprimer = l.id);
    pf = await api.supprimerPosition(l.id);
    aSupprimer = null;
  }

  async function enregistrerCoursManuel() {
    const c = versMicro(coursManuel.valeur);
    if (!c) return (erreurs = ['Cours invalide.']);
    pf = await api.saisirCours(coursManuel.symbole, c);
    coursManuel = null;
  }
</script>

<header class="entete">
  <h1>PEA</h1>
  <button onclick={actualiser} disabled={chargement}>{chargement ? 'Actualisation…' : 'Actualiser les cours'}</button>
</header>

{#if erreurs.length}
  <section class="alertes">{#each erreurs as e}<p>{e}</p>{/each}</section>
{/if}

{#if pf}
  <div class="kpis">
    <div class="kpi"><span>Valeur</span><strong>{euros(pf.valeur_cents)}</strong></div>
    <div class="kpi"><span>Investi (quantité × PRU)</span><strong>{euros(pf.investi_cents)}</strong></div>
    <div class="kpi">
      <span>Plus ou moins-value</span>
      <strong class:positif={pf.pv_cents > 0} class:negatif={pf.pv_cents < 0}>{pf.pv_cents > 0 ? '+' : ''}{euros(pf.pv_cents)}</strong>
    </div>
    <div class="kpi">
      <span>Performance</span>
      <strong class:positif={pf.pv_micro > 0} class:negatif={pf.pv_micro < 0}>{pf.pv_micro > 0 ? '+' : ''}{pct(pf.pv_micro)}</strong>
    </div>
  </div>

  <section class="carte">
    <h3>Mes lignes</h3>
    {#if pf.lignes.length === 0}
      <p class="vide">Ajoutez votre première ligne ci-dessous.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Titre</th><th class="nombre">Quantité</th><th class="nombre">PRU</th><th class="nombre">Cours</th>
            <th class="nombre">Valeur</th><th class="nombre">+/− value</th><th class="nombre">%</th><th></th>
          </tr>
        </thead>
        <tbody>
          {#each pf.lignes as l (l.id)}
            <tr>
              <td>
                <button class="lien" onclick={() => (detail = { symbole: l.symbole, nom: l.nom, pru: l.pru_micro })}>{l.nom}</button>
                <div class="discret petit">{l.isin ?? ''} {l.symbole}</div>
              </td>
              {#if edition?.id === l.id}
                <td class="nombre"><input class="court" bind:value={edition.quantite} inputmode="decimal" /></td>
                <td class="nombre"><input class="court" bind:value={edition.pru} inputmode="decimal" /></td>
              {:else}
                <td class="nombre">{microVersTexte(l.quantite_micro)}</td>
                <td class="nombre">{fmt(l.pru_micro)}</td>
              {/if}
              <td class="nombre">
                {#if coursManuel?.symbole === l.symbole}
                  <input class="court" bind:value={coursManuel.valeur} inputmode="decimal" />
                  <button class="discret" onclick={enregistrerCoursManuel}>OK</button>
                {:else if l.cours_micro !== null}
                  <span title="{l.cours_source} · {heure(l.cours_horodatage)}">{fmt(l.cours_micro)}</span>
                  <div class="discret petit">{heure(l.cours_horodatage)}</div>
                {:else}
                  <button class="discret" onclick={() => (coursManuel = { symbole: l.symbole, valeur: '' })}>saisir</button>
                {/if}
              </td>
              <td class="nombre">{euros(l.valeur_cents)}</td>
              <td class="nombre" class:positif={l.pv_cents > 0} class:negatif={l.pv_cents < 0}>{l.pv_cents > 0 ? '+' : ''}{euros(l.pv_cents)}</td>
              <td class="nombre" class:positif={l.pv_micro > 0} class:negatif={l.pv_micro < 0}>{l.pv_micro > 0 ? '+' : ''}{pct(l.pv_micro)}</td>
              <td class="actions">
                {#if edition?.id === l.id}
                  <button class="discret" onclick={enregistrerEdition}>Enregistrer</button>
                  <button class="discret" onclick={() => (edition = null)}>Annuler</button>
                {:else}
                  <button class="discret" onclick={() => (edition = { id: l.id, quantite: microVersTexte(l.quantite_micro), pru: microVersTexte(l.pru_micro) })}>Modifier</button>
                  <button class="discret" onclick={() => (coursManuel = { symbole: l.symbole, valeur: '' })}>Cours</button>
                  <button class="discret" class:negatif={aSupprimer === l.id} onclick={() => supprimer(l)} aria-label="Supprimer">{aSupprimer === l.id ? 'Confirmer ?' : '✕'}</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    <form class="ajout" onsubmit={ajouter}>
      <h3>Ajouter une ligne</h3>
      <div class="recherche">
        <input bind:value={recherche} oninput={chercher} placeholder="Nom, mnémonique ou ISIN : LVMH, MC, FR0000121014, MSCI World…" />
        {#if enCours}<span class="discret petit">Recherche…</span>{/if}
        {#if resultats.length}
          <ul class="resultats-recherche">
            {#each resultats as t (t.symbole)}
              <li>
                <button type="button" onclick={() => choisir(t)}>
                  <span><b>{t.nom}</b> <span class="discret">{t.symbole} · {t.place} · {t.type}</span></span>
                  <span class="badge" class:positif={t.eligibilite_pea === 'probable'} class:negatif={t.eligibilite_pea.startsWith('non')}>PEA : {t.eligibilite_pea}</span>
                </button>
              </li>
            {/each}
          </ul>
        {:else if recherche.trim().length >= 2 && !enCours && !choisi}
          <p class="discret petit">Aucun titre coté en euros trouvé.</p>
        {/if}
      </div>
      {#if choisi}
        <div class="ligne-formulaire">
          <label>Quantité<input bind:value={saisie.quantite} inputmode="decimal" placeholder="10" /></label>
          <label>PRU (€)<input bind:value={saisie.pru} inputmode="decimal" placeholder="Prix de revient affiché par la banque" /></label>
          <button type="submit" class="principal" disabled={chargement}>Ajouter {choisi.nom}</button>
        </div>
      {/if}
    </form>
    <p class="discret petit">
      Actions et ETF cotés en euros sur les places européennes. L'éligibilité au PEA dépend du pays de la société
      (Union européenne ou EEE) et, pour un ETF, de sa composition : elle est indiquée à titre indicatif, vérifiez-la auprès de votre banque.
      Cours Yahoo Finance, en général différés d'environ 15 minutes. Cliquez sur une ligne pour voir sa courbe.
    </p>
  </section>

  {#if detail}
    <Graphique symbole={detail.symbole} nom={detail.nom} pru={detail.pru} />
  {/if}

  {#if parts.length > 1}
    <Donut titre="Répartition des lignes" {parts} />
  {/if}
{/if}
