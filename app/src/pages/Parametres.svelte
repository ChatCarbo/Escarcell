<script>
  import { onMount } from 'svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { api } from '../lib/api.js';
  import { pct, pctVersMicro } from '../lib/format.js';

  let emplacement = $state('');
  let message = $state(null); // { classe, texte }
  let confirmation = $state('');

  let retenues = $state([]);
  let editionRetenue = $state(null); // { pays, valeur }
  let majAuto = $state(false);
  let maj = $state(null);
  let verification = $state(false);

  onMount(async () => {
    try {
      emplacement = await api.emplacementDonnees();
      retenues = await api.retenues();
      majAuto = await api.verificationAuto(null);
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  });

  async function enregistrerRetenue() {
    const t = pctVersMicro(editionRetenue.valeur);
    if (t === null) return (message = { classe: 'alerte', texte: 'Taux invalide.' });
    try {
      retenues = await api.definirRetenue(editionRetenue.pays, t);
      editionRetenue = null;
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function retenueParDefaut(pays) {
    try {
      retenues = await api.definirRetenue(pays, null);
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function basculerMajAuto() {
    try {
      majAuto = await api.verificationAuto(!majAuto);
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  async function verifier() {
    verification = true;
    try {
      maj = await api.verifierMiseAJour();
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    } finally {
      verification = false;
    }
  }

  const aujourdhui = () => new Date().toISOString().slice(0, 10);
  const horodatage = () => {
    const d = new Date();
    const z = (n) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())}-${z(d.getHours())}${z(d.getMinutes())}`;
  };
  const FILTRES = [{ name: 'Sauvegarde €scarcell', extensions: ['db'] }];
  const FILTRES_CHIFFRES = [{ name: 'Sauvegarde chiffrée €scarcell', extensions: ['escarcell'] }];
  // .escarcelle : sauvegardes chiffrées faites avant le changement de nom, toujours lisibles.
  const FILTRES_TOUS = [{ name: 'Sauvegarde €scarcell', extensions: ['escarcell', 'escarcelle', 'db'] }];
  const LONGUEUR_MIN = 12;

  // Sauvegarde chiffrée
  let mdp = $state('');
  let mdp2 = $state('');
  let compris = $state(false);
  let enCours = $state(false);

  /** Robustesse indicative : longueur et variété des caractères. */
  const robustesse = $derived.by(() => {
    if (!mdp) return null;
    const classes = [/[a-z]/, /[A-Z]/, /[0-9]/, /[^a-zA-Z0-9]/].filter((r) => r.test(mdp)).length;
    const n = [...mdp].length;
    if (n < LONGUEUR_MIN) return { niveau: 'faible', texte: `trop court (${n} / ${LONGUEUR_MIN} caractères minimum)` };
    if (n >= 20 || (n >= 14 && classes >= 3)) return { niveau: 'fort', texte: 'solide' };
    return { niveau: 'moyen', texte: 'correct ; plus long, c\'est mieux (une phrase de 4 ou 5 mots par exemple)' };
  });
  const mdpValide = $derived([...mdp].length >= LONGUEUR_MIN && mdp === mdp2 && compris);

  async function sauvegarderChiffre() {
    message = null;
    const chemin = await save({ defaultPath: `escarcell-${horodatage()}.escarcell`, filters: FILTRES_CHIFFRES });
    if (!chemin) return;
    enCours = true;
    try {
      await api.sauvegarderChiffre(chemin, mdp);
      message = { classe: 'ok', texte: `Sauvegarde chiffrée enregistrée : ${chemin}` };
      mdp = '';
      mdp2 = '';
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    } finally {
      enCours = false;
    }
  }

  async function sauvegarder() {
    message = null;
    const chemin = await save({ defaultPath: `escarcell-${aujourdhui()}.db`, filters: FILTRES });
    if (!chemin) return;
    try {
      await api.sauvegarder(chemin);
      message = { classe: 'ok', texte: `Sauvegarde enregistrée : ${chemin}` };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }

  // Restauration en deux temps : lecture (et déchiffrement) pour afficher la sauvegarde, puis confirmation.
  let restauration = $state(null); // { chemin, chiffree, mdp, info }

  async function choisirRestauration() {
    message = null;
    const chemin = await open({ multiple: false, directory: false, filters: FILTRES_TOUS });
    if (!chemin) return;
    const chiffree = /\.escarcelle?$/.test(String(chemin).toLowerCase());
    restauration = { chemin, chiffree, mdp: '', info: null };
    if (!chiffree) await lireRestauration();
  }

  async function lireRestauration() {
    enCours = true;
    try {
      restauration.info = await api.lireSauvegarde(restauration.chemin, restauration.chiffree ? restauration.mdp : null);
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    } finally {
      enCours = false;
    }
  }

  async function restaurer() {
    enCours = true;
    try {
      await api.restaurer(restauration.chemin, restauration.chiffree ? restauration.mdp : null);
      message = { classe: 'ok', texte: 'Données restaurées. Une copie de sécurité des données précédentes a été faite dans le dossier de données.' };
      restauration = null;
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    } finally {
      enCours = false;
    }
  }

  async function effacer() {
    try {
      await api.toutEffacer(confirmation);
      confirmation = '';
      message = { classe: 'ok', texte: 'Toutes les données ont été effacées.' };
    } catch (e) {
      message = { classe: 'alerte', texte: String(e) };
    }
  }
</script>

<header class="entete"><h1>Paramètres</h1></header>
{#if message}<p class="message {message.classe}">{message.texte}</p>{/if}

<section class="carte">
  <h3>Sauvegarde chiffrée</h3>
  <p>
    Toutes vos données (livrets, lignes du PEA, dividendes, assemblées générales, réglages) dans un seul fichier
    <code>.escarcell</code>, chiffré par un mot de passe : vous pouvez le placer sur une clé USB ou dans un dossier
    synchronisé (OneDrive, Google Drive…) sans que son contenu soit lisible.
  </p>
  <p class="message attention">
    <b>Sans le mot de passe, la sauvegarde est perdue</b> : €scarcell ne le conserve nulle part et personne ne peut le
    retrouver. Notez-le dans un endroit sûr (gestionnaire de mots de passe).
  </p>
  <div class="champs">
  <label>Mot de passe ({LONGUEUR_MIN} caractères minimum)<input type="password" autocomplete="new-password" bind:value={mdp} /></label>
  {#if robustesse}<p class="petit" class:negatif={robustesse.niveau === 'faible'} class:positif={robustesse.niveau === 'fort'}>Robustesse : {robustesse.texte}</p>{/if}
  <label>Mot de passe, une seconde fois<input type="password" autocomplete="new-password" bind:value={mdp2} /></label>
  {#if mdp2 && mdp !== mdp2}<p class="petit negatif">Les deux mots de passe sont différents.</p>{/if}
  <label class="case"><input type="checkbox" bind:checked={compris} /> J'ai compris qu'un mot de passe oublié rend la sauvegarde illisible.</label>
  </div>
  <div class="actions">
    <button class="principal" onclick={sauvegarderChiffre} disabled={!mdpValide || enCours}>{enCours ? 'Chiffrement…' : 'Sauvegarder (chiffré)…'}</button>
  </div>
  <p class="discret petit">
    Chiffrement XChaCha20-Poly1305, clé dérivée du mot de passe par Argon2id. Sauvegarde non chiffrée (fichier .db lisible par tous) :
    <button class="discret" onclick={sauvegarder}>enregistrer une copie non chiffrée…</button>
  </p>
  <p class="discret petit">Fichier de données actuel : {emplacement}</p>
</section>

<section class="carte">
  <h3>Restaurer une sauvegarde</h3>
  <p>Remplace toutes les données actuelles par celles d'une sauvegarde chiffrée (.escarcell, ou .escarcelle pour une sauvegarde faite avant le changement de nom) ou non (.db). Une copie de sécurité des données actuelles est faite automatiquement avant.</p>
  {#if restauration}
    <p class="discret petit">Fichier : {restauration.chemin}</p>
    {#if restauration.chiffree && !restauration.info}
      <div class="champs">
        <label>Mot de passe de la sauvegarde<input type="password" bind:value={restauration.mdp} onkeydown={(e) => e.key === 'Enter' && lireRestauration()} /></label>
      </div>
      <div class="actions">
        <button class="principal" onclick={lireRestauration} disabled={!restauration.mdp || enCours}>{enCours ? 'Déchiffrement…' : 'Ouvrir'}</button>
        <button onclick={() => (restauration = null)}>Annuler</button>
      </div>
    {:else if restauration.info}
      <p class="message attention">
        {#if restauration.info.chiffree}
          Sauvegarde du <b>{restauration.info.cree_le} (UTC)</b>{restauration.info.appareil ? `, faite sur ${restauration.info.appareil}` : ''}, avec €scarcell {restauration.info.version_escarcelle}.
        {:else}
          Sauvegarde non chiffrée.
        {/if}
        Remplacer toutes les données actuelles par cette sauvegarde ?
      </p>
      <div class="actions">
        <button class="principal" onclick={restaurer} disabled={enCours}>Oui, restaurer</button>
        <button onclick={() => (restauration = null)}>Annuler</button>
      </div>
    {:else}
      <button onclick={() => (restauration = null)}>Annuler</button>
    {/if}
  {:else}
    <button onclick={choisirRestauration}>Choisir une sauvegarde…</button>
  {/if}
</section>

<section class="carte">
  <h3>Retenues à la source sur les dividendes</h3>
  <p>
    Taux prélevés par le pays de la société avant le versement. Dans un PEA, cette retenue est perdue. Les retenues proposées sont indicatives :
    vérifiez-les sur vos relevés. 
  </p>
  <table>
    <thead><tr><th>Pays</th><th class="nombre">Retenue</th><th class="nombre">Par défaut</th><th></th></tr></thead>
    <tbody>
      {#each retenues as r (r.pays)}
        <tr>
          <td>{r.nom}</td>
          <td class="nombre">
            {#if editionRetenue?.pays === r.pays}
              <input class="court" bind:value={editionRetenue.valeur} inputmode="decimal" /> %
            {:else}
              {pct(r.taux_micro, 3)}
            {/if}
          </td>
          <td class="nombre discret">{pct(r.defaut_micro, 3)}</td>
          <td class="actions">
            {#if editionRetenue?.pays === r.pays}
              <button class="discret" onclick={enregistrerRetenue}>Enregistrer</button>
              <button class="discret" onclick={() => (editionRetenue = null)}>Annuler</button>
            {:else}
              <button class="discret" onclick={() => (editionRetenue = { pays: r.pays, valeur: (r.taux_micro / 1e4).toLocaleString('fr-FR') })}>Modifier</button>
              {#if r.taux_micro !== r.defaut_micro}<button class="discret" onclick={() => retenueParDefaut(r.pays)}>Rétablir</button>{/if}
            {/if}
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</section>

<section class="carte">
  <h3>Nouvelles versions</h3>
  <div class="champs" style="max-width: none"><label class="case"><input type="checkbox" checked={majAuto} onchange={basculerMajAuto} /> Vérifier au démarrage si une nouvelle version est publiée sur GitHub (désactivé par défaut)</label></div>
  <p><button onclick={verifier} disabled={verification}>{verification ? 'Vérification…' : 'Vérifier maintenant'}</button></p>
  {#if maj}
    {#if maj.disponible}
      <p class="message ok">€scarcell {maj.derniere_version} est disponible (vous avez la {maj.version_actuelle}).
        Téléchargez-la depuis la page <a href={maj.url} target="_blank" rel="noreferrer">de la version</a>.</p>
    {:else}
      <p class="message">Vous avez la dernière version ({maj.version_actuelle}).</p>
    {/if}
  {/if}
  <p class="discret petit">
    La vérification envoie une requête à api.github.com : GitHub voit votre adresse IP, comme pour toute page web, mais aucune
    donnée d'€scarcell n'est transmise. Rien n'est téléchargé ni installé automatiquement.
  </p>
</section>

<section class="carte zone-danger">
  <h3>Tout effacer</h3>
  <p>Supprime définitivement tous les livrets, virements, lignes du PEA, cours, dividendes, AG et réglages. Cette action est irréversible : faites une sauvegarde avant.</p>
  <div class="champs"><label>Tapez EFFACER pour confirmer<input bind:value={confirmation} placeholder="EFFACER" /></label></div>
  <button class="danger" disabled={confirmation.trim() !== 'EFFACER'} onclick={effacer}>Tout effacer</button>
</section>
