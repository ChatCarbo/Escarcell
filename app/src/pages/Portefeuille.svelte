<script>
  import { onMount, onDestroy } from 'svelte';
  import { api, PAYS } from '../lib/api.js';
  import { exporterCsv, eurosCsv, microCsv } from '../lib/export.js';
  import { euros, pct, microVersTexte, versMicro, aujourdHui } from '../lib/format.js';
  import Donut from '../lib/Donut.svelte';
  import Graphique from '../lib/Graphique.svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import ImportPortefeuille from '../lib/ImportPortefeuille.svelte';
  import ImportOperations from '../lib/ImportOperations.svelte';

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
  let edition = $state(null); // { id, journal, quantiteMicro, pruMicro, quantite, pru, pays }
  let coursManuel = $state(null); // { symbole, valeur }
  let aSupprimer = $state(null);
  let messageExport = $state('');
  // Import : { chemin, genre: 'positions' | 'operations', positions } pendant l'aperçu.
  let importEnCours = $state(null);
  let lectureImport = $state(false);
  let delai;
  let minuterie;
  let numeroRecherche = 0; // ignore les réponses d'une recherche dépassée par une frappe plus récente

  const ligneDetail = $derived(pf?.lignes.find((l) => l.symbole === detail) ?? null);
  const evtsLigne = $derived(evenements.filter((e) => e.symbole === detail && e.date >= aujourdHui()));
  const parts = $derived(
    (pf?.lignes ?? []).map((l, i) => ({ cle: String(l.id), libelle: l.nom, montant_cents: l.valeur_cents, couleur: PALETTE[i % PALETTE.length] })),
  );

  const fmt = (micro, max = 4) => ((micro ?? null) === null ? '—' : (micro / 1e6).toLocaleString('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: max }));
  const heure = (s) => (s ? new Date(s * 1000).toLocaleString('fr-FR', { dateStyle: 'short', timeStyle: 'short' }) : '');
  const jourFr = (iso) => (iso ? new Date(iso + 'T00:00:00').toLocaleDateString('fr-FR', { day: 'numeric', month: 'long', year: 'numeric' }) : '—');
  const signe = (v) => (v > 0 ? '+' : '');
  // Période des dividendes comptés : les 12 derniers mois, jusqu'à aujourd'hui inclus.
  const periode12 = () => {
    const f = (d) => d.toLocaleDateString('fr-FR');
    const fin = new Date();
    const debut = new Date(fin);
    debut.setFullYear(fin.getFullYear() - 1);
    debut.setDate(debut.getDate() + 1);
    return `du ${f(debut)} au ${f(fin)}`;
  };
  const AIDE_DIVIDENDES =
    "Somme des dividendes détachés ces 12 derniers mois (dividendes réellement versés, historique Yahoo Finance). " +
    "Une hausse annoncée mais pas encore détachée n'est pas comptée : d'autres applications, qui projettent les prochains " +
    'versements, peuvent afficher un montant différent.';
  const FREQUENCE = { 1: 'annuel', 2: 'semestriel', 3: 'en 3 fois', 4: 'trimestriel', 12: 'mensuel' };

  /** Exécute une commande et affiche son erreur éventuelle au lieu de la laisser se perdre. */
  async function essayer(action) {
    try {
      return await action();
    } catch (e) {
      erreurs = [String(e)];
      return undefined;
    }
  }

  // Recherche au fil de la frappe (après 300 ms sans frappe).
  function chercher() {
    clearTimeout(delai);
    choisi = null;
    const numero = ++numeroRecherche;
    const q = recherche.trim();
    if (q.length < 2) {
      resultats = [];
      enCours = false;
      return;
    }
    delai = setTimeout(async () => {
      enCours = true;
      try {
        const r = await api.chercherTitres(q);
        if (numero === numeroRecherche) resultats = r;
      } catch (e) {
        if (numero === numeroRecherche) erreurs = [String(e)];
      } finally {
        if (numero === numeroRecherche) enCours = false;
      }
    }, 300);
  }

  function choisir(t) {
    numeroRecherche++; // une recherche encore en cours ne doit pas rouvrir la liste
    clearTimeout(delai);
    enCours = false;
    resultats = [];
    detail = null;
    choisi = t;
    recherche = `${t.nom} (${t.symbole})`;
    apercu = { symbole: t.symbole, nom: t.nom };
  }

  async function actualiser() {
    // Pas de requête si une actualisation est déjà en cours ou si le portefeuille est vide.
    if (chargement || !pf?.lignes.length) return;
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

  async function charger() {
    erreurs = [];
    try {
      pf = await api.positions();
      evenements = await api.evenements();
    } catch (e) {
      erreurs = [String(e)];
      return;
    }
    actualiser();
  }

  onMount(() => {
    charger();
    minuterie = setInterval(actualiser, 5 * 60 * 1000); // toutes les 5 minutes
  });
  onDestroy(() => {
    clearInterval(minuterie);
    clearTimeout(delai);
  });

  // Import : le fichier est reconnu tout seul (export Moning, BoursoBank, classeur Excel, historique d'opérations).
  async function choisirImport() {
    const chemin = await open({
      multiple: false,
      filters: [
        { name: 'Export de portefeuille ou d\'opérations', extensions: ['csv', 'xlsx', 'xls', 'ods', 'txt', 'tsv'] },
        { name: 'Tous les fichiers', extensions: ['*'] },
      ],
    });
    if (!chemin) return;
    erreurs = [];
    messageExport = '';
    lectureImport = true;
    const r = await essayer(() => api.analyserFichier(chemin));
    lectureImport = false;
    if (r) importEnCours = { chemin, genre: r.genre, positions: r.positions };
  }

  async function importTermine(texte) {
    importEnCours = null;
    messageExport = texte;
    await charger();
  }

  async function ajouter(ev) {
    ev.preventDefault();
    if (chargement) return;
    erreurs = [];
    const quantite_micro = versMicro(saisie.quantite);
    const pru_micro = versMicro(saisie.pru);
    if (!choisi) return (erreurs = ['Cherchez puis sélectionnez un titre dans la liste.']);
    if (!quantite_micro || quantite_micro <= 0 || pru_micro === null || pru_micro < 0) {
      return (erreurs = ['Renseignez une quantité positive et le PRU.']);
    }
    chargement = true;
    try {
      const t = choisi;
      pf = await api.ajouterPosition({ identifiant: t.isin ?? t.symbole, symbole: t.symbole, nom: t.nom, quantite_micro, pru_micro });
      saisie = { quantite: '', pru: '' };
      recherche = '';
      detail = t.symbole;
      apercu = null;
      choisi = null;
    } catch (e) {
      erreurs = [String(e)];
    } finally {
      chargement = false;
    }
  }

  async function enregistrerEdition() {
    // Ligne calculée par le journal : seul le pays se modifie.
    const q = edition.journal ? edition.quantiteMicro : versMicro(edition.quantite);
    const p = edition.journal ? edition.pruMicro : versMicro(edition.pru);
    if (!q || q <= 0 || p === null || p < 0) return (erreurs = ['Quantité et PRU invalides.']);
    const r = await essayer(() => api.modifierPosition(edition.id, q, p, edition.pays || null));
    if (r) {
      pf = r;
      edition = null;
    }
  }

  // Suppression en deux clics (les boîtes de dialogue du navigateur ne sont pas fiables dans Tauri).
  async function supprimer(l) {
    if (aSupprimer !== l.id) return (aSupprimer = l.id);
    aSupprimer = null;
    const r = await essayer(() => api.supprimerPosition(l.id));
    if (r) {
      pf = r;
      if (detail === l.symbole && !r.lignes.some((x) => x.symbole === l.symbole)) detail = null;
    }
  }

  async function enregistrerCoursManuel() {
    const c = versMicro(coursManuel.valeur);
    if (!c || c <= 0) return (erreurs = ['Cours invalide.']);
    const r = await essayer(() => api.saisirCours(coursManuel.symbole, c));
    if (r) {
      pf = r;
      coursManuel = null;
    }
  }

  async function ajouterAg(ev) {
    ev.preventDefault();
    if (!nouvelEvt.date) return (erreurs = ["Indiquez la date de l'assemblée générale."]);
    const r = await essayer(() => api.ajouterEvenement({ symbole: detail, type: 'AG', date: nouvelEvt.date, note: nouvelEvt.note || null }));
    if (r) {
      evenements = r;
      nouvelEvt = { date: '', note: '' };
    }
  }

  async function supprimerEvt(id) {
    const r = await essayer(() => api.supprimerEvenement(id));
    if (r) evenements = r;
  }

  async function exporter() {
    messageExport = '';
    if (!pf?.lignes.length) return;
    const entetes = ['Titre', 'Symbole', 'ISIN', 'Pays', 'Quantité', 'PRU (€)', 'Cours (€)', 'Valeur (€)', 'Investi (€)',
      'Plus-value (€)', 'Plus-value (%)', 'Variation 1 an (%)', 'Dividende / action, 12 derniers mois (€)', 'Rendement 12 mois (%)',
      'Dividende brut 12 derniers mois (€)', 'Retenue à la source (%)', 'Dividende net 12 derniers mois (€)', 'Prochain détachement estimé'];
    const lignes = pf.lignes.map((l) => [
      l.nom, l.symbole, l.isin ?? '', l.pays ?? '', microCsv(l.quantite_micro), microCsv(l.pru_micro), microCsv(l.cours_micro),
      eurosCsv(l.valeur_cents), eurosCsv(l.investi_cents), eurosCsv(l.pv_cents), microCsv(l.pv_micro * 100, 2),
      microCsv(l.variation_un_an_micro === null ? null : l.variation_un_an_micro * 100, 2), microCsv(l.div_par_action_micro),
      microCsv(l.div_rendement_micro === null ? null : l.div_rendement_micro * 100, 2), eurosCsv(l.div_brut_cents),
      microCsv(l.retenue_micro * 100, 3), eurosCsv(l.div_net_cents), l.div_prochain_estime ?? '',
    ]);
    lignes.push(['Total', '', '', '', '', '', '', eurosCsv(pf.valeur_cents), eurosCsv(pf.investi_cents), eurosCsv(pf.pv_cents),
      microCsv(pf.pv_micro * 100, 2), '', '', microCsv(pf.div_rendement_micro * 100, 2), eurosCsv(pf.div_brut_cents), '',
      eurosCsv(pf.div_net_cents), '']);
    const chemin = await essayer(() => exporterCsv('escarcelle-pea', entetes, lignes));
    if (chemin) messageExport = `Export enregistré : ${chemin}`;
  }
</script>

<header class="entete">
  <h1>PEA</h1>
  <div class="boutons">
    <button onclick={choisirImport} disabled={lectureImport}>{lectureImport ? 'Lecture du fichier…' : 'Importer…'}</button>
    <button onclick={exporter} disabled={!pf?.lignes.length}>Exporter en CSV</button>
    <button onclick={actualiser} disabled={chargement}>{chargement ? 'Actualisation…' : 'Actualiser les cours'}</button>
  </div>
</header>
{#if messageExport}<p class="discret petit">{messageExport}</p>{/if}

{#if importEnCours?.genre === 'positions'}
  <ImportPortefeuille
    chemin={importEnCours.chemin}
    analyse={importEnCours.positions}
    fermer={() => (importEnCours = null)}
    fini={(r) => importTermine(`Import terminé : ${r.positions.compteurs.nouvelles} ligne(s) ajoutée(s), ${r.positions.compteurs.mises_a_jour} mise(s) à jour${r.supprimees ? `, ${r.supprimees} supprimée(s)` : ''}. Copie de sécurité : ${r.sauvegarde}`)}
  />
{:else if importEnCours?.genre === 'operations'}
  <ImportOperations
    chemin={importEnCours.chemin}
    fermer={() => importTermine('')}
    fini={async () => { pf = await api.positions(); actualiser(); }}
  />
{/if}

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
    <div class="kpi"><span>Impôt</span><strong>0 €</strong><small class="discret">tant que l'argent reste dans le PEA</small></div>
  </div>

  {#if pf.lignes.length}
    <div class="kpis">
      <div class="kpi" title={AIDE_DIVIDENDES}>
        <span>Dividendes bruts des 12 derniers mois</span><strong>{euros(pf.div_brut_cents)}</strong>
        <small class="discret">détachés {periode12()}</small>
      </div>
      <div class="kpi" title={AIDE_DIVIDENDES}>
        <span>Dividendes nets des 12 derniers mois</span><strong class="positif">{euros(pf.div_net_cents)}</strong>
        <small class="discret">après retenues étrangères</small>
      </div>
      <div class="kpi"><span>Rendement des dividendes</span><strong>{pct(pf.div_rendement_micro)}</strong><small class="discret">dividendes bruts des 12 derniers mois / valeur actuelle</small></div>
      <div class="kpi" title="Pour chaque ligne : évolution moyenne par an du total des dividendes d'une année civile complète, entre la plus ancienne des 6 dernières années et la dernière (voir la fiche du titre). Pour le PEA : moyenne des lignes, pondérée par leur dividende.">
        <span>Croissance du dividende</span>
        <strong>{pf.div_croissance_micro === null ? '—' : `${signe(pf.div_croissance_micro)}${pct(pf.div_croissance_micro, 1)} / an`}</strong>
        <small class="discret">par an, années civiles complètes</small>
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
            <th class="nombre">Valeur</th><th class="nombre">+/− value</th><th class="nombre">1 an</th><th class="nombre" title="Dividendes bruts des 12 derniers mois / cours actuel">Rend.</th><th></th>
          </tr>
        </thead>
        <tbody>
          {#each pf.lignes as l (l.id)}
            <tr class:selectionnee={detail === l.symbole}>
              <td>
                <button class="lien" onclick={() => { detail = l.symbole; apercu = null; }}>{l.nom}</button>
                <div class="discret petit">{l.symbole}{l.pays ? ` · ${PAYS[l.pays] ?? l.pays}` : ''}{#if l.journal}<span class="badge" title="Quantité et PRU calculés à partir des achats et ventes importés">importée</span>{/if}</div>
              </td>
              {#if edition?.id === l.id && !l.journal}
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
                  <span title="{l.cours_source} · {heure(l.cours_horodatage)}">{fmt(l.cours_micro)}</span>
                  <div class="discret petit">{heure(l.cours_horodatage)}{l.cours_source && l.cours_source !== 'Yahoo Finance' ? ` · ${l.cours_source}` : ''}</div>
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
                  <button class="discret" onclick={() => (edition = { id: l.id, journal: l.journal, quantiteMicro: l.quantite_micro, pruMicro: l.pru_micro, quantite: microVersTexte(l.quantite_micro), pru: microVersTexte(l.pru_micro), pays: l.pays ?? '' })}>Modifier</button>
                  <button class="discret" onclick={() => (coursManuel = { symbole: l.symbole, valeur: '' })}>Cours</button>
                  <button class="discret" class:negatif={aSupprimer === l.id} onclick={() => supprimer(l)} aria-label="Supprimer" title={l.journal ? 'Supprime la ligne et ses achats et ventes importés' : 'Supprimer la ligne'}>{aSupprimer === l.id ? (l.journal ? 'Supprimer avec ses opérations ?' : 'Confirmer ?') : '✕'}</button>
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
        <input bind:value={recherche} oninput={chercher}
          placeholder="Nom, mnémonique ou ISIN : LVMH, MC, FR0000121014, MSCI World…" />
        {#if enCours}<span class="discret petit">Recherche…</span>{/if}
        {#if resultats.length}
          <ul class="resultats-recherche">
            {#each resultats as t}
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
          <label>PRU en euros<input bind:value={saisie.pru} inputmode="decimal" placeholder="Prix de revient affiché par la banque" /></label>
          <button type="submit" class="principal" disabled={chargement}>Ajouter {choisi.nom}</button>
        </div>
      {/if}
    </form>
    <p class="discret petit">
      Actions et ETF cotés en euros sur les places européennes. L'éligibilité au PEA dépend du pays de la société
      (Union européenne ou EEE) et, pour un ETF, de sa composition : elle est indiquée à titre indicatif, vérifiez-la auprès de votre banque.
      Cours Yahoo Finance, en général différés d'environ 15 minutes.
      Cliquez sur une ligne pour voir sa fiche et sa courbe.
    </p>
  </section>

  {#if ligneDetail}
    {@const l = ligneDetail}
    <section class="carte fiche">
      <h3>{l.nom} — dividendes et événements</h3>
      {#if l.div_par_action_micro > 0}
        <div class="fiche-grille">
          <div><span>Dividende / action, 12 derniers mois</span><b>{fmt(l.div_par_action_micro, 2)} €</b><small>{FREQUENCE[l.div_frequence] ?? `${l.div_frequence} versements`}, détachés {periode12()}</small></div>
          <div><span>Rendement</span><b>{l.div_rendement_micro ? pct(l.div_rendement_micro) : '—'}</b><small>12 derniers mois / cours actuel</small></div>
          <div><span>Croissance</span><b>{l.div_croissance_micro === null ? '—' : `${signe(l.div_croissance_micro)}${pct(l.div_croissance_micro, 1)} / an`}</b>{#if l.div_croissance_debut}<small>moyenne par an du total annuel, {l.div_croissance_debut} → {l.div_croissance_fin}</small>{/if}</div>
          <div><span>Prochain détachement</span><b>{jourFr(l.div_prochain_estime)}</b><small>estimé d'après l'an dernier</small></div>
        </div>
        <table class="decomposition">
          <tbody>
            <tr><td>Dividende brut des 12 derniers mois ({microVersTexte(l.quantite_micro)} actions)</td><td class="nombre">{euros(l.div_brut_cents)}</td></tr>
            <tr>
              <td>
                Retenue à la source {l.pays ? `(${PAYS[l.pays] ?? l.pays}, ${pct(l.retenue_micro, 2)})` : ''}
                {#if !l.retenue_connue}<span class="negatif petit"> — pays inconnu : indiquez-le avec « Modifier »</span>{/if}
                {#if l.retenue_micro > 0}<div class="discret petit">Perdue dans un PEA : aucun impôt français sur lequel l'imputer.</div>{/if}
              </td>
              <td class="nombre negatif">−{euros(l.div_retenue_cents)}</td>
            </tr>
            <tr class="total"><td>Dividende net encaissé sur 12 mois</td><td class="nombre positif">{euros(l.div_net_cents)}</td></tr>
          </tbody>
        </table>
        <p class="discret petit">{AIDE_DIVIDENDES}</p>
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
    {#key l.symbole}<Graphique symbole={l.symbole} nom={l.nom} pru={l.pru_micro} />{/key}
  {:else if apercu}
    {#key apercu.symbole}<Graphique symbole={apercu.symbole} nom={apercu.nom} />{/key}
  {/if}

  {#if parts.length > 1}
    <Donut titre="Répartition des lignes" {parts} />
  {/if}
{/if}
