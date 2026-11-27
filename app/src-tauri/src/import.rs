//! Import CSV des opérations du PEA et des livrets (0.4, lot 2).
//!
//! Parcours : analyse du fichier (encodage, séparateurs, correspondance des colonnes, aperçu ligne par
//! ligne), puis import des seules lignes valides en une transaction, après une copie de sécurité de la base.
//! La copie de sécurité se restaure dans Paramètres. Le fichier est relu et contrôlé à nouveau au moment de l'import :
//! l'interface ne transmet jamais d'opérations, seulement le chemin et la correspondance des colonnes.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use escarcell_core::import_csv::{
    analyser_ligne, decoder, detecter_decimale, detecter_separateur, lire_csv, modele, proposer_correspondance,
    valeurs_numeriques, verifier_correspondance, Champ, CompteImport, Decimale, Enregistrement, LigneImport,
    TypeImport,
};
use escarcell_core::import_positions::{est_html, lire_html, reconnaitre, Genre};
use escarcell_core::journal::{rejouer, ErreurJournal, MouvementTitre, SensTitre};
use escarcell_core::livret::{Anomalie, Mouvement};
use escarcell_core::{Date, Money, Price};
use serde::Serialize;
use tauri::State;

use crate::commands::aujourd_hui;
use crate::livrets::{anomalies, date_fr, decrire, params, simulation};
use crate::store::{Compte, LignePeaCalculee, Operation, PlanImport, Position, Store};
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Taille maximale d'un fichier importé.
const TAILLE_MAX: u64 = 20 * 1024 * 1024;
/// Nombre maximal de lignes d'opérations.
const LIGNES_MAX: usize = 100_000;
/// Préfixe des correspondances de colonnes enregistrées dans les paramètres.
const CORRESPONDANCE: &str = "import.correspondance.";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Statut {
    Valide,
    Avertissement,
    /// Intérêts d'un livret : comparés au calcul, jamais importés.
    Ignoree,
    Doublon,
    Erreur,
}

#[derive(Clone, Debug, Serialize)]
pub struct LigneDto {
    /// Numéro de la ligne dans le fichier (l'en-tête est la ligne 1).
    pub numero: usize,
    pub statut: Statut,
    pub messages: Vec<String>,
    /// Ligne retenue pour l'import (valide, avec avertissement, ou doublon importé sur demande).
    pub a_importer: bool,
    pub date: Option<String>,
    pub compte: Option<String>,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub isin: Option<String>,
    pub libelle: Option<String>,
    pub quantite_micro: Option<i64>,
    pub prix_micro: Option<i64>,
    pub montant_cents: Option<i64>,
    pub frais_cents: Option<i64>,
    /// Champs bruts du fichier, pour les lignes illisibles.
    pub brut: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Compteurs {
    pub valides: usize,
    pub avertissements: usize,
    pub erreurs: usize,
    pub doublons: usize,
    pub ignorees: usize,
    pub a_importer: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct AnalyseDto {
    pub fichier: String,
    pub encodage: String,
    pub separateur: String,
    pub decimale: String,
    pub entetes: Vec<String>,
    /// Champ associé à chaque colonne (nom du format de référence), ou `null`.
    pub correspondance: Vec<Option<String>>,
    /// Problèmes qui empêchent tout import (correspondance incomplète, opération existante rendue impossible).
    pub erreurs_globales: Vec<String>,
    pub compteurs: Compteurs,
    pub lignes: Vec<LigneDto>,
}

#[derive(Serialize)]
pub struct ChampDto {
    pub nom: &'static str,
    pub obligatoire: bool,
}

/// Champs du format de référence, pour la correspondance des colonnes.
#[tauri::command]
pub fn champs_import() -> Vec<ChampDto> {
    Champ::TOUS.iter().map(|c| ChampDto { nom: c.nom(), obligatoire: c.obligatoire() }).collect()
}

fn lire_fichier(chemin: &str) -> Resultat<Vec<u8>> {
    let meta = std::fs::metadata(chemin).map_err(|e| format!("Fichier illisible : {e}"))?;
    if meta.len() > TAILLE_MAX {
        return Err("Le fichier dépasse 20 Mo.".into());
    }
    std::fs::read(chemin).map_err(|e| format!("Fichier illisible : {e}"))
}

pub fn nom_fichier(chemin: &str) -> String {
    std::path::Path::new(chemin).file_name().and_then(|n| n.to_str()).unwrap_or(chemin).chars().take(200).collect()
}

/// Fichier lu sous forme de tableau (CSV, tableau HTML ou classeur).
pub struct Tableau {
    pub lignes: Vec<Enregistrement>,
    pub encodage: String,
    /// Séparateur de champs (pour deviner le séparateur décimal) ; `;` pour un tableau HTML ou un classeur.
    pub sep: char,
    pub separateur: String,
}

/// Fichier texte : tableau HTML (exports « Excel » de certaines banques) ou CSV.
pub fn tableau_texte(octets: &[u8]) -> Tableau {
    let (texte, encodage) = decoder(octets);
    if est_html(&texte) {
        return Tableau {
            lignes: lire_html(&texte),
            encodage: encodage.nom().into(),
            sep: ';',
            separateur: "tableau HTML".into(),
        };
    }
    let sep = detecter_separateur(&texte);
    Tableau {
        lignes: lire_csv(&texte, sep),
        encodage: encodage.nom().into(),
        sep,
        separateur: match sep {
            '\t' => "tabulation".into(),
            c => c.to_string(),
        },
    }
}

/// Cellule d'un classeur, en texte. Les nombres sont écrits avec une virgule décimale (au plus 6 décimales).
fn cellule(c: &calamine::Data) -> String {
    use calamine::Data;
    let nombre = |f: f64| {
        let s = format!("{f:.6}");
        let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
        s.replace('.', ",")
    };
    match c {
        Data::Empty | Data::Error(_) => String::new(),
        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => nombre(*f),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(d) => {
            escarcell_core::import_positions::date_excel(d.as_f64()).map(|d| d.to_string()).unwrap_or_default()
        }
        // Variante ajoutée par une version future de calamine : cellule ignorée.
        #[allow(unreachable_patterns)]
        _ => String::new(),
    }
}

/// Classeur Excel (.xlsx, .xls) ou OpenDocument (.ods) : première feuille dont l'en-tête est reconnu.
fn lire_classeur(chemin: &str) -> Resultat<Tableau> {
    use calamine::Reader;
    let mut classeur = calamine::open_workbook_auto(chemin).map_err(|e| format!("Classeur illisible : {e}"))?;
    let mut feuilles: Vec<Vec<Enregistrement>> = classeur
        .worksheets()
        .into_iter()
        .map(|(_, plage)| {
            let premiere = plage.start().map_or(0, |(l, _)| l as usize);
            plage
                .rows()
                .enumerate()
                .map(|(i, r)| Enregistrement { ligne: premiere + i + 1, champs: r.iter().map(cellule).collect() })
                .filter(|e| e.champs.iter().any(|c| !c.trim().is_empty()))
                .collect()
        })
        .collect();
    if feuilles.is_empty() {
        return Err("Le classeur ne contient aucune feuille.".into());
    }
    let i = feuilles.iter().position(|f| reconnaitre(f).is_some()).unwrap_or(0);
    Ok(Tableau {
        lignes: feuilles.swap_remove(i),
        encodage: "classeur".into(),
        sep: ';',
        separateur: "classeur".into(),
    })
}

/// Lit un fichier d'import : classeur (reconnu à son contenu, pas à son extension) ou texte.
pub fn lire_tableau(chemin: &str) -> Resultat<Tableau> {
    let octets = lire_fichier(chemin)?;
    if octets.starts_with(b"PK\x03\x04") || octets.starts_with(&[0xD0, 0xCF, 0x11, 0xE0]) {
        return lire_classeur(chemin);
    }
    Ok(tableau_texte(&octets))
}

/// Ligne analysée, avec son état de travail.
struct Travail {
    numero: usize,
    brut: Vec<String>,
    ligne: Option<LigneImport>,
    erreurs: Vec<String>,
    avertissements: Vec<String>,
    infos: Vec<String>,
    doublon: bool,
}

impl Travail {
    fn candidate(&self, inclure_doublons: bool) -> bool {
        self.erreurs.is_empty()
            && (!self.doublon || inclure_doublons)
            && self.ligne.as_ref().is_some_and(|l| l.type_ != TypeImport::Interets)
    }
}

/// Opération de la base correspondant à une ligne du fichier.
fn operation(l: &LigneImport) -> Operation {
    Operation {
        id: 0,
        compte_id: 0,
        date: l.date.to_string(),
        type_: l.type_.code().unwrap_or("").to_string(),
        montant_cents: Some(l.montant.cents()),
        isin: l.isin.clone(),
        quantite_micro: l.quantite_micro,
        cours_micro: l.prix.map(Price::micro),
        frais_cents: l.frais.cents(),
        valeur_plan_cents: None,
        libelle: l.libelle.clone(),
    }
}

type CleDoublon = (String, String, String, Option<String>, Option<i64>, Option<i64>);

/// Clé de doublon : compte, date, type, ISIN, quantité et montant.
fn cle(type_compte: &str, o: &Operation) -> CleDoublon {
    (type_compte.into(), o.date.clone(), o.type_.clone(), o.isin.clone(), o.quantite_micro, o.montant_cents)
}

fn mouvement_titre(o: &Operation) -> Option<MouvementTitre> {
    let sens = match o.type_.as_str() {
        "ACHAT" => SensTitre::Achat,
        "VENTE" => SensTitre::Vente,
        _ => return None,
    };
    Some(MouvementTitre {
        date: o.date.parse().ok()?,
        isin: o.isin.clone()?,
        sens,
        quantite_micro: o.quantite_micro?,
        prix: Price::from_micro(o.cours_micro?),
        frais: Money::from_cents(o.frais_cents),
    })
}

fn quantite_fr(q: i64) -> String {
    let entier = q / 1_000_000;
    let reste = q % 1_000_000;
    if reste == 0 {
        entier.to_string()
    } else {
        format!("{entier},{}", format!("{reste:06}").trim_end_matches('0'))
    }
}

/// Résultat complet d'une analyse : ce qui est montré et ce qui serait écrit.
struct Analyse {
    dto: AnalyseDto,
    plan: PlanImport,
    /// ISIN des nouvelles lignes du PEA (sans ligne existante), dont il faut trouver le symbole de cotation.
    nouveaux_isins: Vec<String>,
}

/// Analyse un fichier sans rien écrire. `symboles` donne le symbole et le nom des nouvelles lignes du PEA
/// (sinon l'ISIN sert de symbole et le libellé du fichier de nom).
#[cfg(test)]
fn analyser(
    db: &Store,
    octets: &[u8],
    fichier: &str,
    correspondance: Option<Vec<Option<String>>>,
    inclure_doublons: bool,
    jour: Date,
    symboles: &HashMap<String, (String, Option<String>)>,
) -> Resultat<Analyse> {
    analyser_tableau(db, &tableau_texte(octets), fichier, correspondance, inclure_doublons, jour, symboles)
}

fn analyser_tableau(
    db: &Store,
    tableau: &Tableau,
    fichier: &str,
    correspondance: Option<Vec<Option<String>>>,
    inclure_doublons: bool,
    jour: Date,
    symboles: &HashMap<String, (String, Option<String>)>,
) -> Resultat<Analyse> {
    let sep = tableau.sep;
    // L'en-tête peut être précédé de lignes de titre (exports de banque).
    let debut = match reconnaitre(&tableau.lignes) {
        Some((i, Genre::Operations)) => i,
        _ => 0,
    };
    let mut enregistrements: Vec<Enregistrement> = tableau.lignes[debut.min(tableau.lignes.len())..].to_vec();
    if enregistrements.is_empty() {
        return Err("Le fichier est vide.".into());
    }
    let entete = enregistrements.remove(0);
    if enregistrements.len() > LIGNES_MAX {
        return Err(format!("Le fichier contient plus de {LIGNES_MAX} lignes : découpez-le."));
    }
    let entetes: Vec<String> = entete.champs.iter().map(|e| e.trim().to_string()).collect();
    let corr: Vec<Option<Champ>> = match correspondance {
        Some(c) => {
            if c.len() != entetes.len() {
                return Err("La correspondance ne correspond pas aux colonnes du fichier : relancez l'analyse.".into());
            }
            c.iter()
                .map(|n| match n.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                    None => Ok(None),
                    Some(n) => Champ::depuis_nom(n).map(Some).ok_or_else(|| format!("champ inconnu : {n}")),
                })
                .collect::<Resultat<_>>()?
        }
        None => proposer_correspondance(&entetes),
    };
    let dec = detecter_decimale(valeurs_numeriques(&enregistrements, &corr), sep);
    let mut dto = AnalyseDto {
        fichier: fichier.into(),
        encodage: tableau.encodage.clone(),
        separateur: tableau.separateur.clone(),
        decimale: match dec {
            Decimale::Virgule => "virgule".into(),
            Decimale::Point => "point".into(),
        },
        entetes,
        correspondance: corr.iter().map(|c| c.map(|c| c.nom().to_string())).collect(),
        erreurs_globales: Vec::new(),
        compteurs: Compteurs::default(),
        lignes: Vec::new(),
    };
    if let Err(e) = verifier_correspondance(&corr) {
        dto.erreurs_globales.push(e);
        return Ok(Analyse { dto, plan: PlanImport::default(), nouveaux_isins: Vec::new() });
    }

    // 1. Contrôle de chaque ligne prise isolément.
    let mut t: Vec<Travail> = enregistrements
        .iter()
        .map(|e| {
            let a = analyser_ligne(&e.champs, &corr, dec);
            let mut w = Travail {
                numero: e.ligne,
                brut: e.champs.clone(),
                ligne: a.ligne,
                erreurs: a.erreurs,
                avertissements: a.avertissements,
                infos: Vec::new(),
                doublon: false,
            };
            if w.ligne.as_ref().is_some_and(|l| l.date > jour) {
                w.erreurs.push("date dans le futur".into());
            }
            w
        })
        .collect();

    // 2. Doublons : même compte, date, type, ISIN, quantité et montant qu'une opération déjà enregistrée.
    // Comptage : deux lignes identiques du fichier face à une seule opération enregistrée → un seul doublon.
    let comptes = db.comptes().map_err(err)?;
    let mut existantes: HashMap<String, Vec<Operation>> = HashMap::new();
    for c in &comptes {
        if ["PEA", "LIVRET_A", "LDDS", "LEP"].contains(&c.type_.as_str()) && !existantes.contains_key(&c.type_) {
            existantes.insert(c.type_.clone(), db.operations(c.id).map_err(err)?);
        }
    }
    let mut deja: HashMap<CleDoublon, usize> = HashMap::new();
    for (tc, ops) in &existantes {
        for o in ops {
            *deja.entry(cle(tc, o)).or_default() += 1;
        }
    }
    for w in t.iter_mut().filter(|w| w.erreurs.is_empty()) {
        let Some(l) = &w.ligne else { continue };
        if l.type_ == TypeImport::Interets {
            continue;
        }
        if let Some(n) = deja.get_mut(&cle(l.compte.code(), &operation(l))) {
            if *n > 0 {
                *n -= 1;
                w.doublon = true;
                w.infos.push(if inclure_doublons {
                    "déjà enregistrée : importée quand même, sur votre demande".into()
                } else {
                    "déjà enregistrée : ignorée".into()
                });
            }
        }
    }

    // 3. Livrets : plafond, solde et solde de départ, contrôlés à la date de chaque opération avec tout
    // l'historique (opérations enregistrées et lignes retenues). Une ligne refusée peut en rendre une autre
    // impossible (retrait après un versement refusé) : on recommence jusqu'à ce que plus rien ne change.
    for compte in [CompteImport::LivretA, CompteImport::Ldds, CompteImport::Lep] {
        let tc = compte.code();
        let p = params(tc);
        let avant = existantes.get(tc).cloned().unwrap_or_default();
        let du_livret = |w: &Travail| w.ligne.as_ref().is_some_and(|l| l.compte == compte);
        for w in t.iter_mut().filter(|w| du_livret(w) && w.erreurs.is_empty()) {
            let l = w.ligne.as_ref().expect("ligne lue");
            if l.type_ == TypeImport::Solde && l.montant.cents() > p.plafond.cents() / 2 * 3 {
                w.erreurs.push(format!(
                    "un solde de départ ne peut dépasser le plafond ({}) que des intérêts capitalisés : au-delà de {}, vérifiez le montant",
                    p.plafond,
                    Money::from_cents(p.plafond.cents() / 2 * 3)
                ));
            }
        }
        let anomalies_avant: BTreeSet<i64> = anomalies(tc, &avant)?.into_iter().map(|(id, _, _)| id).collect();
        // Identifiant d'une ligne du fichier dans le contrôle : négatif (−1 − indice), jamais celui d'une opération.
        let historique = |t: &[Travail]| -> Vec<Operation> {
            let mut ops = avant.clone();
            for (i, w) in t.iter().enumerate() {
                if du_livret(w) && w.candidate(inclure_doublons) {
                    ops.push(Operation { id: -1 - i as i64, ..operation(w.ligne.as_ref().expect("ligne lue")) });
                }
            }
            ops
        };
        // Seules les anomalies du premier jour concerné sont sûres : les suivantes peuvent n'être que la
        // conséquence d'une ligne refusée (retrait après un versement refusé). On refuse donc jour après jour,
        // en recalculant à chaque fois ; au-delà de 500 jours en anomalie, le reste est refusé d'un coup.
        let mut finales = Vec::new();
        for tour in 0.. {
            let ops = historique(&t);
            let trouvees = anomalies(tc, &ops)?;
            let bloquantes: Vec<&(i64, Mouvement, Anomalie)> =
                trouvees.iter().filter(|(id, _, a)| *id < 0 && !matches!(a, Anomalie::PlafondDepasse { .. })).collect();
            let Some(premier) = bloquantes.iter().map(|(_, m, _)| m.date).min() else {
                finales = trouvees;
                break;
            };
            for (id, m, a) in bloquantes {
                if m.date == premier || tour >= 500 {
                    t[(-1 - id) as usize].erreurs.push(decrire(a, p.plafond, m.date));
                }
            }
        }
        for (id, m, a) in finales {
            if id < 0 {
                // Seul le plafond reste : avertissement (relevé réel qui le dépasse, intérêts compris).
                t[(-1 - id) as usize].avertissements.push(decrire(&a, p.plafond, m.date));
            } else if !anomalies_avant.contains(&id) {
                dto.erreurs_globales.push(format!(
                    "{} : avec ce fichier, l'opération déjà enregistrée du {} ({}) ne serait plus possible : {}.",
                    compte.libelle(),
                    date_fr(m.date),
                    m.montant,
                    decrire(&a, p.plafond, m.date)
                ));
            }
        }

        // Intérêts du fichier : comparés au calcul d'€scarcell, année par année, jamais importés.
        let lignes_interets: Vec<usize> = t
            .iter()
            .enumerate()
            .filter(|(_, w)| du_livret(w) && w.erreurs.is_empty())
            .filter(|(_, w)| w.ligne.as_ref().is_some_and(|l| l.type_ == TypeImport::Interets))
            .map(|(i, _)| i)
            .collect();
        if lignes_interets.is_empty() {
            continue;
        }
        let ops = historique(&t);
        let (_, annees) = simulation(tc, &ops, jour)?;
        let depart = ops.iter().filter(|o| o.type_ == "VALORISATION").filter_map(|o| o.date.parse::<Date>().ok()).min();
        // Intérêts crédités au 31 décembre ou dans les premiers jours de janvier : année précédente.
        let annee_de = |d: Date| if d.month() == 1 && d.day() <= 15 { d.year() - 1 } else { d.year() };
        let mut fichier_par_annee: BTreeMap<i32, Money> = BTreeMap::new();
        for &i in &lignes_interets {
            let l = t[i].ligne.as_ref().expect("ligne lue");
            *fichier_par_annee.entry(annee_de(l.date)).or_insert(Money::ZERO) += l.montant;
        }
        for &i in &lignes_interets {
            let l = t[i].ligne.as_ref().expect("ligne lue");
            let annee = annee_de(l.date);
            let message = if annee >= jour.year() {
                format!("intérêts {annee} : année non terminée, non comparés")
            } else if depart.is_some_and(|d| d > Date::new(annee, 1, 1).expect("1er janvier")) {
                format!("intérêts {annee} : l'historique commence par un solde de départ en cours d'année, comparaison impossible")
            } else {
                let calcule = annees.iter().find(|a| a.annee == annee).map(|a| a.interets).unwrap_or(Money::ZERO);
                let fichier = fichier_par_annee[&annee];
                if calcule == fichier {
                    format!("intérêts {annee} conformes au calcul d'€scarcell ({calcule}) ; non importés, ils sont recalculés")
                } else {
                    format!(
                        "intérêts {annee} : €scarcell calcule {calcule}, le fichier indique {fichier} ; vérifiez les opérations de l'année (non importés, ils sont recalculés)"
                    )
                }
            };
            t[i].infos.push(message);
        }
    }

    // 4. PEA : ventes supérieures à la quantité détenue, lignes saisies à la main remplacées par le calcul.
    let positions = db.positions().map_err(err)?;
    let compte_pea = comptes.iter().find(|c| c.type_ == "PEA");
    let ops_pea = existantes.get("PEA").cloned().unwrap_or_default();
    let mut journal: Vec<MouvementTitre> = ops_pea.iter().filter_map(mouvement_titre).collect();
    let nb_existants = journal.len();
    let mut indice_ligne = Vec::new();
    for (i, w) in t.iter().enumerate() {
        if w.candidate(inclure_doublons) && w.ligne.as_ref().is_some_and(|l| l.compte == CompteImport::Pea) {
            if let Some(m) = mouvement_titre(&operation(w.ligne.as_ref().expect("ligne lue"))) {
                journal.push(m);
                indice_ligne.push(i);
            }
        }
    }
    let rejeu = rejouer(&journal);
    for (k, e) in &rejeu.erreurs {
        if *k < nb_existants {
            dto.erreurs_globales.push(format!(
                "Le journal du PEA déjà enregistré est incohérent (vente du {} supérieure à la quantité détenue).",
                date_fr(journal[*k].date)
            ));
            continue;
        }
        let w = &mut t[indice_ligne[*k - nb_existants]];
        w.erreurs.push(match e {
            ErreurJournal::VenteSuperieure { detenue_micro } => format!(
                "vente de {} titres supérieure aux {} détenus à cette date",
                quantite_fr(journal[*k].quantite_micro),
                quantite_fr(*detenue_micro)
            ),
            ErreurJournal::Invalide => "quantité, prix ou frais invalides".into(),
        });
    }
    let mut touches: BTreeSet<String> = BTreeSet::new();
    let mut deja_averti: BTreeSet<String> = BTreeSet::new();
    for w in t.iter_mut() {
        if !w.candidate(inclure_doublons) {
            continue;
        }
        let Some(l) = &w.ligne else { continue };
        if l.compte != CompteImport::Pea {
            continue;
        }
        if let Some(c) = compte_pea {
            if l.date.to_string() < c.ouverture {
                w.avertissements.push(format!(
                    "opération antérieure à l'ouverture du PEA ({})",
                    c.ouverture.parse::<Date>().map(date_fr).unwrap_or_else(|_| c.ouverture.clone())
                ));
            }
        }
        if !matches!(l.type_, TypeImport::Achat | TypeImport::Vente) {
            continue;
        }
        let isin = l.isin.clone().expect("ISIN contrôlé");
        let manuelles: Vec<&Position> =
            positions.iter().filter(|p| !p.journal && p.isin.as_deref() == Some(isin.as_str())).collect();
        if !manuelles.is_empty() && deja_averti.insert(isin.clone()) {
            for p in manuelles {
                w.avertissements.push(format!(
                    "remplace la ligne « {} » saisie à la main ({} titres, PRU {} €) par le calcul des opérations",
                    p.nom,
                    quantite_fr(p.quantite_micro),
                    Price::from_micro(p.pru_micro)
                ));
            }
        }
        touches.insert(isin);
    }

    // 5. Statuts, compteurs et plan d'écriture.
    let mut plan = PlanImport { fichier: fichier.into(), ..PlanImport::default() };
    let mut ouverture: BTreeMap<&'static str, Date> = BTreeMap::new();
    let mut nom_par_isin: HashMap<String, String> = HashMap::new();
    for w in &t {
        let a_importer = w.candidate(inclure_doublons);
        let statut = if !w.erreurs.is_empty() {
            Statut::Erreur
        } else if w.doublon {
            Statut::Doublon
        } else if w.ligne.as_ref().is_some_and(|l| l.type_ == TypeImport::Interets) {
            Statut::Ignoree
        } else if !w.avertissements.is_empty() {
            Statut::Avertissement
        } else {
            Statut::Valide
        };
        let c = &mut dto.compteurs;
        match statut {
            Statut::Valide => c.valides += 1,
            Statut::Avertissement => c.avertissements += 1,
            Statut::Erreur => c.erreurs += 1,
            Statut::Doublon => c.doublons += 1,
            Statut::Ignoree => c.ignorees += 1,
        }
        if a_importer {
            c.a_importer += 1;
            let l = w.ligne.as_ref().expect("ligne lue");
            let o = ouverture.entry(l.compte.code()).or_insert(l.date);
            *o = (*o).min(l.date);
            if let (Some(isin), Some(lib)) = (&l.isin, &l.libelle) {
                nom_par_isin.entry(isin.clone()).or_insert_with(|| lib.clone());
            }
            plan.operations.push((l.compte.code().to_string(), operation(l)));
        }
        let l = w.ligne.as_ref();
        dto.lignes.push(LigneDto {
            numero: w.numero,
            statut,
            messages: w.erreurs.iter().chain(&w.avertissements).chain(&w.infos).cloned().collect(),
            a_importer,
            date: l.map(|l| l.date.to_string()),
            compte: l.map(|l| l.compte.libelle().to_string()),
            type_: l.map(|l| l.type_.nom().to_string()),
            isin: l.and_then(|l| l.isin.clone()),
            libelle: l.and_then(|l| l.libelle.clone()),
            quantite_micro: l.and_then(|l| l.quantite_micro),
            prix_micro: l.and_then(|l| l.prix.map(Price::micro)),
            montant_cents: l.map(|l| l.montant.cents()),
            frais_cents: l.map(|l| l.frais.cents()),
            brut: if l.is_none() { w.brut.iter().take(20).cloned().collect() } else { Vec::new() },
        });
    }
    plan.lignes = plan.operations.len() as i64;
    for (type_, date) in ouverture {
        if !comptes.iter().any(|c| c.type_ == type_) {
            let nom = match type_ {
                "LIVRET_A" => "Livret A",
                t => t,
            };
            plan.comptes.push(Compte {
                id: 0,
                nom: nom.into(),
                type_: type_.into(),
                etablissement: String::new(),
                produit: None,
                ouverture: date.to_string(),
            });
        }
    }
    let mut nouveaux_isins = Vec::new();
    for isin in touches {
        let l = rejeu.lignes.get(&isin).copied().unwrap_or_default();
        let existante = positions.iter().any(|p| p.isin.as_deref() == Some(isin.as_str()));
        if !existante {
            nouveaux_isins.push(isin.clone());
        }
        let (symbole, nom) = match symboles.get(&isin) {
            Some((s, n)) => (s.clone(), n.clone().or_else(|| nom_par_isin.get(&isin).cloned())),
            None => (isin.clone(), nom_par_isin.get(&isin).cloned()),
        };
        plan.lignes_pea.push(LignePeaCalculee {
            isin: isin.clone(),
            quantite_micro: l.quantite_micro,
            pru_micro: l.pru().micro(),
            symbole,
            nom: nom.unwrap_or(isin),
        });
    }
    Ok(Analyse { dto, plan, nouveaux_isins })
}

/// Analyse un fichier et rend l'aperçu ligne par ligne, sans rien enregistrer.
/// `correspondance` : champ de chaque colonne (`None` pour la proposition automatique).
#[tauri::command]
pub fn analyser_import(
    etat: State<'_, Etat>,
    chemin: String,
    correspondance: Option<Vec<Option<String>>>,
    inclure_doublons: bool,
) -> Resultat<AnalyseDto> {
    let tableau = lire_tableau(&chemin)?;
    let db = etat.db();
    Ok(analyser_tableau(
        &db,
        &tableau,
        &nom_fichier(&chemin),
        correspondance,
        inclure_doublons,
        aujourd_hui(),
        &HashMap::new(),
    )?
    .dto)
}

#[derive(Serialize)]
pub struct RapportDto {
    pub import_id: i64,
    /// Copie de sécurité faite juste avant l'import.
    pub sauvegarde: String,
    pub analyse: AnalyseDto,
}

/// Importe les lignes valides (et les doublons si demandé) en une seule transaction.
/// Le fichier est relu et contrôlé à nouveau ; un problème global empêche tout import.
#[tauri::command]
pub async fn importer(
    etat: State<'_, Etat>,
    chemin: String,
    correspondance: Option<Vec<Option<String>>>,
    inclure_doublons: bool,
) -> Resultat<RapportDto> {
    let tableau = lire_tableau(&chemin)?;
    let fichier = nom_fichier(&chemin);
    let a_resoudre = {
        let db = etat.db();
        analyser_tableau(
            &db,
            &tableau,
            &fichier,
            correspondance.clone(),
            inclure_doublons,
            aujourd_hui(),
            &HashMap::new(),
        )?
        .nouveaux_isins
    };
    // Symbole de cotation des nouvelles lignes (réseau, hors du verrou de la base). Facultatif : à défaut,
    // l'ISIN sert de symbole et le cours peut être saisi à la main.
    let mut symboles = HashMap::new();
    for isin in a_resoudre.into_iter().take(200) {
        if let Ok(Some((s, n))) = crate::cours::rechercher(&etat.http, &isin).await {
            if crate::cours::symbole_valide(&s) {
                symboles.insert(isin, (s, n));
            }
        }
    }
    let mut db = etat.db();
    let a = analyser_tableau(&db, &tableau, &fichier, correspondance, inclure_doublons, aujourd_hui(), &symboles)?;
    if !a.dto.erreurs_globales.is_empty() {
        return Err(format!("Import impossible : {}", a.dto.erreurs_globales.join(" ")));
    }
    if a.plan.operations.is_empty() {
        return Err("Aucune ligne à importer.".into());
    }
    let copie = crate::reglages::copie_de_securite(&db, &etat.chemin_db, "avant-import")?;
    let import_id = db.importer(&a.plan).map_err(err)?;
    Ok(RapportDto { import_id, sauvegarde: copie.to_string_lossy().into_owned(), analyse: a.dto })
}

// ---------- Correspondances enregistrées et modèle ----------

#[derive(Serialize)]
pub struct CorrespondanceDto {
    pub nom: String,
    /// Intitulés de colonnes du fichier et champ associé (format de référence).
    pub colonnes: Vec<(String, Option<String>)>,
}

/// Encodage d'une correspondance : une ligne « intitulé<TAB>champ » par colonne.
fn encoder(colonnes: &[(String, Option<String>)]) -> String {
    colonnes
        .iter()
        .map(|(e, c)| format!("{}\t{}", e.replace(['\t', '\n', '\r'], " "), c.as_deref().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn decoder_correspondance(v: &str) -> Vec<(String, Option<String>)> {
    v.lines()
        .map(|l| {
            let (e, c) = l.split_once('\t').unwrap_or((l, ""));
            (e.to_string(), Champ::depuis_nom(c.trim()).map(|c| c.nom().to_string()))
        })
        .collect()
}

#[tauri::command]
pub fn correspondances(etat: State<'_, Etat>) -> Resultat<Vec<CorrespondanceDto>> {
    Ok(etat
        .db()
        .parametres(CORRESPONDANCE)
        .map_err(err)?
        .into_iter()
        .map(|(cle, v)| CorrespondanceDto {
            nom: cle[CORRESPONDANCE.len()..].to_string(),
            colonnes: decoder_correspondance(&v),
        })
        .collect())
}

/// Enregistre (ou remplace) une correspondance de colonnes sous un nom.
#[tauri::command]
pub fn enregistrer_correspondance(
    etat: State<'_, Etat>,
    nom: String,
    colonnes: Vec<(String, Option<String>)>,
) -> Resultat<Vec<CorrespondanceDto>> {
    let nom = nom.trim();
    if nom.is_empty() || nom.chars().count() > 60 {
        return Err("Donnez un nom de 1 à 60 caractères.".into());
    }
    let champs: Vec<Option<Champ>> = colonnes.iter().map(|(_, c)| c.as_deref().and_then(Champ::depuis_nom)).collect();
    verifier_correspondance(&champs)?;
    etat.db().definir_parametre(&format!("{CORRESPONDANCE}{nom}"), &encoder(&colonnes)).map_err(err)?;
    correspondances(etat)
}

#[tauri::command]
pub fn supprimer_correspondance(etat: State<'_, Etat>, nom: String) -> Resultat<Vec<CorrespondanceDto>> {
    etat.db().supprimer_parametre(&format!("{CORRESPONDANCE}{}", nom.trim())).map_err(err)?;
    correspondances(etat)
}

/// Enregistre le modèle CSV (UTF-8 avec marque d'ordre, lisible par Excel).
#[tauri::command]
pub fn enregistrer_modele(chemin: String) -> Resultat<()> {
    if !chemin.to_ascii_lowercase().ends_with(".csv") {
        return Err("Le modèle doit être un fichier .csv.".into());
    }
    let mut octets = vec![0xEF, 0xBB, 0xBF];
    octets.extend_from_slice(modele().as_bytes());
    std::fs::write(&chemin, octets).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jour() -> Date {
        "2026-11-26".parse().unwrap()
    }

    fn analyse(db: &Store, csv: &str, doublons: bool) -> Analyse {
        analyser(db, csv.as_bytes(), "test.csv", None, doublons, jour(), &HashMap::new()).unwrap()
    }

    fn statuts(a: &Analyse) -> Vec<Statut> {
        a.dto.lignes.iter().map(|l| l.statut).collect()
    }

    const ENTETE: &str = "date;compte;type;isin;libelle;quantite;prix_unitaire;montant;frais;devise;note\n";

    #[test]
    fn import_complet_puis_doublons() {
        let mut db = Store::en_memoire().unwrap();
        let csv = format!(
            "{ENTETE}2025-01-02;PEA;versement;;;;;5000;;;\n\
             2025-01-06;PEA;achat;FR001400U5Q4;Amundi PEA Monde;100;45,1234;;1,99;;\n\
             2025-02-06;PEA;achat;FR001400U5Q4;Amundi PEA Monde;10;50;;1,99;;\n\
             2025-09-15;PEA;vente;FR001400U5Q4;;20;52,10;;1,99;;\n\
             2025-01-01;Livret A;solde;;;;;10000;;;\n\
             2025-03-10;Livret A;versement;;;;;1500;;;\n\
             2025-12-31;Livret A;interets;;;;;1;;;\n"
        );
        let a = analyse(&db, &csv, false);
        assert!(a.dto.erreurs_globales.is_empty());
        assert_eq!(
            statuts(&a),
            vec![
                Statut::Valide,
                Statut::Valide,
                Statut::Valide,
                Statut::Valide,
                Statut::Valide,
                Statut::Valide,
                Statut::Ignoree
            ]
        );
        assert!(a.dto.lignes[6].messages[0].contains("€scarcell calcule"), "{:?}", a.dto.lignes[6].messages);
        assert_eq!(a.plan.comptes.len(), 2);
        assert_eq!(a.plan.comptes.iter().find(|c| c.type_ == "PEA").unwrap().ouverture, "2025-01-02");
        db.importer(&a.plan).unwrap();

        let lignes = db.positions().unwrap();
        assert_eq!(lignes.len(), 1);
        // Coût : 4 512,34 + 1,99 + 500 + 1,99 = 5 016,32 € pour 110 titres ; il en reste 90 au même PRU.
        assert_eq!(lignes[0].quantite_micro, 90_000_000);
        assert_eq!(lignes[0].pru_micro, 45_602_909);
        assert!(lignes[0].journal);
        assert_eq!(lignes[0].nom, "Amundi PEA Monde");

        // Second import du même fichier : aucun ajout.
        let b = analyse(&db, &csv, false);
        assert_eq!(b.dto.compteurs.doublons, 6);
        assert_eq!(b.dto.compteurs.a_importer, 0);
        assert!(b.dto.lignes[6].messages[0].contains("€scarcell calcule"));
    }

    #[test]
    fn vente_superieure_et_plafond() {
        let db = Store::en_memoire().unwrap();
        let csv = format!(
            "{ENTETE}2025-01-06;PEA;achat;FR001400U5Q4;;10;50;;;;\n\
             2025-01-07;PEA;vente;FR001400U5Q4;;11;50;;;;\n\
             2025-01-02;LDDS;versement;;;;;12500;;;\n\
             2025-03-02;LDDS;retrait;;;;;13000;;;\n\
             2025-04-02;LDDS;retrait;;;;;100;;;\n\
             2030-01-01;LDDS;versement;;;;;1;;;\n"
        );
        let a = analyse(&db, &csv, false);
        assert_eq!(
            statuts(&a),
            vec![Statut::Valide, Statut::Erreur, Statut::Avertissement, Statut::Erreur, Statut::Valide, Statut::Erreur]
        );
        assert!(a.dto.lignes[1].messages[0].contains("11 titres supérieure aux 10"), "{:?}", a.dto.lignes[1].messages);
        assert!(a.dto.lignes[2].messages[0].contains("plafond"), "{:?}", a.dto.lignes[2].messages);
        assert!(a.dto.lignes[3].messages[0].contains("insuffisant"));
        assert!(a.dto.lignes[5].messages[0].contains("futur"));
        assert_eq!(a.plan.operations.len(), 3);
    }

    #[test]
    fn ligne_manuelle_remplacee() {
        let mut db = Store::en_memoire().unwrap();
        db.ajouter_position(&Position {
            id: 0,
            isin: Some("FR0000120271".into()),
            symbole: "TTE.PA".into(),
            nom: "TotalEnergies".into(),
            quantite_micro: 5_000_000,
            pru_micro: 50_000_000,
            pays: Some("FR".into()),
            journal: false,
        })
        .unwrap();
        let csv = format!("{ENTETE}2025-01-06;PEA;achat;FR0000120271;;10;55;;;;\n");
        let a = analyse(&db, &csv, false);
        assert_eq!(statuts(&a), vec![Statut::Avertissement]);
        assert!(a.dto.lignes[0].messages[0].contains("saisie à la main"));
        assert!(a.nouveaux_isins.is_empty());
        db.importer(&a.plan).unwrap();
        let p = &db.positions().unwrap()[0];
        assert_eq!((p.quantite_micro, p.pru_micro, p.journal), (10_000_000, 55_000_000, true));
        assert_eq!((p.symbole.as_str(), p.pays.as_deref()), ("TTE.PA", Some("FR")));
    }

    #[test]
    fn deux_lignes_identiques_face_a_une_operation() {
        let mut db = Store::en_memoire().unwrap();
        let une = format!("{ENTETE}2025-03-10;LDDS;versement;;;;;100;;;\n");
        let a = analyse(&db, &une, false);
        db.importer(&a.plan).unwrap();
        let deux = format!("{une}2025-03-10;LDDS;versement;;;;;100;;;\n");
        let b = analyse(&db, &deux, false);
        assert_eq!(statuts(&b), vec![Statut::Doublon, Statut::Valide]);
        let c = analyse(&db, &deux, true);
        assert_eq!(c.dto.compteurs.a_importer, 2);
    }

    #[test]
    fn correspondance_incomplete() {
        let db = Store::en_memoire().unwrap();
        let a = analyse(&db, "Quand;Combien\n2025-01-01;10\n", false);
        assert_eq!(a.dto.erreurs_globales.len(), 1);
        assert!(a.dto.lignes.is_empty());
        let a = analyser(
            &db,
            "Quand;Où;Quoi;Combien\n10/03/2025;LDDS;versement;10,5\n".as_bytes(),
            "x.csv",
            Some(vec![Some("date".into()), Some("compte".into()), Some("type".into()), Some("montant".into())]),
            false,
            jour(),
            &HashMap::new(),
        )
        .unwrap();
        assert_eq!(statuts(&a), vec![Statut::Valide]);
        assert_eq!(a.plan.operations[0].1.montant_cents, Some(1_050));
    }

    #[test]
    fn correspondance_encodee_relue() {
        let c = vec![("Date\td'op".to_string(), Some("date".to_string())), ("X".into(), None)];
        assert_eq!(
            decoder_correspondance(&encoder(&c)),
            vec![("Date d'op".to_string(), Some("date".to_string())), ("X".into(), None)]
        );
    }

    /// Le modèle importé sur une base vide donne les mêmes PRU et soldes qu'une saisie manuelle équivalente.
    #[test]
    fn modele_identique_a_la_saisie_manuelle() {
        let mut db = Store::en_memoire().unwrap();
        let a = analyse(&db, &modele(), false);
        let s: Vec<Statut> = statuts(&a);
        assert_eq!(s.iter().filter(|x| **x == Statut::Valide).count(), 7, "{:?}", a.dto.lignes);
        assert_eq!(s[7], Statut::Ignoree);
        db.importer(&a.plan).unwrap();
        // PEA : 100 × 45,1234 € + 1,99 € de frais, puis 20 vendus : PRU (4 512,34 + 1,99) / 100 = 45,1433 €.
        let p = &db.positions().unwrap()[0];
        assert_eq!((p.quantite_micro, p.pru_micro), (80_000_000, 45_143_300));
        // Livret A : même solde que le solde de départ et le versement saisis à la main.
        let manuel = Store::en_memoire().unwrap();
        let id = manuel
            .ajouter_compte(&Compte {
                id: 0,
                nom: "Livret A".into(),
                type_: "LIVRET_A".into(),
                etablissement: String::new(),
                produit: None,
                ouverture: "2025-01-01".into(),
            })
            .unwrap();
        for (date, type_, m) in [("2025-01-01", "VALORISATION", 1_000_000), ("2025-03-10", "VERSEMENT", 150_000)] {
            manuel
                .ajouter_operation(&Operation {
                    id: 0,
                    compte_id: id,
                    date: date.into(),
                    type_: type_.into(),
                    montant_cents: Some(m),
                    isin: None,
                    quantite_micro: None,
                    cours_micro: None,
                    frais_cents: 0,
                    valeur_plan_cents: None,
                    libelle: None,
                })
                .unwrap();
        }
        let solde = |db: &Store| {
            let c = db.comptes().unwrap().into_iter().find(|c| c.type_ == "LIVRET_A").unwrap();
            simulation("LIVRET_A", &db.operations(c.id).unwrap(), jour()).unwrap().0
        };
        assert_eq!(solde(&db), solde(&manuel));
    }

    /// Cas le plus lent : des milliers de retraits impossibles, un par jour (refus jour après jour, borné).
    #[test]
    fn performance_retraits_impossibles() {
        let db = Store::en_memoire().unwrap();
        let mut csv = ENTETE.to_string();
        for i in 0..3_000 {
            let d = Date::depuis_jours_unix("2020-03-01".parse::<Date>().unwrap().jours_unix() + i);
            csv.push_str(&format!("{d};LEP;retrait;;;;;1;;;\n"));
        }
        let debut = std::time::Instant::now();
        let a = analyse(&db, &csv, false);
        assert_eq!(a.dto.compteurs.erreurs, 3_000);
        assert!(debut.elapsed().as_secs() < 5, "{:?}", debut.elapsed());
    }

    /// 5 000 lignes analysées rapidement (exigence : import en moins de 5 s).
    #[test]
    fn performance_5000_lignes() {
        let db = Store::en_memoire().unwrap();
        let mut csv = ENTETE.to_string();
        for i in 0..5_000 {
            let d = Date::depuis_jours_unix("2021-01-01".parse::<Date>().unwrap().jours_unix() + i / 4);
            if i % 2 == 0 {
                csv.push_str(&format!("{d};PEA;achat;FR001400U5Q4;;1;{}0;;0,99;;\n", 40 + i % 7));
            } else {
                csv.push_str(&format!("{d};Livret A;versement;;;;;{};;;\n", 1 + i % 3));
            }
        }
        let debut = std::time::Instant::now();
        let a = analyse(&db, &csv, false);
        assert_eq!(a.dto.compteurs.a_importer, 5_000, "{:?}", a.dto.lignes.iter().find(|l| l.statut == Statut::Erreur));
        assert!(debut.elapsed().as_secs() < 5, "{:?}", debut.elapsed());
    }
}
