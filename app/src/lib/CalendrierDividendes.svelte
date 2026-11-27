<script>
  // Calendrier des dividendes du PEA : à rapprocher, par mois, par année, synthèse.
  import { untrack } from 'svelte';
  import { api } from './api.js';
  import { euros, pct, microVersTexte, versCents } from './format.js';

  // pf : portefeuille (lignes, rendements). Le calendrier est recalculé à chaque mise à jour du portefeuille
  // (ligne modifiée, cours et dividendes actualisés).
  let { pf } = $props();

  let cal = $state(null);
  let erreur = $state(null);
  let vue = $state('mois');
  let annee = $state(null);
  let saisies = $state({}); // montant modifié d'un détachement à rapprocher, par clé symbole|date
  let enCours = $state(false);
  let aSupprimer = $state(null);
  let demande = 0;

  const MOIS = ['janv.', 'févr.', 'mars', 'avr.', 'mai', 'juin', 'juil.', 'août', 'sept.', 'oct.', 'nov.', 'déc.'];
  const MOIS_LONGS = ['Janvier', 'Février', 'Mars', 'Avril', 'Mai', 'Juin', 'Juillet', 'Août', 'Septembre', 'Octobre', 'Novembre', 'Décembre'];
  const STATUTS = { annonce: { texte: 'Annoncé', classe: 'positif' }, estime: { texte: 'Estimé', classe: '' } };
  const jour = (iso) => iso.split('-').reverse().join('/');
  const cle = (a) => `${a.symbole}|${a.date}`;

  async function charger() {
    const n = ++demande;
    try {
      const c = await api.calendrierDividendes(annee);
      if (n !== demande) return;
      cal = c;
      annee = c.annee;
      erreur = null;
    } catch (e) {
      if (n === demande) erreur = String(e);
    }
  }

  $effect(() => {
    pf;
    untrack(charger);
  });

  async function action(f) {
    enCours = true;
    try {
      await f();
      await charger();
    } catch (e) {
      erreur = String(e);
    } finally {
      enCours = false;
    }
  }

  function enregistrer(a) {
    const texte = saisies[cle(a)];
    const cents = texte === undefined ? a.net_cents : versCents(texte);
    if (!cents || cents <= 0) return (erreur = 'Indiquez le montant net reçu, en euros.');
    action(() => api.enregistrerDividende(a.symbole, a.date, cents));
  }

  function supprimer(id) {
    if (aSupprimer !== id) return (aSupprimer = id);
    aSupprimer = null;
    action(() => api.supprimerDividende(id));
  }

  // À venir, regroupé par mois.
  const groupes = $derived.by(() => {
    if (!cal) return [];
    const g = [];
    for (const a of cal.a_venir) {
      const [y, m] = a.date.split('-').map(Number);
      const titre = `${MOIS_LONGS[m - 1]} ${y}`;
      if (!g.length || g[g.length - 1].titre !== titre) g.push({ titre, elements: [], total: 0 });
      g[g.length - 1].elements.push(a);
      g[g.length - 1].total += a.net_cents;
    }
    return g;
  });

  // Histogramme : hauteur proportionnelle au plus haut mois de l'année.
  const maxMois = $derived(cal ? Math.max(1, ...cal.mois.map((m) => m.percu_cents + m.annonce_cents + m.estime_cents)) : 1);
  const totalAnnee = $derived(cal ? cal.mois.reduce((s, m) => s + m.percu_cents + m.annonce_cents + m.estime_cents, 0) : 0);
  const percuAnnee = $derived(cal ? cal.mois.reduce((s, m) => s + m.percu_cents, 0) : 0);
  const top5 = $derived(pf ? [...pf.lignes].filter((l) => l.div_net_cents > 0).sort((a, b) => b.div_net_cents - a.div_net_cents).slice(0, 5) : []);
</script>

<section class="carte calendrier">
  <h3>Calendrier des dividendes</h3>
  {#if erreur}<p class="message alerte">{erreur}</p>{/if}

  {#if cal?.a_rapprocher.length}
    <div class="message attention">
      <b>Dividendes détachés à enregistrer</b>
      <p class="petit">Montant net prérempli d'après la quantité détenue aujourd'hui : corrigez-le d'après votre relevé si besoin.</p>
      <div class="defilement">
        <table>
          <tbody>
            {#each cal.a_rapprocher as a (cle(a))}
              <tr>
                <td>{jour(a.date)}</td>
                <td class="titre-import">{a.nom}<div class="discret petit">{microVersTexte(a.quantite_micro)} × {microVersTexte(a.par_action_micro)} €{a.retenue_cents ? `, retenue ${euros(a.retenue_cents)}` : ''}</div></td>
                <td class="nombre">
                  <input class="court" inputmode="decimal" aria-label="Montant net reçu"
                    value={saisies[cle(a)] ?? (a.net_cents / 100).toFixed(2).replace('.', ',')}
                    oninput={(e) => (saisies[cle(a)] = e.currentTarget.value)} /> €
                </td>
                <td class="actions">
                  <button onclick={() => enregistrer(a)} disabled={enCours}>Enregistrer</button>
                  <button class="discret" onclick={() => action(() => api.ignorerDividende(a.symbole, a.date))} disabled={enCours}>Ignorer</button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    </div>
  {/if}

  <div class="segments" role="group" aria-label="Vue du calendrier">
    <button class:actif={vue === 'mois'} onclick={() => (vue = 'mois')}>À venir</button>
    <button class:actif={vue === 'annee'} onclick={() => (vue = 'annee')}>Année</button>
    <button class:actif={vue === 'percus'} onclick={() => (vue = 'percus')}>Perçus</button>
    <button class:actif={vue === 'synthese'} onclick={() => (vue = 'synthese')}>Synthèse</button>
  </div>

  {#if !cal}
    <p class="discret">Chargement…</p>
  {:else if vue === 'mois'}
    {#if cal.a_venir.length === 0}
      <p class="vide">Aucun détachement prévu dans les 12 prochains mois (ou historique pas encore récupéré : cliquez sur « Actualiser les cours »).</p>
    {:else}
      <p>12 prochains mois : <b class="positif">{euros(cal.total_a_venir_net_cents)}</b> nets attendus.</p>
      <div class="defilement">
        <table class="calendrier-liste">
          <thead><tr><th>Date</th><th>Titre</th><th>Statut</th><th class="nombre">Par action</th><th class="nombre">Net</th></tr></thead>
          {#each groupes as g (g.titre)}
            <tbody>
              <tr class="mois-titre"><td colspan="4">{g.titre}</td><td class="nombre">{euros(g.total)}</td></tr>
              {#each g.elements as a (cle(a))}
                <tr>
                  <td>{jour(a.date)}</td>
                  <td class="titre-import">{a.nom}</td>
                  <td><span class="badge {STATUTS[a.statut].classe}">{STATUTS[a.statut].texte}</span></td>
                  <td class="nombre">{microVersTexte(a.par_action_micro)} €</td>
                  <td class="nombre" title={`Brut ${euros(a.brut_cents)}${a.retenue_cents ? `, retenue ${euros(a.retenue_cents)}` : ''}`}>{euros(a.net_cents)}</td>
                </tr>
              {/each}
            </tbody>
          {/each}
        </table>
      </div>
      <p class="discret petit">
        Estimé : même date que l'an dernier, montant du même coupon l'an dernier ; si le dernier dividende a changé par
        rapport à l'an dernier, le même changement est appliqué (au-delà de ±50 %, il ne l'est pas). Quantité détenue
        aujourd'hui, retenue à la source déduite. À confirmer sur le site de la société.
      </p>
    {/if}
  {:else if vue === 'annee'}
    <div class="actions">
      <label>Année
        <select value={annee} onchange={(e) => { annee = Number(e.currentTarget.value); charger(); }}>
          {#each cal.annees as a (a)}<option value={a}>{a}</option>{/each}
        </select>
      </label>
      <span>Total {cal.annee} : <b>{euros(totalAnnee)}</b> nets, dont {euros(percuAnnee)} perçus</span>
    </div>
    <svg class="histogramme" viewBox="0 0 600 220" role="img" aria-label={`Dividendes nets par mois en ${cal.annee}`}>
      {#each cal.mois as m, i (m.mois)}
        {@const x = 10 + i * 49}
        {@const h1 = (m.percu_cents / maxMois) * 170}
        {@const h2 = (m.annonce_cents / maxMois) * 170}
        {@const h3 = (m.estime_cents / maxMois) * 170}
        <g>
          <title>{MOIS_LONGS[i]} : perçu {euros(m.percu_cents)}, annoncé {euros(m.annonce_cents)}, estimé {euros(m.estime_cents)}</title>
          <rect x={x} y={190 - h1} width="36" height={h1} class="barre-percu" />
          <rect x={x} y={190 - h1 - h2} width="36" height={h2} class="barre-annonce" />
          <rect x={x} y={190 - h1 - h2 - h3} width="36" height={h3} class="barre-estime" />
          <text x={x + 18} y="207" text-anchor="middle">{MOIS[i]}</text>
        </g>
      {/each}
      <line x1="5" y1="190" x2="595" y2="190" class="axe" />
    </svg>
    <p class="legende-histogramme petit">
      <span><i class="barre-percu"></i>Perçu (enregistré)</span>
      <span><i class="barre-annonce"></i>Annoncé</span>
      <span><i class="barre-estime"></i>Estimé</span>
    </p>
  {:else if vue === 'percus'}
    {#if cal.percus.length === 0}
      <p class="vide">Aucun dividende enregistré. Les détachements récents sont proposés ci-dessus pendant 90 jours ; un import d'opérations peut aussi en ajouter.</p>
    {:else}
      <div class="defilement">
        <table>
          <thead><tr><th>Date</th><th>Titre</th><th class="nombre">Net reçu</th><th></th></tr></thead>
          <tbody>
            {#each cal.percus as d (d.id)}
              <tr>
                <td>{jour(d.date)}</td>
                <td class="titre-import">{d.nom}</td>
                <td class="nombre positif">{euros(d.net_cents)}</td>
                <td class="actions"><button class="discret" class:negatif={aSupprimer === d.id} onclick={() => supprimer(d.id)}>{aSupprimer === d.id ? 'Confirmer ?' : '✕'}</button></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {:else if vue === 'synthese' && pf}
    <div class="kpis">
      <div class="kpi"><span>Dividendes nets, 12 derniers mois</span><strong class="positif">{euros(pf.div_net_cents)}</strong><small class="discret">détachés, quantité actuelle</small></div>
      <div class="kpi"><span>Dividendes nets, 12 prochains mois</span><strong>{euros(cal.total_a_venir_net_cents)}</strong><small class="discret">annoncés et estimés</small></div>
      <div class="kpi"><span>Rendement sur la valeur</span><strong>{pct(pf.div_rendement_micro)}</strong><small class="discret">brut 12 mois / valeur actuelle</small></div>
      <div class="kpi"><span>Rendement sur le PRU</span><strong>{pct(pf.div_rendement_pru_micro)}</strong><small class="discret">brut 12 mois / montant investi</small></div>
    </div>
    {#if top5.length}
      <h4>Ce qui rapporte le plus (12 derniers mois, net)</h4>
      <div class="defilement">
        <table>
          <tbody>
            {#each top5 as l (l.id)}
              <tr>
                <td class="titre-import">{l.nom}</td>
                <td class="nombre">{euros(l.div_net_cents)}</td>
                <td class="nombre discret">{pf.div_net_cents ? pct(Math.round((l.div_net_cents / pf.div_net_cents) * 1e6), 0) : ''}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/if}
</section>
