<script>
  import { api } from '../lib/api.js';
  import { euros, pct, pctVersMicro, versCents } from '../lib/format.js';

  let h = $state({ initial: '5000', mensuel: '200', annees: 20, fonds_euros: '30', rendement_uc: '6', ter: '0,2' });
  let resultats = $state([]);
  let erreur = $state('');

  async function comparer(ev) {
    ev?.preventDefault();
    erreur = '';
    try {
      resultats = await api.comparerPer({
        versement_initial_cents: versCents(h.initial) ?? 0,
        versement_mensuel_cents: versCents(h.mensuel) ?? 0,
        annees: Number(h.annees) || 1,
        part_fonds_euros_micro: pctVersMicro(h.fonds_euros) ?? 0,
        rendement_brut_uc_micro: pctVersMicro(h.rendement_uc) ?? 0,
        ter_uc_micro: pctVersMicro(h.ter) ?? 0,
      });
    } catch (e) {
      erreur = String(e);
    }
  }
</script>

<section class="carte">
  <h3>Comparer les PER (gestion libre)</h3>
  <form class="ligne-formulaire" onsubmit={comparer}>
    <label>Versement initial (€)<input bind:value={h.initial} inputmode="decimal" /></label>
    <label>Par mois (€)<input bind:value={h.mensuel} inputmode="decimal" /></label>
    <label>Durée (ans)<input type="number" min="1" max="50" bind:value={h.annees} /></label>
    <label>Fonds euros (%)<input bind:value={h.fonds_euros} inputmode="decimal" /></label>
    <label>Rendement UC brut (%)<input bind:value={h.rendement_uc} inputmode="decimal" /></label>
    <label>Frais ETF (%)<input bind:value={h.ter} inputmode="decimal" /></label>
    <button type="submit" class="principal">Comparer</button>
  </form>
  {#if erreur}<p class="erreur">{erreur}</p>{/if}
  {#if resultats.length}
    <table>
      <thead><tr><th>Contrat</th><th class="nombre">Capital final</th><th class="nombre">Frais cumulés</th><th class="nombre">Écart</th></tr></thead>
      <tbody>
        {#each resultats as r (r.nom)}
          <tr class:discret={r.plafonnee}>
            <td>{r.nom}{#if r.plafonnee}<span class="badge">non comparable : fonds euros limités à {pct(r.part_fonds_euros_micro, 0)}</span>{/if}</td>
            <td class="nombre">{euros(r.capital_final_cents)}</td>
            <td class="nombre">{euros(r.frais_totaux_cents)}</td>
            <td class="nombre" class:negatif={r.ecart_cents < 0}>{r.plafonnee ? '—' : euros(r.ecart_cents)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="discret">
      Versements : {euros(resultats[0].versements_cents)}. Hypothèses constantes : rendement 2025 de chaque fonds euros, rendement UC identique pour tous.
      Bonus commerciaux exclus. Escarcelle ne perçoit rien des établissements comparés.
    </p>
  {/if}
</section>
