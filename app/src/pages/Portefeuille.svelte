<script>
  import { onMount, onDestroy } from 'svelte';
  import { api, PAYS, symboleDevise } from '../lib/api.js';
  import { exporterCsv, eurosCsv, microCsv } from '../lib/export.js';
  import { euros, pct, microVersTexte, versMicro, aujourdHui } from '../lib/format.js';
  import Donut from '../lib/Donut.svelte';
  import Graphique from '../lib/Graphique.svelte';

  /** enveloppe : 'PEA' ou 'CTO' */
  let { enveloppe } = $props();
  const estPea = $derived(enveloppe === 'PEA');

  const PALETTE = ['#10B981', '#38BDF8', '#FBBF24', '#0284C7', '#34D399', '#F97316', '#8B5CF6'];

  let pf = $state(null);
  let erreurs = $state([]);
  let chargement = $state(false);
  let saisie = $state({ quantite: '', pru: '' });
  let recherche = $state('');
  let resultats = $state([]);
  let choisi = $state(null); // titre sélectionné dans la recherche
  let enCours = $state(false);
  let detail = $state(null); // symbole de la ligne détaillée (fiche + graphique)
  let apercu = $state(null); // { symbole, nom } : titre recherché, pas encore détenu
  let evenements = $state([]);
  let nouvelEvt = $state({ date: '', note: '' });
  let delai;

  const ligneDetail = $derived(pf?.lignes.find((l) => l.symbole === detail) ?? null);

  // Recherche au fil de la frappe (après 300 ms sans frappe).
  function chercher() {
    clearTimeout(delai);
    choisi = null;
    const q = recherche.trim();
    if (q.length < 2) return (resultats = []);
    delai = setTimeout(async () => {
      enCours = true;
      try {
        resultats = await api.chercherTitres(q, enveloppe);
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
    detail = null;
    apercu = { symbole: t.symbole, nom: t.nom, devise: t.devise };
  }
  let edition = $state(null); // { id, quantite, pru, pays }
  let coursManuel = $state(null); // { symbole, valeur }
  let minuterie;

  const fmt = (micro, max = 4) => (micro ?? null) === null ? '—' : (micro / 1e6).toLocaleString('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: max });
  const heure = (s) => (s ? new Date(s * 1000).toLocaleString('fr-FR', { dateStyle: 'short', timeStyle: 'short' }) : '');
  const jourFr = (iso) => (iso ? new Date(iso + 'T00:00:00').toLocaleDateString('fr-FR', { day: 'numeric', month: 'long', year: 'numeric' }) : '—');
  const signe = (v) => (v > 0 ? '+' : '');
  const FREQUENCE = { 1: 'annuel', 2: 'semestriel', 3: 'en 3 fois', 4: 'trimestriel', 12: 'mensuel' };

  let parts = $derived(
    (pf?.lignes ?? []).map((l, i) => ({ cle: String(l.id), libelle: l.nom, montant_cents: l.valeur_cents, couleur: PALETTE[i % PALETTE.length] })),
  );

  async function actualiser() {
    chargement = true;
    try {
      const r = await api.actualiserCours(enveloppe);
      pf = r.portefeuille;
      erreurs = r.erreurs;
    } catch (e) {
      erreurs = [String(e)];
    } finally {
      chargement = false;
    }
  }

  async function charger() {
    pf = null;
    detail = null;
    apercu = null;
    erreurs = [];
    pf = await api.positions(enveloppe);
    evenements = await api.evenements();
    if (pf.lignes.length) actualiser();
  }

  // Recharge quand on passe du PEA au CTO.
  $effect(() => {
    enveloppe;
    charger();
  });

  onMount(() => {
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
      pf = await api.ajouterPosition(enveloppe, { identifiant: choisi.isin ?? choisi.symbole, symbole: choisi.symbole, nom: choisi.nom, quantite_micro, pru_micro });
      saisie = { quantite: '', pru: '' };
      recherche = '';
      detail = choisi.symbole;
      apercu = null;
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
    try {
      pf = await api.modifierPosition(enveloppe, edition.id, q, p, edition.pays || null);
      edition = null;
    } catch (e) {
      erreurs = [String(e)];
    }
  }

  // Suppression en deux clics (les boîtes de dialogue du navigateur ne sont pas fiables dans Tauri).
  let aSupprimer = $state(null);
  async function supprimer(l) {
    if (aSupprimer !== l.id) return (aSupprimer = l.id);
    pf = await api.supprimerPosition(enveloppe, l.id);
    aSupprimer = null;
    if (detail === l.symbole) detail = null;
  }

  async function enregistrerCoursManuel() {
    const c = versMicro(coursManuel.valeur);
    if (!c) return (erreurs = ['Cours invalide.']);
    pf = await api.saisirCours(enveloppe, coursManuel.symbole, c);
    coursManuel = null;
  }

  async function ajouterAg(ev) {
    ev.preventDefault();
    if (!nouvelEvt.date) return (erreurs = ["Indiquez la date de l'assemblée générale."]);
    try {
      evenements = await api.ajouterEvenement({ symbole: detail, type: 'AG', date: nouvelEvt.date, note: nouvelEvt.note || null });
      nouvelEvt = { date: '', note: '' };
    } catch (e) {
      erreurs = [String(e)];
    }
  }

  async function supprimerEvt(id) {
    evenements = await api.supprimerEvenement(id);
  }

  // PRU converti dans la devise de cotation, pour la ligne « Mon PRU » du graphique (au taux du jour).
  const pruDevise = (l) => (l.devise === 'EUR' ? l.pru_micro : l.taux_change_micro ? Math.round((l.pru_micro * l.taux_change_micro) / 1e6) : null);

  let messageExport = $state('');
  async function exporter() {
    messageExport = '';
    const entetes = ['Titre', 'Symbole', 'ISIN', 'Pays', 'Devise', 'Quantité', 'PRU (€)', 'Cours (devise)', 'Cours (€)', 'Valeur (€)', 'Investi (€)',
      'Plus-value (€)', 'Plus-value (%)', ...(estPea ? [] : ['Impôt sur la plus-value (€)', 'Plus-value nette (€)']),
      'Variation 1 an (%)', 'Dividende annuel / action (devise)', 'Rendement (%)', 'Dividende brut annuel (€)', 'Retenue à la source (%)',
      'Dividende net annuel (€)', 'Prochain détachement estimé'];
    const lignes = pf.lignes.map((l) => [
      l.nom, l.symbole, l.isin ?? '', l.pays ?? '', l.devise, microCsv(l.quantite_micro), microCsv(l.pru_micro), microCsv(l.cours_micro),
      microCsv(l.cours_eur_micro), eurosCsv(l.valeur_cents), eurosCsv(l.investi_cents), eurosCsv(l.pv_cents), microCsv(l.pv_micro * 100, 2),
      ...(estPea ? [] : [eurosCsv(l.impot_pv_cents), eurosCsv(l.pv_nette_cents)]),
      microCsv(l.variation_un_an_micro === null ? null : l.variation_un_an_micro * 100, 2), microCsv(l.div_par_action_micro),
      microCsv(l.div_rendement_micro === null ? null : l.div_rendement_micro * 100, 2), eurosCsv(l.div_brut_cents),
      microCsv(l.retenue_micro * 100, 3), eurosCsv(l.div_net_cents), l.div_prochain_estime ?? '',
    ]);
    lignes.push(['Total', '', '', '', 'EUR', '', '', '', '', eurosCsv(pf.valeur_cents), eurosCsv(pf.investi_cents), eurosCsv(pf.pv_cents),
      microCsv(pf.pv_micro * 100, 2), ...(estPea ? [] : [eurosCsv(pf.impot_pv_cents), eurosCsv(pf.pv_nette_cents)]), '', '',
      microCsv(pf.div_rendement_micro * 100, 2), eurosCsv(pf.div_brut_cents), '', eurosCsv(pf.div_net_cents), '']);
    try {
      const chemin = await exporterCsv(`escarcelle-${enveloppe.toLowerCase()}`, entetes, lignes);
      if (chemin) messageExport = `Export enregistré : ${chemin}`;
    } catch (e) {
      erreurs = [String(e)];
    }
  }

  const evtsLigne = $derived(evenements.filter((e) => e.symbole === detail && e.date >= aujourdHui()));
</script>

<header class="entete">
  <h1>{estPea ? 'PEA' : 'Compte-titres (CTO)'}</h1>
  <div class="boutons">
    <button onclick={exporter} disabled={!pf?.lignes.length}>Exporter en CSV</button>
    <button onclick={actualiser} disabled={chargement}>{chargement ? 'Actualisation…' : 'Actualiser les cours'}</button>
  </div>
</header>
{#if messageExport}<p class="discret petit">{messageExport}</p>{/if}

{#if erreurs.length}
  <section class="alertes">{#each erreurs as e}<p>{e}</p>{/each}</section>
{/if}

{#if pf}
  <div class="kpis">
    <div class="kpi"><span>Valeur</span><strong>{euros(pf.valeur_cents)}</strong></div>
    <div class="kpi"><span>Investi (quantité × PRU)</span><strong>{euros(pf.investi_cents)}</strong></div>
    <div class="kpi">
      <span>Plus ou moins-value</span>
      <strong class:positif={pf.pv_cents > 0} class:negatif={pf.pv_cents < 0}>{signe(pf.pv_cents)}{euros(pf.pv_cents)}</strong>
      <small class:positif={pf.pv_micro > 0} class:negatif={pf.pv_micro < 0}>{signe(pf.pv_micro)}{pct(pf.pv_micro)}</small>
    </div>
    {#if estPea}
      <div class="kpi"><span>Impôt</span><strong>0 €</strong><small class="discret">tant que l'argent reste dans le PEA</small></div>
    {:else}
      <div class="kpi">
        <span>Après flat tax (31,4 %)</span>
        <strong class:positif={pf.pv_nette_cents > 0} class:negatif={pf.pv_nette_cents < 0}>{signe(pf.pv_nette_cents)}{euros(pf.pv_nette_cents)}</strong>
        <small class="discret">si tout était vendu aujourd'hui</small>
      </div>
    {/if}
  </div>

  {#if pf.lignes.length}
    <div class="kpis">
      <div class="kpi"><span>Dividendes annuels bruts</span><strong>{euros(pf.div_brut_cents)}</strong></div>
      <div class="kpi">
        <span>Dividendes annuels nets</span><strong class="positif">{euros(pf.div_net_cents)}</strong>
        <small class="discret">{estPea ? 'après retenues étrangères' : 'après retenues, flat tax et crédit d\'impôt'}</small>
      </div>
      <div class="kpi"><span>Rendement des dividendes</span><strong>{pct(pf.div_rendement_micro)}</strong><small class="discret">brut, sur la valeur actuelle</small></div>
      <div class="kpi">
        <span>Croissance du dividende</span>
        <strong>{pf.div_croissance_micro === null ? '—' : `${signe(pf.div_croissance_micro)}${pct(pf.div_croissance_micro, 1)} / an`}</strong>
        <small class="discret">moyenne sur les dernières années</small>
      </div>
    </div>
  {/if}

  <section class="carte">
    <h3>Mes lignes</h3>
    {#if pf.lignes.length === 0}
      <p class="vide">Ajoutez votre première ligne ci-dessous.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>Titre</th><th class="nombre">Quantité</th><th class="nombre">PRU (€)</th><th class="nombre">Cours</th>
            <th class="nombre">Valeur</th><th class="nombre">+/− value</th><th class="nombre">1 an</th><th class="nombre">Rend.</th><th></th>
          </tr>
        </thead>
        <tbody>
          {#each pf.lignes as l (l.id)}
            <tr class:selectionnee={detail === l.symbole}>
              <td>
                <button class="lien" onclick={() => { detail = l.symbole; apercu = null; }}>{l.nom}</button>
                <div class="discret petit">{l.symbole}{l.pays ? ` · ${PAYS[l.pays] ?? l.pays}` : ''}</div>
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
                  <input class="court" bind:value={coursManuel.valeur} inputmode="decimal" placeholder="Cours" />
                  <button class="discret" onclick={enregistrerCoursManuel}>OK</button>
                  <button class="discret" onclick={() => (coursManuel = null)}>✕</button>
                {:else if l.cours_micro !== null}
                  <span title="{l.cours_source} · {heure(l.cours_horodatage)}">{fmt(l.cours_micro)}{l.devise !== 'EUR' ? ` ${symboleDevise(l.devise)}` : ''}</span>
                  {#if l.devise !== 'EUR'}
                    <div class="discret petit">{l.cours_eur_micro !== null ? `≈ ${fmt(l.cours_eur_micro, 2)} €` : 'taux de change manquant'}</div>
                  {/if}
                  <div class="discret petit">{heure(l.cours_horodatage)}</div>
                {:else}
                  <button class="discret" onclick={() => (coursManuel = { symbole: l.symbole, valeur: '' })}>saisir</button>
                {/if}
              </td>
              <td class="nombre">{euros(l.valeur_cents)}</td>
              <td class="nombre" class:positif={l.pv_cents > 0} class:negatif={l.pv_cents < 0}>
                {signe(l.pv_cents)}{euros(l.pv_cents)}
                <div class="petit">{signe(l.pv_micro)}{pct(l.pv_micro)}</div>
              </td>
              <td class="nombre" class:positif={l.variation_un_an_micro > 0} class:negatif={l.variation_un_an_micro < 0}>
                {l.variation_un_an_micro === null ? '—' : `${signe(l.variation_un_an_micro)}${pct(l.variation_un_an_micro, 0)}`}
              </td>
              <td class="nombre">{l.div_rendement_micro ? pct(l.div_rendement_micro) : '—'}</td>
              <td class="actions">
                {#if edition?.id === l.id}
                  <select bind:value={edition.pays} title="Pays de la société (retenue à la source)">
                    <option value="">Pays : automatique</option>
                    {#each Object.entries(PAYS) as [code, nom]}<option value={code}>{nom}</option>{/each}
                  </select>
                  <button class="discret" onclick={enregistrerEdition}>Enregistrer</button>
                  <button class="discret" onclick={() => (edition = null)}>Annuler</button>
                {:else}
                  <button class="discret" onclick={() => (edition = { id: l.id, quantite: microVersTexte(l.quantite_micro), pru: microVersTexte(l.pru_micro), pays: l.pays ?? '' })}>Modifier</button>
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
                  <span><b>{t.nom}</b> <span class="discret">{t.symbole} · {t.place} · {t.type}{t.devise !== 'EUR' ? ` · ${t.devise}` : ''}</span></span>
                  {#if estPea}
                    <span class="badge" class:positif={t.eligibilite_pea === 'probable'} class:negatif={t.eligibilite_pea.startsWith('non')}>PEA : {t.eligibilite_pea}</span>
                  {/if}
                </button>
              </li>
            {/each}
          </ul>
        {:else if recherche.trim().length >= 2 && !enCours && !choisi}
          <p class="discret petit">{estPea ? 'Aucun titre coté en euros trouvé.' : 'Aucun titre trouvé.'}</p>
        {/if}
      </div>
      {#if choisi}
        <div class="ligne-formulaire">
          <label>Quantité<input bind:value={saisie.quantite} inputmode="decimal" placeholder="10" /></label>
          <label>PRU en euros<input bind:value={saisie.pru} inputmode="decimal" placeholder="Prix de revient affiché par la banque" /></label>
          <button type="submit" class="principal" disabled={chargement}>Ajouter {choisi.nom}</button>
        </div>
        {#if choisi.devise !== 'EUR'}
          <p class="discret petit">
            Titre coté en {choisi.devise} : saisissez le PRU <b>en euros</b>, change et frais compris, tel que votre banque l'affiche.
            Le cours est converti en euros au taux de change du jour ; la plus-value obtenue est celle qui sera imposée en France.
          </p>
        {/if}
      {/if}
    </form>
    <p class="discret petit">
      {#if estPea}
        Actions et ETF cotés en euros sur les places européennes. L'éligibilité au PEA dépend du pays de la société
        (Union européenne ou EEE) et, pour un ETF, de sa composition : elle est indiquée à titre indicatif, vérifiez-la auprès de votre banque.
      {:else}
        Actions et ETF cotés en Europe, aux États-Unis, en Suisse, à Londres, dans les pays nordiques, à Toronto, Tokyo, Hong Kong et Sydney.
        Le PRU est saisi en euros, frais de courtage et de change compris. Les cours en devises sont convertis au taux du jour (Yahoo Finance).
      {/if}
      Cours Yahoo Finance, en général différés d'environ 15 minutes. Cliquez sur une ligne pour voir sa fiche et sa courbe.
    </p>
  </section>

  {#if ligneDetail}
    {@const l = ligneDetail}
    <section class="carte fiche">
      <h3>{l.nom} — dividendes et événements</h3>
      {#if l.div_par_action_micro > 0}
        <div class="fiche-grille">
          <div><span>Dividende annuel / action</span><b>{fmt(l.div_par_action_micro, 2)} {symboleDevise(l.devise)}</b><small>{FREQUENCE[l.div_frequence] ?? `${l.div_frequence} versements`}</small></div>
          <div><span>Rendement</span><b>{l.div_rendement_micro ? pct(l.div_rendement_micro) : '—'}</b></div>
          <div><span>Croissance</span><b>{l.div_croissance_micro === null ? '—' : `${signe(l.div_croissance_micro)}${pct(l.div_croissance_micro, 1)} / an`}</b></div>
          <div><span>Prochain détachement</span><b>{jourFr(l.div_prochain_estime)}</b><small>estimé d'après l'an dernier</small></div>
        </div>
        <table class="decomposition">
          <tbody>
            <tr><td>Dividende brut annuel ({microVersTexte(l.quantite_micro)} actions)</td><td class="nombre">{euros(l.div_brut_cents)}</td></tr>
            <tr>
              <td>
                Retenue à la source {l.pays ? `(${PAYS[l.pays] ?? l.pays}, ${pct(l.retenue_micro, 2)})` : ''}
                {#if !l.retenue_connue}<span class="negatif petit"> — pays inconnu : indiquez-le avec « Modifier »</span>{/if}
                {#if estPea && l.retenue_micro > 0}<div class="discret petit">Perdue dans un PEA : aucun impôt français sur lequel l'imputer.</div>{/if}
              </td>
              <td class="nombre negatif">−{euros(l.div_retenue_cents)}</td>
            </tr>
            {#if !estPea}
              <tr><td>Impôt sur le revenu 12,8 % <span class="discret petit">(après crédit d'impôt de {euros(l.div_credit_cents)})</span></td><td class="nombre negatif">−{euros(l.div_impot_cents)}</td></tr>
              <tr><td>Prélèvements sociaux 18,6 %</td><td class="nombre negatif">−{euros(l.div_ps_cents)}</td></tr>
            {/if}
            <tr class="total"><td>Dividende net encaissé par an</td><td class="nombre positif">{euros(l.div_net_cents)}</td></tr>
          </tbody>
        </table>
      {:else}
        <p class="discret">Aucun dividende détaché ces 12 derniers mois (ou historique pas encore récupéré : cliquez sur « Actualiser les cours »).</p>
      {/if}

      <h4>Assemblées générales</h4>
      {#each evtsLigne as e (e.id)}
        <p class="evenement">
          <b>{jourFr(e.date)}</b> {e.note ?? ''}
          <button class="discret" onclick={() => supprimerEvt(e.id)} aria-label="Supprimer">✕</button>
        </p>
      {:else}
        <p class="discret petit">Aucune AG à venir enregistrée.</p>
      {/each}
      <form class="ligne-formulaire" onsubmit={ajouterAg}>
        <label>Date de l'AG<input type="date" bind:value={nouvelEvt.date} /></label>
        <label>Note (lieu, vote…)<input bind:value={nouvelEvt.note} placeholder="Facultatif" /></label>
        <button type="submit">Ajouter l'AG</button>
      </form>
      <p class="discret petit">
        Les sociétés françaises publient la date de leur AG au BALO (Bulletin des annonces légales obligatoires, sur
        journal-officiel.gouv.fr) et dans l'espace « Actionnaires » de leur site. Une alerte apparaît sur le tableau de bord 45 jours avant.
      </p>
    </section>
    <Graphique symbole={l.symbole} nom={l.nom} pru={pruDevise(l)} devise={l.devise} />
  {:else if apercu}
    <Graphique symbole={apercu.symbole} nom={apercu.nom} devise={apercu.devise} />
  {/if}

  {#if parts.length > 1}
    <Donut titre="Répartition des lignes" {parts} />
  {/if}
{/if}
