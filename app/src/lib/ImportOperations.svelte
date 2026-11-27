<script>
  // Import détaillé d'un historique d'opérations (PEA et livrets), ouvert depuis la page PEA.
  import { onMount } from 'svelte';
  import { save } from '@tauri-apps/plugin-dialog';
  import { api } from './api.js';
  import { euros, microVersTexte } from './format.js';
  import { exporterCsv, eurosCsv, microCsv } from './export.js';

  let { chemin, fini, fermer } = $props();

  let message = $state(null); // { classe, texte }
  let champs = $state([]); // [{ nom, obligatoire }]
  let analyse = $state(null);
  let correspondance = $state(null); // champ par colonne, null = proposition automatique
  let inclureDoublons = $state(false);
  let filtre = $state('problemes');
  let limite = $state(200);
  let enCours = $state(false);
  let confirmerImport = $state(false);
  let rapport = $state(null);
  let enregistrees = $state([]);
  let nomCorrespondance = $state('');

  // Chaque analyse a un numéro : une réponse arrivée après une analyse plus récente est ignorée.
  let demande = 0;

  const STATUTS = {
    valide: { texte: 'Valide', classe: 'positif' },
    avertissement: { texte: 'Avertissement', classe: 'attention' },
    erreur: { texte: 'Erreur', classe: 'negatif' },
    doublon: { texte: 'Doublon', classe: '' },
    ignoree: { texte: 'Non importée', classe: '' },
  };
  const LIBELLES = {
    date: 'Date', compte: 'Compte', type: 'Type', isin: 'ISIN', libelle: 'Libellé', quantite: 'Quantité',
    prix_unitaire: 'Prix unitaire', montant: 'Montant', frais: 'Frais', devise: 'Devise', note: 'Note',
  };

  onMount(async () => {
    try {
      champs = await api.champsImport();
      enregistrees = await api.correspondances();
      await analyser();
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  });

  async function analyser() {
    if (!chemin) return;
    const n = ++demande;
    enCours = true;
    try {
      const a = await api.analyserImport(chemin, correspondance, inclureDoublons);
      if (n !== demande) return;
      analyse = a;
      correspondance = [...a.correspondance];
      confirmerImport = false;
    } catch (e) {
      if (n === demande) message = { classe: 'alerte', texte: String(e) };
    } finally {
      if (n === demande) enCours = false;
    }
  }

  async function telechargerModele() {
    const c = await save({ defaultPath: 'modele-escarcell.csv', filters: [{ name: 'Fichier CSV', extensions: ['csv'] }] });
    if (!c) return;
    try {
      await api.enregistrerModele(c);
      message = { classe: 'ok', texte: `Modèle enregistré : ${c}` };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  function changerColonne(i, valeur) {
    const c = [...correspondance];
    // Un champ n'est associé qu'à une colonne : on le retire de l'ancienne.
    if (valeur) for (let k = 0; k < c.length; k++) if (k !== i && c[k] === valeur) c[k] = null;
    c[i] = valeur || null;
    correspondance = c;
    analyser();
  }

  function appliquerEnregistree(nom) {
    const e = enregistrees.find((x) => x.nom === nom);
    if (!e || !analyse) return;
    // Association par intitulé de colonne : l'ordre des colonnes peut changer d'un export à l'autre.
    correspondance = analyse.entetes.map((t) => e.colonnes.find(([intitule]) => intitule === t)?.[1] ?? null);
    nomCorrespondance = nom;
    analyser();
  }

  async function enregistrerCorrespondance() {
    try {
      const colonnes = analyse.entetes.map((t, i) => [t, correspondance[i] ?? null]);
      enregistrees = await api.enregistrerCorrespondance(nomCorrespondance, colonnes);
      message = { classe: 'ok', texte: `Correspondance « ${nomCorrespondance.trim()} » enregistrée.` };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function supprimerCorrespondance() {
    try {
      enregistrees = await api.supprimerCorrespondance(nomCorrespondance);
      nomCorrespondance = '';
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function importer() {
    if (!confirmerImport) return (confirmerImport = true);
    confirmerImport = false;
    enCours = true;
    message = null;
    try {
      rapport = await api.importer(chemin, correspondance, inclureDoublons);
      analyse = rapport.analyse;
      fini?.(rapport);
      message = {
        classe: 'ok',
        texte: `${rapport.analyse.compteurs.a_importer} opération(s) importée(s). Copie de sécurité faite avant l'import : ${rapport.sauvegarde}`,
      };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    } finally {
      enCours = false;
    }
  }

  async function exporterRapport() {
    const entetes = ['ligne', 'statut', 'importee', 'date', 'compte', 'type', 'isin', 'libelle', 'quantite', 'prix_unitaire', 'montant', 'frais', 'messages'];
    const lignes = analyse.lignes.map((l) => [
      l.numero, STATUTS[l.statut].texte, rapport && l.a_importer ? 'oui' : 'non', l.date ?? '', l.compte ?? '', l.type ?? '',
      l.isin ?? '', l.libelle ?? '', microCsv(l.quantite_micro, 6), microCsv(l.prix_micro, 6), eurosCsv(l.montant_cents),
      eurosCsv(l.frais_cents), (l.brut.length ? `[${l.brut.join(' | ')}] ` : '') + l.messages.join(' ; '),
    ]);
    try {
      const c = await exporterCsv('rapport-import', entetes, lignes);
      if (c) message = { classe: 'ok', texte: `Rapport enregistré : ${c}` };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  const affichees = $derived(
    analyse ? analyse.lignes.filter((l) => filtre === 'toutes' || l.statut !== 'valide') : [],
  );
  const c = $derived(analyse?.compteurs);
  const bloque = $derived(!analyse || analyse.erreurs_globales.length > 0 || !c || c.a_importer === 0);
</script>

<section class="carte">
  <div class="livret-tete">
    <h3>Importer un historique d'opérations</h3>
    <button class="discret" onclick={fermer} aria-label="Fermer">✕</button>
  </div>
  <p>
    Ce fichier contient des opérations (achats, ventes, dividendes, versements, retraits) : chaque ligne est contrôlée
    avant tout enregistrement. Pour un achat ou une vente, le montant est calculé (quantité × prix) et les frais
    s'ajoutent au PRU ; les intérêts des livrets ne sont pas importés, €scarcell les recalcule.
  </p>
  {#if analyse}
    <p class="discret petit">
      {analyse.fichier} · {analyse.encodage} · séparateur {analyse.separateur} · décimales : {analyse.decimale} · {analyse.lignes.length} ligne(s)
    </p>
  {:else if enCours}
    <p class="discret">Lecture du fichier…</p>
  {/if}
  {#if message}<p class="message {message.classe}">{message.texte}</p>{/if}
  <p class="discret petit"><button class="discret" onclick={telechargerModele}>Télécharger le modèle de fichier…</button></p>
</section>

{#if analyse}
  <section class="carte">
    <h3>Colonnes</h3>
    <p class="discret petit">Associez chaque colonne du fichier à un champ d'€scarcell. Date, compte et type sont obligatoires.</p>
    <div class="defilement">
      <table class="colonnes-import">
        <thead><tr><th>Colonne du fichier</th><th>Champ</th></tr></thead>
        <tbody>
          {#each analyse.entetes as entete, i (i)}
            <tr>
              <td>{entete || `(colonne ${i + 1} sans titre)`}</td>
              <td>
                <select value={correspondance?.[i] ?? ''} onchange={(e) => changerColonne(i, e.currentTarget.value)} disabled={enCours || !!rapport}>
                  <option value="">— ignorer —</option>
                  {#each champs as ch (ch.nom)}
                    <option value={ch.nom}>{LIBELLES[ch.nom] ?? ch.nom}{ch.obligatoire ? ' *' : ''}</option>
                  {/each}
                </select>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <div class="ligne-formulaire correspondances">
      {#if enregistrees.length}
        <label>Correspondance enregistrée
          <select onchange={(e) => appliquerEnregistree(e.currentTarget.value)} value="">
            <option value="">Choisir…</option>
            {#each enregistrees as e (e.nom)}<option value={e.nom}>{e.nom}</option>{/each}
          </select>
        </label>
      {/if}
      <label>Nom de cette correspondance<input bind:value={nomCorrespondance} maxlength="60" placeholder="Export de ma banque" /></label>
      <div class="actions">
        <button onclick={enregistrerCorrespondance} disabled={!nomCorrespondance.trim() || analyse.erreurs_globales.length > 0}>Enregistrer</button>
        {#if enregistrees.some((e) => e.nom === nomCorrespondance.trim())}
          <button class="discret" onclick={supprimerCorrespondance}>Supprimer</button>
        {/if}
      </div>
    </div>
  </section>

  {#each analyse.erreurs_globales as e}
    <p class="message alerte">{e}</p>
  {/each}

  {#if c && analyse.lignes.length}
    <section class="carte">
      <h3>Aperçu</h3>
      <div class="kpis compteurs-import">
        <div class="kpi"><span>Valides</span><strong class="positif">{c.valides}</strong></div>
        <div class="kpi"><span>Avertissements</span><strong>{c.avertissements}</strong></div>
        <div class="kpi"><span>Erreurs</span><strong class:negatif={c.erreurs > 0}>{c.erreurs}</strong></div>
        <div class="kpi"><span>Doublons</span><strong>{c.doublons}</strong></div>
        <div class="kpi"><span>Non importées</span><strong>{c.ignorees}</strong></div>
      </div>
      <div class="actions">
        <div class="segments" role="group" aria-label="Lignes affichées">
          <button class:actif={filtre === 'problemes'} onclick={() => (filtre = 'problemes')}>À vérifier</button>
          <button class:actif={filtre === 'toutes'} onclick={() => (filtre = 'toutes')}>Toutes</button>
        </div>
        {#if c.doublons > 0}
          <label class="case-simple">
            <input type="checkbox" checked={inclureDoublons} onchange={(e) => { inclureDoublons = e.currentTarget.checked; analyser(); }} disabled={enCours || !!rapport} />
            Importer aussi les doublons
          </label>
        {/if}
      </div>
      {#if affichees.length === 0}
        <p class="vide">Toutes les lignes sont valides.</p>
      {:else}
        <div class="defilement">
          <table class="apercu-import">
            <thead>
              <tr>
                <th>Ligne</th><th>Statut</th><th>Date</th><th>Compte</th><th>Type</th><th>Titre</th>
                <th class="nombre">Quantité</th><th class="nombre">Prix</th><th class="nombre">Montant</th><th class="nombre">Frais</th><th>Détail</th>
              </tr>
            </thead>
            <tbody>
              {#each affichees.slice(0, limite) as l (l.numero)}
                <tr>
                  <td class="nombre">{l.numero}</td>
                  <td><span class="badge {STATUTS[l.statut].classe}">{STATUTS[l.statut].texte}</span></td>
                  <td>{l.date ?? ''}</td>
                  <td>{l.compte ?? ''}</td>
                  <td>{l.type ?? ''}</td>
                  <td class="titre-import">{l.libelle ?? ''}{#if l.isin}<div class="discret petit">{l.isin}</div>{/if}</td>
                  <td class="nombre">{l.quantite_micro === null ? '' : microVersTexte(l.quantite_micro)}</td>
                  <td class="nombre">{l.prix_micro === null ? '' : microVersTexte(l.prix_micro)}</td>
                  <td class="nombre">{l.montant_cents === null ? '' : euros(l.montant_cents)}</td>
                  <td class="nombre">{l.frais_cents ? euros(l.frais_cents) : ''}</td>
                  <td class="messages-import">
                    {#if l.brut.length}<div class="discret petit">{l.brut.join(' | ')}</div>{/if}
                    {#each l.messages as m}<div class="petit" class:negatif={l.statut === 'erreur'}>{m}</div>{/each}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
        {#if affichees.length > limite}
          <p><button class="discret" onclick={() => (limite += 500)}>Afficher plus ({affichees.length - limite} restantes)</button></p>
        {/if}
      {/if}
      <div class="actions">
        {#if !rapport}
          <button class="principal" onclick={importer} disabled={bloque || enCours}>
            {enCours ? 'Import…' : confirmerImport ? `Confirmer l'import de ${c.a_importer} ligne(s) ?` : `Importer ${c.a_importer} ligne(s)`}
          </button>
          {#if confirmerImport}<button onclick={() => (confirmerImport = false)}>Annuler</button>{/if}
        {/if}
        <button onclick={exporterRapport}>Exporter le rapport (CSV)</button>
      </div>
      {#if !rapport && c.erreurs > 0 && c.a_importer > 0}
        <p class="discret petit">Les lignes en erreur ne seront pas importées. Vous pouvez corriger le fichier puis le choisir à nouveau.</p>
      {/if}
    </section>
  {/if}
{/if}
