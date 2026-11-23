<script>
  import { api } from './api.js';

  /** pru : prix de revient (en millionièmes) pour tracer la ligne d'achat, facultatif */
  let { symbole, nom, pru = null } = $props();

  const PERIODES = [['3mo', '3 mois'], ['6mo', '6 mois'], ['1y', '1 an'], ['2y', '2 ans'], ['5y', '5 ans']];
  let periode = $state('1y');
  let a = $state(null);
  let erreur = $state('');
  let chargement = $state(false);
  let couches = $state({ moyennes: true, bollinger: false, niveaux: true });
  let survol = $state(null);

  $effect(() => {
    const s = symbole, p = periode;
    chargement = true;
    erreur = '';
    api.analyseTitre(s, p)
      .then((r) => (a = r))
      .catch((e) => { erreur = String(e); a = null; })
      .finally(() => (chargement = false));
  });

  const W = 860, H = 280, D = 64; // largeur, hauteur du graphique de cours, marge droite (axe)
  const HR = 80; // hauteur RSI et MACD
  const pruVal = $derived(pru ? pru / 1e6 : null);

  let echelle = $derived.by(() => {
    if (!a?.points.length) return null;
    const p = a.points;
    let min = Infinity, max = -Infinity;
    for (const x of p) {
      min = Math.min(min, x.bas, couches.bollinger && x.bol_bas ? x.bol_bas : Infinity);
      max = Math.max(max, x.haut, couches.bollinger && x.bol_haut ? x.bol_haut : -Infinity);
    }
    if (pruVal) { min = Math.min(min, pruVal); max = Math.max(max, pruVal); }
    const marge = (max - min) * 0.05 || 1;
    min -= marge; max += marge;
    const pas = (W - D) / Math.max(p.length - 1, 1);
    return { min, max, pas, x: (i) => i * pas, y: (v) => H - ((v - min) / (max - min)) * H };
  });

  // Trace une série en sautant les valeurs absentes.
  function chemin(cle, y = echelle?.y) {
    if (!a || !echelle) return '';
    let d = '', ouvert = false;
    a.points.forEach((p, i) => {
      const v = p[cle];
      if (v === null || v === undefined) { ouvert = false; return; }
      d += `${ouvert ? 'L' : 'M'}${echelle.x(i).toFixed(1)},${y(v).toFixed(1)}`;
      ouvert = true;
    });
    return d;
  }

  let bandeBollinger = $derived.by(() => {
    if (!a || !echelle || !couches.bollinger) return '';
    const pts = a.points.map((p, i) => [i, p]).filter(([, p]) => p.bol_haut !== null);
    if (!pts.length) return '';
    const haut = pts.map(([i, p]) => `${echelle.x(i)},${echelle.y(p.bol_haut)}`).join('L');
    const bas = pts.reverse().map(([i, p]) => `${echelle.x(i)},${echelle.y(p.bol_bas)}`).join('L');
    return `M${haut}L${bas}Z`;
  });

  const yRsi = (v) => HR - (v / 100) * HR;
  let macdEchelle = $derived.by(() => {
    if (!a) return null;
    const vals = a.points.flatMap((p) => [p.macd, p.macd_signal]).filter((v) => v !== null);
    const m = Math.max(...vals.map(Math.abs), 1e-9);
    return (v) => HR / 2 - (v / m) * (HR / 2 - 2);
  });

  const graduations = $derived(echelle ? Array.from({ length: 5 }, (_, k) => echelle.min + ((echelle.max - echelle.min) * (k + 0.5)) / 5) : []);
  const datesAxe = $derived(a ? a.points.map((p, i) => [i, p]).filter(([i]) => i % Math.ceil(a.points.length / 6) === 0) : []);
  const fmtDate = (t) => new Date(t * 1000).toLocaleDateString('fr-FR', { day: '2-digit', month: 'short', year: '2-digit' });
  const fmtPrix = (v) => v?.toLocaleString('fr-FR', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

  function bouger(ev) {
    if (!a || !echelle) return;
    const r = ev.currentTarget.getBoundingClientRect();
    const x = ((ev.clientX - r.left) / r.width) * W;
    const i = Math.max(0, Math.min(a.points.length - 1, Math.round(x / echelle.pas)));
    survol = i;
  }

  const SENS = { haussier: 'positif', baissier: 'negatif', neutre: 'discret' };
  const SYNTHESE = {
    haussier: 'Les signaux techniques penchent plutôt à la hausse',
    baissier: 'Les signaux techniques penchent plutôt à la baisse',
    neutre: 'Pas de tendance technique nette',
  };
  const dernier = $derived(a?.points.at(-1));
  const variation = $derived(a && a.points.length > 1 ? (dernier.cloture / a.points[0].cloture - 1) * 100 : 0);

  // Étiquettes des niveaux espacées d'au moins 14 px pour rester lisibles quand les niveaux sont proches.
  const etiquettes = $derived.by(() => {
    if (!a || !echelle) return [];
    const visibles = a.niveaux
      .filter((n) => n.prix > echelle.min && n.prix < echelle.max)
      .map((n) => ({ ...n, y: echelle.y(n.prix) }))
      .sort((p, q) => q.y - p.y);
    let plafond = Infinity;
    for (const n of visibles) {
      n.yTexte = Math.min(n.y - 4, plafond - 14);
      plafond = n.yTexte;
    }
    return visibles;
  });
</script>

<section class="carte graphique">
  <div class="graphique-tete">
    <div>
      <h3>{nom}</h3>
      {#if dernier}
        <strong class="solde">{fmtPrix(dernier.cloture)} €</strong>
        <span class:positif={variation > 0} class:negatif={variation < 0}>{variation > 0 ? '+' : ''}{variation.toFixed(1).replace('.', ',')} % sur la période</span>
      {/if}
    </div>
    <div class="segments">
      {#each PERIODES as [v, l]}<button type="button" class:actif={periode === v} onclick={() => (periode = v)}>{l}</button>{/each}
    </div>
  </div>

  <div class="couches">
    <label class="case"><input type="checkbox" bind:checked={couches.moyennes} /> Moyennes 20/50/200 j</label>
    <label class="case"><input type="checkbox" bind:checked={couches.bollinger} /> Bandes de Bollinger</label>
    <label class="case"><input type="checkbox" bind:checked={couches.niveaux} /> Supports et résistances</label>
  </div>

  {#if erreur}<p class="erreur">{erreur}</p>{/if}
  {#if chargement && !a}<p class="discret">Chargement de l'historique…</p>{/if}

  {#if a && echelle}
    <svg viewBox="0 0 {W} {H + 20}" class="courbe" onmousemove={bouger} onmouseleave={() => (survol = null)} role="img" aria-label="Cours de {nom}">
      {#each graduations as g}
        <line x1="0" x2={W - D} y1={echelle.y(g)} y2={echelle.y(g)} class="grille" />
        <text x={W - D + 6} y={echelle.y(g) + 4} class="axe">{fmtPrix(g)}</text>
      {/each}
      {#each datesAxe as [i, p]}<text x={echelle.x(i)} y={H + 15} class="axe">{fmtDate(p.t)}</text>{/each}

      {#if couches.bollinger}<path d={bandeBollinger} class="bollinger" />{/if}
      {#if couches.niveaux}
        {#each etiquettes as n}
          <line x1="0" x2={W - D} y1={n.y} y2={n.y} class="niveau {n.type}" />
          <text x="4" y={n.yTexte} class="niveau-texte {n.type}">{n.type === 'support' ? 'Support' : 'Résistance'} {fmtPrix(n.prix)} ({n.touches}×)</text>
        {/each}
      {/if}
      {#if pruVal}
        <line x1="0" x2={W - D} y1={echelle.y(pruVal)} y2={echelle.y(pruVal)} class="ligne-pru" />
        <text x={W - D - 4} y={echelle.y(pruVal) - 4} class="niveau-texte pru" text-anchor="end">Mon PRU {fmtPrix(pruVal)}</text>
      {/if}
      {#if couches.moyennes}
        <path d={chemin('sma20')} class="mm mm20" />
        <path d={chemin('sma50')} class="mm mm50" />
        <path d={chemin('sma200')} class="mm mm200" />
      {/if}
      <path d={chemin('cloture')} class="cours" />

      {#if survol !== null}
        {@const p = a.points[survol]}
        <line x1={echelle.x(survol)} x2={echelle.x(survol)} y1="0" y2={H} class="curseur" />
        <circle cx={echelle.x(survol)} cy={echelle.y(p.cloture)} r="3.5" class="point" />
        <g transform="translate({Math.min(echelle.x(survol) + 8, W - D - 150)},8)">
          <rect width="142" height="44" rx="4" class="bulle" />
          <text x="8" y="17" class="bulle-texte">{fmtDate(p.t)}</text>
          <text x="8" y="35" class="bulle-texte fort">{fmtPrix(p.cloture)} €{p.rsi !== null ? ` · RSI ${p.rsi.toFixed(0)}` : ''}</text>
        </g>
      {/if}
    </svg>
    {#if couches.moyennes}
      <div class="frise-legende">
        <span><i class="leg mm20"></i>moyenne 20 j</span><span><i class="leg mm50"></i>50 j</span><span><i class="leg mm200"></i>200 j</span>
      </div>
    {/if}

    <div class="indicateurs">
      <div>
        <span class="discret petit">RSI 14 (au-dessus de 70 : surachat ; en dessous de 30 : survente)</span>
        <svg viewBox="0 0 {W} {HR}" class="courbe">
          <rect x="0" y={yRsi(70)} width={W - D} height={yRsi(30) - yRsi(70)} class="zone-rsi" />
          <line x1="0" x2={W - D} y1={yRsi(70)} y2={yRsi(70)} class="grille" />
          <line x1="0" x2={W - D} y1={yRsi(30)} y2={yRsi(30)} class="grille" />
          <text x={W - D + 6} y={yRsi(70) + 4} class="axe">70</text>
          <text x={W - D + 6} y={yRsi(30) + 4} class="axe">30</text>
          <path d={chemin('rsi', yRsi)} class="rsi" />
        </svg>
      </div>
      <div>
        <span class="discret petit">MACD (12, 26, 9) : quand la ligne bleue passe au-dessus de l'orange, l'élan devient positif</span>
        <svg viewBox="0 0 {W} {HR}" class="courbe">
          {#each a.points as p, i}
            {#if p.macd !== null && p.macd_signal !== null}
              {@const h = p.macd - p.macd_signal}
              <rect x={echelle.x(i) - echelle.pas * 0.35} width={Math.max(echelle.pas * 0.7, 0.8)}
                y={Math.min(macdEchelle(h), HR / 2)} height={Math.abs(macdEchelle(h) - HR / 2)} class={h >= 0 ? 'histo-pos' : 'histo-neg'} />
            {/if}
          {/each}
          <line x1="0" x2={W - D} y1={HR / 2} y2={HR / 2} class="grille" />
          <path d={chemin('macd', macdEchelle)} class="macd" />
          <path d={chemin('macd_signal', macdEchelle)} class="macd-signal" />
        </svg>
      </div>
    </div>

    <div class="signaux">
      <p class="message {a.synthese === 'haussier' ? 'ok' : a.synthese === 'baissier' ? 'alerte' : 'attention'}">
        <b>{SYNTHESE[a.synthese]}.</b>
        {a.signaux.length ? `${a.signaux.length} ${a.signaux.length > 1 ? 'signaux détectés' : 'signal détecté'}.` : 'Aucun signal particulier en ce moment.'}
      </p>
      {#each a.signaux as s}
        <div class="signal">
          <span class="badge {SENS[s.sens]}">{s.sens}</span>
          <div><b>{s.nom}</b><br /><span class="discret">{s.explication}</span></div>
        </div>
      {/each}
      <p class="discret petit">
        L'analyse technique décrit le comportement passé du cours. Elle ne prédit pas l'avenir de façon fiable et beaucoup de
        signaux se révèlent faux, surtout sur une seule journée. Ce n'est pas un conseil d'achat ou de vente.
      </p>
    </div>
  {/if}
</section>
