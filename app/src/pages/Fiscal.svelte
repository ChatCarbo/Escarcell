<script>
  import { api } from '../lib/api.js';
  import { euros, pct, versCents } from '../lib/format.js';

  let s = $state({
    revenu: '', couple: false, enfants: 0,
    donsAide: '', donsAutres: '',
  });
  let r = $state(null);
  let erreur = $state('');

  async function calculer(ev) {
    ev?.preventDefault();
    erreur = '';
    const revenu = versCents(s.revenu);
    if (revenu === null) return (erreur = 'Indiquez le revenu net imposable du foyer.');
    try {
      r = await api.simulationFiscale({
        revenu_cents: revenu,
        couple: s.couple,
        enfants: Number(s.enfants) || 0,
        versement_per_cents: 0,
        plafond_per_cents: null,
        tmi_retraite_micro: 0,
        dons_aide_personnes_cents: versCents(s.donsAide) ?? 0,
        dons_interet_general_cents: versCents(s.donsAutres) ?? 0,
      });
    } catch (e) {
      erreur = String(e);
    }
  }
</script>

<header class="entete"><h1>Impôt et dons</h1></header>
{#if erreur}<p class="erreur">{erreur}</p>{/if}

<div class="grille-2">
  <form class="carte" onsubmit={calculer}>
    <h3>Mon foyer (revenus 2025)</h3>
    <label>Revenu net imposable du foyer (€)<input bind:value={s.revenu} inputmode="decimal" placeholder="Indiqué sur l'avis d'imposition" /></label>
    <label class="case"><input type="checkbox" bind:checked={s.couple} /> Marié ou pacsé</label>
    <label>Enfants à charge<input type="number" min="0" bind:value={s.enfants} /></label>

    <h3>Dons de l'année</h3>
    <label>Aide aux personnes en difficulté (€)<input bind:value={s.donsAide} inputmode="decimal" placeholder="Restos du Cœur, Secours populaire…" /></label>
    <label>Autres associations d'intérêt général (€)<input bind:value={s.donsAutres} inputmode="decimal" placeholder="0" /></label>

    <button type="submit" class="principal">Calculer</button>
  </form>

  {#if r}
    <div class="colonne">
      <section class="carte">
        <h3>Mon impôt</h3>
        <dl class="resultats">
          <dt>Impôt avant dons</dt><dd>{euros(r.impot.impot_cents)}</dd>
          <dt>Tranche marginale (TMI)</dt><dd>{pct(r.impot.tmi_micro, 0)}</dd>
          {#if r.impot.decote_cents > 0}
            <dt>Décote</dt><dd>−{euros(r.impot.decote_cents)}</dd>
            <dt>Taux marginal réel (zone de décote)</dt><dd>{pct(r.impot.taux_marginal_effectif_micro)}</dd>
          {/if}
          <dt>Taux moyen</dt><dd>{pct(r.impot.taux_moyen_micro)}</dd>
          {#if r.dons}
            <dt><b>Impôt après dons</b></dt><dd><b>{euros(r.impot_final_cents)}</b></dd>
            <dt>Économie totale</dt><dd class="positif">{euros(r.economie_totale_cents)}</dd>
          {/if}
        </dl>
      </section>

      {#if r.dons}
        <section class="carte">
          <h3>Réduction pour dons</h3>
          <dl class="resultats">
            <dt>Réduction d'impôt</dt><dd class="positif">{euros(r.dons.reduction_cents)}</dd>
            <dt>Coût réel de vos dons</dt><dd>{euros(r.dons.cout_reel_cents)}</dd>
            {#if r.dons.excedent_reportable_cents > 0}<dt>Reportable sur 5 ans (au-delà de 20 % du revenu)</dt><dd>{euros(r.dons.excedent_reportable_cents)}</dd>{/if}
            {#if r.dons.perdue_cents > 0}<dt>Réduction perdue (impôt insuffisant)</dt><dd class="negatif">{euros(r.dons.perdue_cents)}</dd>{/if}
          </dl>
          <p class="discret petit">75 % pour les organismes d'aide aux personnes en difficulté ou aux victimes de violence, jusqu'à 2 000 € de dons par an ; 66 % au-delà et pour les autres associations, dans la limite de 20 % du revenu imposable. Un don de 100 € à une association classique coûte 34 €. La réduction ne peut pas dépasser l'impôt dû.</p>
        </section>
      {/if}
    </div>
  {/if}
</div>
