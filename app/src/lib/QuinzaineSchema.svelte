<script>
  // Frise des quinzaines autour d'une date : chaque jour est coloré selon qu'il rapporte ou non.
  // sens : 'VERSEMENT' ou 'RETRAIT' ; conseil : réponse de conseil_livret.
  let { sens, date, conseil } = $props();

  const COULEURS = {
    remunere: '#10B981',
    attente: '#FBBF24',
    perdu: '#EF4444',
    hors: '#CBD5E1',
  };

  const d = (s) => new Date(s + 'T00:00:00Z');
  const iso = (x) => x.toISOString().slice(0, 10);
  const plus = (x, n) => new Date(x.getTime() + n * 86400000);

  // Fenêtre : de la quinzaine précédente à la quinzaine suivant la date d'effet.
  let jours = $derived.by(() => {
    if (!conseil) return [];
    const debutQ = d(conseil.debut_quinzaine);
    const debut = plus(debutQ, -15);
    const fin = plus(d(conseil.fin_quinzaine), 16);
    const res = [];
    for (let x = debut; x <= fin; x = plus(x, 1)) {
      const j = iso(x);
      let etat;
      if (sens === 'VERSEMENT') {
        // Avant le versement : argent ailleurs ; jusqu'à la fin de quinzaine : en attente ; ensuite : rémunéré.
        etat = j < date ? 'hors' : j < conseil.date_effet ? 'attente' : 'remunere';
      } else {
        // Avant la quinzaine : rémunéré ; de son début au retrait : perdu ; ensuite : argent retiré.
        etat = j < conseil.date_effet ? 'remunere' : j <= date ? 'perdu' : 'hors';
      }
      res.push({ j, jour: x.getUTCDate(), etat, conseil: j === conseil.date_conseillee, choix: j === date });
    }
    return res;
  });

  const L = 12; // largeur d'un jour
</script>

{#if jours.length}
  <svg viewBox="0 0 {jours.length * L} 58" class="frise" role="img" aria-label="Frise des quinzaines">
    {#each jours as x, i}
      <rect x={i * L + 1} y="14" width={L - 2} height="22" rx="2" fill={COULEURS[x.etat]} opacity={x.etat === 'hors' ? 0.5 : 1} />
      {#if x.jour === 1 || x.jour === 16}
        <line x1={i * L} x2={i * L} y1="8" y2="42" class="frise-borne" />
        <text x={i * L + 2} y="52" class="frise-texte">{x.jour === 1 ? `1er/${x.j.slice(5, 7)}` : `16/${x.j.slice(5, 7)}`}</text>
      {/if}
      {#if x.choix}
        <text x={i * L + L / 2} y="10" class="frise-repere">▼</text>
      {/if}
      {#if x.conseil && !x.choix}
        <text x={i * L + L / 2} y="10" class="frise-repere conseil">★</text>
      {/if}
    {/each}
  </svg>
  <div class="frise-legende">
    {#if sens === 'VERSEMENT'}
      <span><i style="background:{COULEURS.attente}"></i>en attente, ne rapporte pas</span>
      <span><i style="background:{COULEURS.remunere}"></i>rapporte des intérêts</span>
      <span>▼ date choisie</span>
      <span class="conseil">★ dernier jour pour verser sans perdre de quinzaine</span>
    {:else}
      <span><i style="background:{COULEURS.remunere}"></i>a rapporté</span>
      <span><i style="background:{COULEURS.perdu}"></i>quinzaine perdue pour ce montant</span>
      <span>▼ date choisie</span>
      <span class="conseil">★ date conseillée si vous pouvez attendre</span>
    {/if}
  </div>
{/if}
