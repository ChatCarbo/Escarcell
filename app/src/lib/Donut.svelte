<script>
  import { euros } from './format.js';

  /** parts : [{ cle, libelle, montant_cents, couleur }] */
  let { titre, parts = [] } = $props();

  const R = 15.915; // circonférence de 100 : les longueurs sont des pourcentages
  let survol = $state(null);

  // Six parts au plus ; au-delà, regroupement en « Autres ».
  let visibles = $derived.by(() => {
    const p = parts.filter((x) => x.montant_cents > 0).sort((a, b) => b.montant_cents - a.montant_cents);
    if (p.length <= 6) return p;
    const autres = p.slice(5).reduce((s, x) => s + x.montant_cents, 0);
    return [...p.slice(0, 5), { cle: 'autres', libelle: 'Autres', montant_cents: autres, couleur: '#CBD5E1' }];
  });
  let total = $derived(visibles.reduce((s, x) => s + x.montant_cents, 0));
  let segments = $derived.by(() => {
    let cumul = 0;
    return visibles.map((x) => {
      const part = total ? (x.montant_cents / total) * 100 : 0;
      const s = { ...x, part, debut: cumul };
      cumul += part;
      return s;
    });
  });
</script>

<section class="carte">
  <h3>{titre}</h3>
  {#if total === 0}
    <p class="vide">Aucun montant à afficher.</p>
  {:else}
    <div class="donut">
      <svg viewBox="0 0 42 42" role="img" aria-label={titre}>
        {#each segments as s (s.cle)}
          <circle
            cx="21" cy="21" r={R} fill="none"
            stroke={s.couleur ?? '#94A3B8'}
            stroke-width={survol === s.cle ? 7 : 6}
            stroke-dasharray="{Math.max(s.part - 0.5, 0.2)} {100 - Math.max(s.part - 0.5, 0.2)}"
            stroke-dashoffset={25 - s.debut}
            role="presentation"
            onmouseenter={() => (survol = s.cle)}
            onmouseleave={() => (survol = null)}
          >
            <title>{s.libelle} : {euros(s.montant_cents)}</title>
          </circle>
        {/each}
        <text x="21" y="20.5" class="donut-total">{euros(total)}</text>
        <text x="21" y="25" class="donut-sous">total</text>
      </svg>
      <ul class="legende">
        {#each segments as s (s.cle)}
          <li class:actif={survol === s.cle}>
            <span class="pastille" style="background:{s.couleur ?? '#94A3B8'}"></span>
            <span class="libelle">{s.libelle}</span>
            <span class="nombre">{euros(s.montant_cents)}</span>
            <span class="nombre discret">{s.part.toFixed(0)} %</span>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>
