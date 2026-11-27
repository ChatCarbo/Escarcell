<script>
  import { exporterCsv, eurosCsv } from '../lib/export.js';
  import { onMount } from 'svelte';
  import { api } from '../lib/api.js';
  import { aujourdHui, euros, pct, versCents } from '../lib/format.js';
  import QuinzaineSchema from '../lib/QuinzaineSchema.svelte';

  let livrets = $state([]);
  let erreur = $state('');
  let op = $state({ livret: 'LIVRET_A', sens: 'VERSEMENT', date: aujourdHui(), montant: '' });
  let conseil = $state(null);
  let repartition = $state(null);
  let aSupprimer = $state(null);

  const dateFr = (s) => {
    const d = new Date(s + 'T00:00:00');
    const mois = d.toLocaleDateString('fr-FR', { month: 'long' });
    return `${d.getDate() === 1 ? '1er' : d.getDate()} ${mois}`;
  };
  const detenus = $derived(livrets.filter((l) => l.compte_id !== null));
  const total = $derived(detenus.reduce((s, l) => s + l.solde_cents, 0));
  const interets = $derived(detenus.reduce((s, l) => s + l.interets_estimes_cents, 0));

  async function charger() {
    try {
      livrets = await api.livrets();
      if (!detenus.find((l) => l.type === op.livret) && detenus.length) op.livret = detenus[0].type;
    } catch (e) {
      erreur = String(e);
    }
  }
  onMount(charger);

  // Conseil recalculé à chaque changement de livret, de sens, de date ou de montant ;
  // seule la réponse à la dernière demande est affichée.
  let numeroConseil = 0;
  $effect(() => {
    const { livret, sens, date, montant } = op;
    const numero = ++numeroConseil;
    if (!date || sens === 'VALORISATION') {
      conseil = null;
      return;
    }
    api
      .conseilLivret(livret, sens, date, versCents(montant) ?? 0)
      .then((c) => { if (numero === numeroConseil) conseil = c; })
      .catch(() => { if (numero === numeroConseil) conseil = null; });
  });

  async function ouvrir(type) {
    try {
      livrets = await api.ouvrirLivret(type);
      op.livret = type;
    } catch (e) {
      erreur = String(e);
    }
  }

  async function enregistrer(ev) {
    ev.preventDefault();
    erreur = '';
    const l = livrets.find((x) => x.type === op.livret);
    const m = versCents(op.montant);
    if (!l?.compte_id || !m || m <= 0) return (erreur = 'Choisissez un livret et un montant.');
    // Plafond et solde sont contrôlés par l'application à la date de valeur du virement (et non sur le solde
    // d'aujourd'hui, qui comprend les intérêts capitalisés depuis).
    try {
      await api.ajouterOperation({
        id: 0, compte_id: l.compte_id, date: op.date, type: op.sens, montant_cents: m,
        isin: null, quantite_micro: null, cours_micro: null, frais_cents: 0, valeur_plan_cents: null, libelle: null,
      });
    } catch (e) {
      return (erreur = String(e));
    }
    op.montant = '';
    await charger();
  }

  async function supprimer(id) {
    if (aSupprimer !== id) return (aSupprimer = id);
    try {
      await api.supprimerOperation(id);
    } catch (e) {
      erreur = String(e);
    }
    aSupprimer = null;
    await charger();
  }

  async function repartir() {
    const m = versCents(op.montant);
    if (op.sens === 'VALORISATION') return (repartition = null);
    try {
      repartition = m && m > 0 ? await api.repartirLivrets(op.sens, m) : null;
    } catch (e) {
      erreur = String(e);
    }
  }

  let messageExport = $state('');
  // Livrets déjà au plafond (aujourd'hui), pour expliquer la répartition d'un versement.
  const pleins = $derived(op.sens === 'VERSEMENT' ? detenus.filter((l) => l.solde_cents >= l.plafond_cents).map((l) => l.nom) : []);
  const choisiPlein = $derived.by(() => {
    const l = detenus.find((x) => x.type === op.livret);
    return op.sens === 'VERSEMENT' && l && l.solde_cents >= l.plafond_cents && op.date >= aujourdHui() ? l.nom : null;
  });

  const LIBELLES = { VERSEMENT: 'Versement', RETRAIT: 'Retrait', VALORISATION: 'Solde de départ' };
  // Le solde de départ n'est proposé que pour un livret qui n'en a pas encore.
  const aUnDepart = $derived(livrets.find((x) => x.type === op.livret)?.operations.some((o) => o.type === 'VALORISATION') ?? false);
  $effect(() => {
    if (aUnDepart && op.sens === 'VALORISATION') op.sens = 'VERSEMENT';
  });

  async function exporter() {
    messageExport = '';
    const lignes = detenus.flatMap((l) =>
      l.operations.map((o) => [l.nom, o.date, LIBELLES[o.type], eurosCsv(o.type === 'RETRAIT' ? -o.montant_cents : o.montant_cents)]),
    );
    try {
      const chemin = await exporterCsv('escarcell-livrets', ['Livret', 'Date de valeur', 'Opération', 'Montant (€)'], lignes);
      if (chemin) messageExport = `Export enregistré : ${chemin}`;
    } catch (e) {
      erreur = String(e);
    }
  }
</script>

<header class="entete">
  <h1>Livrets réglementés</h1>
  <button onclick={exporter} disabled={!detenus.some((l) => l.operations.length)}>Exporter l'historique en CSV</button>
</header>
{#if messageExport}<p class="discret petit">{messageExport}</p>{/if}
{#if erreur}<p class="erreur">{erreur}</p>{/if}

<div class="kpis">
  <div class="kpi"><span>Total sur les livrets</span><strong>{euros(total)}</strong></div>
  <div class="kpi"><span>Intérêts estimés cette année</span><strong class="positif">{euros(interets)}</strong></div>
</div>

<div class="grille-3">
  {#each livrets as l (l.type)}
    <section class="carte livret">
      <div class="livret-tete">
        <h3>{l.nom}</h3>
        <span class="discret">{pct(l.taux_micro)} net</span>
      </div>
      {#if l.compte_id === null}
        <p class="vide">Vous n'avez pas ce livret.</p>
        <span class="discret petit">Au plafond toute l'année {new Date().getFullYear()} : {euros(l.interets_max_cents)} d'intérêts</span>
        <button onclick={() => ouvrir(l.type)}>J'ai un {l.nom}</button>
      {:else}
        <strong class="solde">{euros(l.solde_cents)}</strong>
        <div class="jauge"><div style="width:{Math.min(100, (l.solde_cents / l.plafond_cents) * 100)}%"></div></div>
        <span class="discret petit">{euros(l.solde_cents)} / {euros(l.plafond_cents)} · intérêts {new Date().getFullYear()} : {euros(l.interets_estimes_cents)}</span>
        <span class="discret petit" title="Livret resté au plafond du 1er janvier au 31 décembre, aux taux de l'année">Maximum possible {new Date().getFullYear()} : {euros(l.interets_max_cents)}</span>
        {#if l.annees.length}
          <span class="petit">Intérêts cumulés depuis {l.annees[0].annee} : <b class="positif">{euros(l.interets_cumules_cents)}</b></span>
          <details class="petit">
            <summary>Détail par année</summary>
            <div class="defilement"><table>
              <thead><tr><th>Année</th><th class="nombre">Taux moy.</th><th class="nombre">Intérêts</th><th class="nombre">Solde 31/12</th></tr></thead>
              <tbody>
                {#each l.annees as a (a.annee)}
                  <tr>
                    <td>{a.annee}{a.en_cours ? ' (est.)' : ''}</td>
                    <td class="nombre">{pct(a.taux_moyen_micro)}</td>
                    <td class="nombre">{euros(a.interets_cents)}</td>
                    <td class="nombre">{euros(a.solde_fin_cents)}</td>
                  </tr>
                {/each}
              </tbody>
            </table></div>
            <p class="discret">Chaque quinzaine est payée au taux en vigueur à son premier jour ; l'année en cours suppose qu'aucun autre virement n'aura lieu.</p>
          </details>
        {/if}
      {/if}
    </section>
  {/each}
</div>

{#if detenus.length}
  <div class="grille-2">
    <form class="carte" onsubmit={enregistrer}>
      <h3>Nouveau virement</h3>
      <div class="segments">
        <button type="button" class:actif={op.sens === 'VERSEMENT'} onclick={() => (op.sens = 'VERSEMENT')}>Versement</button>
        <button type="button" class:actif={op.sens === 'RETRAIT'} onclick={() => (op.sens = 'RETRAIT')}>Retrait</button>
        {#if !aUnDepart}
          <button type="button" class:actif={op.sens === 'VALORISATION'} onclick={() => { op.sens = 'VALORISATION'; repartition = null; }}>Solde de départ</button>
        {/if}
      </div>
      <label>Livret
        <select bind:value={op.livret}>{#each detenus as l}<option value={l.type}>{l.nom}</option>{/each}</select>
      </label>
      <label>Date de valeur<input type="date" min="2020-02-01" bind:value={op.date} /></label>
      <label>{op.sens === 'VALORISATION' ? 'Solde lu sur le relevé, intérêts compris (€)' : 'Montant (€)'}<input bind:value={op.montant} inputmode="decimal" onblur={repartir} /></label>
      <button type="submit" class="principal">Enregistrer</button>
      {#if repartition && (repartition.parts.length || repartition.reste_cents > 0)}
        <p class="discret petit">
          {#if op.sens === 'VERSEMENT'}
            Conseil (soldes d'aujourd'hui) : verser
            {repartition.parts.map(([n, m]) => `${euros(m)} sur le ${n}`).join(', ')}
            {#if pleins.length}— {pleins.join(' et ')} {pleins.length > 1 ? 'sont' : 'est'} au plafond{:else}— meilleur taux d'abord{/if}.
            {#if repartition.reste_cents > 0}{euros(repartition.reste_cents)} ne peuvent être placés sur aucun livret (plafonds atteints).{/if}
          {:else}
            Conseil (soldes d'aujourd'hui) : retirer
            {repartition.parts.map(([n, m]) => `${euros(m)} sur le ${n}`).join(', ')}
            — taux le plus bas d'abord ; à taux égal, le livret au plus petit plafond.
            {#if repartition.reste_cents > 0}Il manque {euros(repartition.reste_cents)} : les soldes ne suffisent pas.{/if}
          {/if}
        </p>
        {#if choisiPlein}
          <p class="erreur petit">Le {choisiPlein} est déjà au plafond : ce versement sera refusé.</p>
        {/if}
      {/if}
    </form>

    <section class="carte">
      <h3>{op.sens === 'VERSEMENT' ? 'Quand verser ?' : op.sens === 'RETRAIT' ? 'Quand retirer ?' : 'Solde de départ'}</h3>
      {#if op.sens === 'VALORISATION'}
        <p class="message ok">
          Inutile de ressaisir des années de virements : indiquez le solde figurant sur un relevé, <b>intérêts compris</b>,
          à sa date. Le plus simple : le solde au 31 décembre de votre relevé annuel, daté du <b>1er janvier</b> suivant.
        </p>
        <p class="discret petit">
          Daté d'un 1er ou d'un 16, ce solde rapporte dès cette quinzaine ; sinon à partir de la suivante (un solde au
          31 décembre comprend déjà les intérêts de l'année). Il peut dépasser le plafond (grâce aux intérêts capitalisés). Un seul solde de départ par livret ; les virements saisis ensuite doivent être datés après lui.
          Date au plus tôt : 1er février 2020.
        </p>
      {:else if conseil}
        {#if op.sens === 'VERSEMENT'}
          <p class="message ok">
            Versé le <b>{dateFr(op.date)}</b>, l'argent rapporte à partir du <b>{dateFr(conseil.date_effet)}</b>.
            Vous pouvez attendre jusqu'au <b>{dateFr(conseil.date_conseillee)}</b> sans rien perdre :
            verser plus tôt dans la même quinzaine ne rapporte pas davantage.
          </p>
        {:else if conseil.jours > 0}
          <!-- Le choix du message dépend de la date seule (jours déjà passés dans la quinzaine), pas du gain :
               sans montant saisi, le gain vaut 0 et la date n'en est pas meilleure pour autant. -->
          <p class="message attention">
            Retiré le <b>{dateFr(op.date)}</b>, ce montant ne rapporte plus depuis le <b>{dateFr(conseil.date_effet)}</b> :
            les {conseil.jours} jours déjà passés dans cette quinzaine ne sont pas payés.
            Si vous pouvez attendre le <b>{dateFr(conseil.date_conseillee)}</b>,
            {#if conseil.gain_cents > 0}vous gagnez <b>{euros(conseil.gain_cents)}</b>.{:else}ce montant rapporte une quinzaine de plus.{/if}
            Si vous ne pouvez pas attendre, retirer aujourd'hui ou le {dateFr(conseil.fin_quinzaine)} revient au même.
          </p>
        {:else}
          <p class="message ok">
            Bonne date : le <b>{dateFr(op.date)}</b> est le premier jour d'une quinzaine, la précédente est entièrement payée.
          </p>
        {/if}
        <QuinzaineSchema sens={op.sens} date={op.date} {conseil} />
      {/if}
    </section>
  </div>

  <section class="carte regles">
    <h3>La règle des quinzaines en deux phrases</h3>
    <div class="grille-2">
      <p><span class="pastille-regle vert"></span><b>Versez avant le 15 ou avant la fin du mois.</b> L'argent rapporte à partir du 16 ou du 1er suivant, quel que soit le jour où vous l'avez versé dans la quinzaine.</p>
      <p><span class="pastille-regle rouge"></span><b>Retirez le 1er ou le 16.</b> Un retrait fait perdre toute la quinzaine en cours : retirer le 14 coûte autant que retirer le 1er, mais retirer le 16 fait gagner la quinzaine du 1er au 15.</p>
    </div>
    <p class="discret petit">Les dates sont des dates de valeur : un virement fait un vendredi 15 peut être daté du lundi 17 par la banque. Prévoyez un ou deux jours ouvrés de marge. Les intérêts sont versés au 31 décembre.</p>
  </section>

  <section class="carte">
    <h3>Historique</h3>
    {#each detenus as l (l.type)}
      {#if l.operations.length}
        <h4>{l.nom}</h4>
        <table>
          <tbody>
            {#each l.operations as o (o.id)}
              <tr>
                <td>{new Date(o.date + 'T00:00:00').toLocaleDateString('fr-FR')}</td>
                <td>{LIBELLES[o.type]}</td>
                <td class="nombre" class:positif={o.type === 'VERSEMENT'} class:negatif={o.type === 'RETRAIT'}>{o.type === 'VERSEMENT' ? '+' : o.type === 'RETRAIT' ? '−' : '='}{euros(o.montant_cents)}</td>
                <td class="actions"><button class="discret" class:negatif={aSupprimer === o.id} onclick={() => supprimer(o.id)}>{aSupprimer === o.id ? 'Confirmer ?' : '✕'}</button></td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {/each}
  </section>
{/if}
