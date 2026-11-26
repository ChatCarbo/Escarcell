//! Import en un clic d'un export de portefeuille (Moning, BoursoBank, Fortuneo, tableur) : chaque titre
//! du fichier crée ou met à jour une ligne du PEA (quantité et PRU). Le fichier peut aussi être un historique
//! d'opérations : l'interface bascule alors sur l'import détaillé (`import`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use escarcelle_core::import_csv::detecter_decimale;
use escarcelle_core::import_positions::{
    export_pea_pme, lire_position, nom_de_recherche, nom_du_format, reconnaitre, regrouper, ChampPosition, Genre,
    LecturePosition, PositionLue,
};
use serde::Serialize;
use tauri::State;

use crate::cours::{coter, rechercher, symbole_valide};
use crate::import::{lire_tableau, Tableau};
use crate::store::{Position, Store};
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Ce qu'Internet a appris d'un titre : symbole de cotation, nom, devise de cotation.
#[derive(Clone, Debug, Default)]
pub struct Resolution {
    pub symbole: Option<String>,
    pub nom: Option<String>,
    pub devise: Option<String>,
    /// Symbole trouvé seulement d'après le nom du titre (à vérifier).
    pub par_nom: bool,
}

/// Résolutions déjà faites pendant la session (l'analyse puis l'import ne cherchent qu'une fois).
fn cache() -> &'static Mutex<HashMap<String, Resolution>> {
    static C: OnceLock<Mutex<HashMap<String, Resolution>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StatutPosition {
    Nouvelle,
    MiseAJour,
    Identique,
    Ignoree,
    Erreur,
}

#[derive(Clone, Debug, Serialize)]
pub struct PositionDto {
    pub numero: usize,
    pub statut: StatutPosition,
    pub messages: Vec<String>,
    pub nom: Option<String>,
    pub isin: Option<String>,
    pub symbole: Option<String>,
    pub quantite_micro: Option<i64>,
    pub pru_micro: Option<i64>,
    /// Valeurs actuelles de la ligne mise à jour.
    pub avant_quantite_micro: Option<i64>,
    pub avant_pru_micro: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CompteursPositions {
    pub nouvelles: usize,
    pub mises_a_jour: usize,
    pub identiques: usize,
    pub ignorees: usize,
    pub erreurs: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct PositionsDto {
    pub fichier: String,
    /// Moning, BoursoBank, Escarcelle ou tableau.
    pub format: String,
    pub lignes: Vec<PositionDto>,
    /// Lignes du PEA saisies à la main qui ne sont pas dans le fichier (nom).
    pub absentes: Vec<String>,
    pub compteurs: CompteursPositions,
}

#[derive(Serialize)]
pub struct FichierDto {
    /// « positions » (photo du portefeuille) ou « operations » (historique, import détaillé).
    pub genre: &'static str,
    pub positions: Option<PositionsDto>,
}

/// Écritures d'un import de photo, appliquées en une transaction.
#[derive(Clone, Debug, Default)]
pub struct PlanPositions {
    /// Lignes existantes remplacées (identifiants) par une ligne mise à jour.
    pub mises_a_jour: Vec<(Vec<i64>, Position)>,
    pub nouvelles: Vec<Position>,
    /// Lignes saisies à la main absentes du fichier (supprimées seulement sur demande).
    pub absentes: Vec<i64>,
}

/// Lignes du fichier : (numéro de ligne, lecture).
fn lire(tableau: &Tableau) -> Resultat<(String, Vec<(usize, LecturePosition)>)> {
    let Some((i, Genre::Positions(corr))) = reconnaitre(&tableau.lignes) else {
        return Err("Colonnes non reconnues : il faut au moins la quantité, le prix de revient (PRU) et l'ISIN, \
                    le symbole ou le nom de chaque titre."
            .into());
    };
    if export_pea_pme(&tableau.lignes, i) {
        return Err("Ce fichier est l'export d'un PEA-PME : Escarcelle ne suit que le PEA.".into());
    }
    let donnees = &tableau.lignes[i + 1..];
    let valeurs = donnees.iter().flat_map(|l| {
        l.champs
            .iter()
            .zip(&corr)
            .filter(|(_, c)| matches!(c, Some(ChampPosition::Quantite | ChampPosition::Pru)))
            .map(|(v, _)| v.as_str())
    });
    let dec = detecter_decimale(valeurs, tableau.sep);
    Ok((
        nom_du_format(&tableau.lignes[i].champs).to_string(),
        donnees.iter().map(|l| (l.ligne, lire_position(&l.champs, &corr, dec))).collect(),
    ))
}

/// Clé d'un titre dans le fichier : ISIN, sinon symbole, sinon nom.
fn cle(p: &PositionLue) -> String {
    p.isin.clone().or(p.symbole.clone()).or(p.nom.as_ref().map(|n| n.to_lowercase())).unwrap_or_default()
}

/// Ce qu'il faut demander à Internet pour un titre : symbole d'après l'ISIN ou le nom, devise d'après le symbole.
fn requete(p: &PositionLue) -> String {
    p.symbole.clone().or(p.isin.clone()).or(p.nom.clone()).unwrap_or_default()
}

/// Symbole Euronext Paris déduit du mnémonique du libellé (Fortuneo : « TOTALENERGIES (TTE) » → TTE.PA).
fn symbole_mnemonique(p: &PositionLue) -> Option<String> {
    p.mnemonique.as_ref().map(|m| format!("{m}.PA"))
}

/// Ligne existante correspondant au titre : même ISIN, ou même symbole (du fichier, trouvé, ou déduit du mnémonique).
fn meme_titre(e: &Position, p: &PositionLue, trouve: Option<&str>) -> bool {
    (p.isin.is_some() && e.isin == p.isin)
        || [p.symbole.as_deref(), trouve, symbole_mnemonique(p).as_deref()]
            .into_iter()
            .flatten()
            .any(|s| e.symbole.eq_ignore_ascii_case(s))
}

fn fr(micro: i64) -> String {
    let e = micro / 1_000_000;
    let r = micro % 1_000_000;
    if r == 0 {
        e.to_string()
    } else {
        format!("{e},{}", format!("{r:06}").trim_end_matches('0'))
    }
}

/// Prépare l'import d'une photo : compare chaque titre aux lignes du PEA (par ISIN, puis par symbole).
pub fn planifier(
    fichier: &str,
    format: &str,
    lues: Vec<(usize, LecturePosition)>,
    existantes: &[Position],
    resolutions: &HashMap<String, Resolution>,
) -> (PositionsDto, PlanPositions) {
    // Regroupement des lignes d'un même titre (un export peut le répéter, par exemple sur deux comptes).
    let mut ordre: Vec<(usize, Option<PositionLue>, Vec<String>, bool)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (numero, l) in lues {
        match l {
            LecturePosition::Position(p) => {
                let k = cle(&p);
                if let Some(&j) = index.get(&k) {
                    let premier = ordre[j].0;
                    if let Some(a) = ordre[j].1.as_ref() {
                        let r = regrouper(a, &p);
                        ordre[j].1 = Some(r);
                    }
                    ordre[j].2.push(format!("additionnée avec la ligne {numero} du même titre"));
                    ordre.push((
                        numero,
                        None,
                        vec![format!("même titre que la ligne {premier} : regroupée avec elle")],
                        false,
                    ));
                } else {
                    index.insert(k, ordre.len());
                    ordre.push((numero, Some(p), Vec::new(), false));
                }
            }
            LecturePosition::Ignoree(m) => ordre.push((numero, None, vec![m], false)),
            LecturePosition::Erreur(e) => ordre.push((numero, None, e, true)),
        }
    }

    let mut dto = PositionsDto {
        fichier: fichier.into(),
        format: format.into(),
        lignes: Vec::new(),
        absentes: Vec::new(),
        compteurs: CompteursPositions::default(),
    };
    let mut plan = PlanPositions::default();
    let mut touchees: Vec<i64> = Vec::new();
    for (numero, p, mut messages, erreur) in ordre {
        let Some(p) = p else {
            let statut = if erreur { StatutPosition::Erreur } else { StatutPosition::Ignoree };
            dto.lignes.push(PositionDto {
                numero,
                statut,
                messages,
                nom: None,
                isin: None,
                symbole: None,
                quantite_micro: None,
                pru_micro: None,
                avant_quantite_micro: None,
                avant_pru_micro: None,
            });
            continue;
        };
        let r = resolutions.get(&requete(&p)).cloned().unwrap_or_default();
        let symbole = p.symbole.clone().or(r.symbole.clone()).or(symbole_mnemonique(&p));
        let pareilles: Vec<&Position> = existantes.iter().filter(|e| meme_titre(e, &p, r.symbole.as_deref())).collect();
        if r.par_nom {
            if let Some(s) = &r.symbole {
                messages.push(format!("titre retrouvé d'après son nom ({s}) : vérifiez qu'il s'agit bien de lui"));
            }
        }
        let mut statut;
        let (mut avant_q, mut avant_p) = (None, None);
        let symbole_final;
        if pareilles.iter().any(|e| e.journal) {
            statut = StatutPosition::Erreur;
            messages.push(
                "ligne calculée à partir des opérations importées : elle se met à jour par un import d'opérations"
                    .into(),
            );
            symbole_final = symbole.clone();
        } else if let Some(premiere) = pareilles.first() {
            avant_q = Some(premiere.quantite_micro);
            avant_p = Some(premiere.pru_micro);
            symbole_final = Some(premiere.symbole.clone());
            if pareilles.len() == 1 && premiere.quantite_micro == p.quantite_micro && premiere.pru_micro == p.pru_micro
            {
                statut = StatutPosition::Identique;
            } else {
                statut = StatutPosition::MiseAJour;
                if pareilles.len() > 1 {
                    messages
                        .push(format!("{} lignes saisies pour ce titre : remplacées par une seule", pareilles.len()));
                }
                plan.mises_a_jour.push((
                    pareilles.iter().map(|e| e.id).collect(),
                    Position {
                        id: 0,
                        isin: premiere.isin.clone().or(p.isin.clone()),
                        symbole: premiere.symbole.clone(),
                        nom: premiere.nom.clone(),
                        quantite_micro: p.quantite_micro,
                        pru_micro: p.pru_micro,
                        pays: premiere.pays.clone(),
                        journal: false,
                    },
                ));
            }
            touchees.extend(pareilles.iter().map(|e| e.id));
        } else {
            statut = StatutPosition::Nouvelle;
            let s = symbole.clone().or(p.isin.clone()).unwrap_or_default();
            symbole_final = Some(s.clone());
            if !symbole_valide(&s) {
                statut = StatutPosition::Erreur;
                messages.push("titre introuvable : indiquez son ISIN ou son symbole".into());
            } else if let Some(d) = r.devise.as_deref().filter(|d| *d != "EUR") {
                statut = StatutPosition::Erreur;
                messages.push(format!("coté en {d} : un PEA ne contient que des titres cotés en euros"));
            } else {
                if r.symbole.is_none() && p.symbole.is_none() && p.mnemonique.is_none() {
                    messages.push("symbole de cotation introuvable : le cours pourra être saisi à la main".into());
                }
                plan.nouvelles.push(Position {
                    id: 0,
                    isin: p.isin.clone(),
                    symbole: s.clone(),
                    nom: p.nom.clone().or(r.nom.clone()).unwrap_or(s),
                    quantite_micro: p.quantite_micro,
                    pru_micro: p.pru_micro,
                    pays: None,
                    journal: false,
                });
            }
        }
        if statut == StatutPosition::MiseAJour {
            messages.insert(
                0,
                format!(
                    "{} titres, PRU {} € → {} titres, PRU {} €",
                    fr(avant_q.unwrap_or(0)),
                    fr(avant_p.unwrap_or(0)),
                    fr(p.quantite_micro),
                    fr(p.pru_micro)
                ),
            );
        }
        if statut == StatutPosition::Identique {
            avant_q = None;
            avant_p = None;
        }
        dto.lignes.push(PositionDto {
            numero,
            statut,
            messages,
            nom: p.nom.clone().or(r.nom.clone()),
            isin: p.isin.clone(),
            symbole: symbole_final,
            quantite_micro: Some(p.quantite_micro),
            pru_micro: Some(p.pru_micro),
            avant_quantite_micro: avant_q,
            avant_pru_micro: avant_p,
        });
    }
    for e in existantes {
        if !e.journal && !touchees.contains(&e.id) {
            dto.absentes.push(e.nom.clone());
            plan.absentes.push(e.id);
        }
    }
    let c = &mut dto.compteurs;
    for l in &dto.lignes {
        match l.statut {
            StatutPosition::Nouvelle => c.nouvelles += 1,
            StatutPosition::MiseAJour => c.mises_a_jour += 1,
            StatutPosition::Identique => c.identiques += 1,
            StatutPosition::Ignoree => c.ignorees += 1,
            StatutPosition::Erreur => c.erreurs += 1,
        }
    }
    (dto, plan)
}

/// Cherche sur Internet le symbole, le nom et la devise des titres qui ne sont pas déjà dans le PEA.
/// Une recherche qui échoue n'empêche rien : le titre garde son ISIN comme symbole.
async fn resoudre(
    etat: &Etat,
    lues: &[(usize, LecturePosition)],
    existantes: &[Position],
) -> HashMap<String, Resolution> {
    let mut res = HashMap::new();
    for (_, l) in lues {
        let LecturePosition::Position(p) = l else { continue };
        let connue = existantes.iter().any(|e| meme_titre(e, p, None));
        let q = requete(p);
        if connue || q.is_empty() || res.contains_key(&q) || res.len() >= 200 {
            continue;
        }
        // Verrou relâché aussitôt : jamais tenu pendant une recherche sur Internet.
        let deja = cache().lock().unwrap_or_else(|e| e.into_inner()).get(&q).cloned();
        if let Some(r) = deja {
            res.insert(q, r);
            continue;
        }
        let mut r = Resolution::default();
        // 1. Symbole donné par le fichier (Moning). 2. Recherche par ISIN. 3. Mnémonique du libellé, s'il est coté.
        // 4. Recherche par nom (ISIN provisoire, par exemple des actions à prime de fidélité).
        if let Some(s) = &p.symbole {
            r.symbole = Some(s.clone());
        } else if let Some(Ok(Some((s, n)))) = match &p.isin {
            Some(i) => Some(rechercher(&etat.http, i).await),
            None => None,
        } {
            if symbole_valide(&s) {
                r.symbole = Some(s);
                r.nom = n;
            }
        }
        if r.symbole.is_none() {
            if let Some(s) = symbole_mnemonique(p).filter(|s| symbole_valide(s)) {
                if let Ok(c) = coter(&etat.http, &s).await {
                    r.devise = Some(c.devise.clone());
                    r.symbole = Some(s);
                }
            }
        }
        if r.symbole.is_none() {
            let nom = p.nom.as_deref().map(nom_de_recherche).unwrap_or_default();
            if !nom.is_empty() {
                if let Ok(Some((s, n))) = rechercher(&etat.http, &nom).await {
                    if symbole_valide(&s) {
                        r.symbole = Some(s);
                        r.nom = n;
                        r.par_nom = true;
                    }
                }
            }
        }
        if let (Some(s), None) = (&r.symbole, &r.devise) {
            if let Ok(c) = coter(&etat.http, s).await {
                r.devise = Some(c.devise.clone());
            }
        }
        cache().lock().unwrap_or_else(|e| e.into_inner()).insert(q.clone(), r.clone());
        res.insert(q, r);
    }
    res
}

async fn preparer(etat: &Etat, chemin: &str) -> Resultat<Option<(PositionsDto, PlanPositions)>> {
    let tableau = lire_tableau(chemin)?;
    if matches!(reconnaitre(&tableau.lignes), Some((_, Genre::Operations))) {
        return Ok(None);
    }
    let fichier = crate::import::nom_fichier(chemin);
    let (format, lues) = lire(&tableau)?;
    let existantes = etat.db().positions().map_err(err)?;
    let resolutions = resoudre(etat, &lues, &existantes).await;
    // Relu après les recherches : une ligne a pu être ajoutée entre-temps.
    let existantes = etat.db().positions().map_err(err)?;
    Ok(Some(planifier(&fichier, &format, lues, &existantes, &resolutions)))
}

/// Analyse un fichier choisi depuis la page PEA : photo du portefeuille (aperçu des lignes) ou historique
/// d'opérations (l'interface ouvre alors l'import détaillé).
#[tauri::command]
pub async fn analyser_fichier(etat: State<'_, Etat>, chemin: String) -> Resultat<FichierDto> {
    Ok(match preparer(&etat, &chemin).await? {
        Some((dto, _)) => FichierDto { genre: "positions", positions: Some(dto) },
        None => FichierDto { genre: "operations", positions: None },
    })
}

#[derive(Serialize)]
pub struct RapportPositionsDto {
    pub sauvegarde: String,
    pub positions: PositionsDto,
    pub supprimees: usize,
}

/// Importe une photo du portefeuille : nouvelles lignes et mises à jour en une transaction, après une copie
/// de sécurité (restaurable dans Paramètres). Les lignes absentes du fichier ne sont supprimées que si `supprimer_absentes`.
#[tauri::command]
pub async fn importer_positions(
    etat: State<'_, Etat>,
    chemin: String,
    supprimer_absentes: bool,
) -> Resultat<RapportPositionsDto> {
    let Some((dto, mut plan)) = preparer(&etat, &chemin).await? else {
        return Err("Ce fichier est un historique d'opérations.".into());
    };
    if !supprimer_absentes {
        plan.absentes.clear();
    }
    if plan.mises_a_jour.is_empty() && plan.nouvelles.is_empty() && plan.absentes.is_empty() {
        return Err("Rien à importer : le PEA est déjà à jour.".into());
    }
    let mut db = etat.db();
    let copie = crate::reglages::copie_de_securite(&db, &etat.chemin_db, "avant-import")?;
    let supprimees = plan.absentes.len();
    appliquer(&mut db, &plan)?;
    Ok(RapportPositionsDto { sauvegarde: copie.to_string_lossy().into_owned(), positions: dto, supprimees })
}

pub fn appliquer(db: &mut Store, plan: &PlanPositions) -> Resultat<()> {
    db.importer_positions(&plan.mises_a_jour, &plan.nouvelles, &plan.absentes).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::tableau_texte;

    const MONING: &str = "Ticker,Total Shares,Price average,Price average in stock currency,Name,Latest Price,Custom asset yield,Dividend Yield,Value Stock Currency,Value,Value invested,Return,Dividends,Price Variation One Year,ISIN,Dividend safety,Growth score\n\
AI.PA,11.00,140.99,true,Air Liquide SA,190.32,,1.79,2093.52,2093.52,1550.89,34.99,36.96,30,FR0000120073,,\n\
DCAM.PA,1618.00,5.32,true,Amundi PEA Monde (MSCI World) UCITS ETF,6.10,,0.0,9876.27,9876.27,8601.29,14.82,0.0,13,FR001400U5Q4,,\n\
AM.PA,10.00,156.99,true,Dassault Aviation SA,301.40,,1.6,3014.0,3014.0,1569.86,91.99,47.8,14,FR0014004L86,,\n\
TTE.PA,50.00,35.07,true,TotalEnergies SE,78.36,,4.62,3918.0,3918.0,1753.35,123.46,182.5,45,FR0000120271,,\n";

    fn plan(db: &Store, csv: &str, res: &HashMap<String, Resolution>) -> (PositionsDto, PlanPositions) {
        let (format, lues) = lire(&tableau_texte(csv.as_bytes())).unwrap();
        planifier("test.csv", &format, lues, &db.positions().unwrap(), res)
    }

    fn ligne(isin: Option<&str>, symbole: &str, q: i64, pru: i64) -> Position {
        Position {
            id: 0,
            isin: isin.map(String::from),
            symbole: symbole.into(),
            nom: symbole.into(),
            quantite_micro: q,
            pru_micro: pru,
            pays: Some("FR".into()),
            journal: false,
        }
    }

    #[test]
    fn moning_sur_un_pea_vide_puis_a_jour() {
        let mut db = Store::en_memoire().unwrap();
        let (dto, p) = plan(&db, MONING, &HashMap::new());
        assert_eq!(dto.format, "Moning");
        assert_eq!(dto.compteurs.nouvelles, 4);
        appliquer(&mut db, &p).unwrap();
        let lignes = db.positions().unwrap();
        assert_eq!(lignes.len(), 4);
        let ai = lignes.iter().find(|l| l.symbole == "AI.PA").unwrap();
        assert_eq!(
            (ai.quantite_micro, ai.pru_micro, ai.isin.as_deref()),
            (11_000_000, 140_990_000, Some("FR0000120073"))
        );
        assert_eq!(ai.nom, "Air Liquide SA");

        // Le même fichier une seconde fois : rien à faire.
        let (dto, p) = plan(&db, MONING, &HashMap::new());
        assert_eq!(dto.compteurs.identiques, 4);
        assert!(p.nouvelles.is_empty() && p.mises_a_jour.is_empty());
    }

    #[test]
    fn boursobank_met_a_jour_par_isin_et_garde_le_pays() {
        let mut db = Store::en_memoire().unwrap();
        db.ajouter_position(&ligne(Some("FR0004050250"), "NRO.PA", 5_000_000, 30_000_000)).unwrap();
        db.ajouter_position(&ligne(None, "MC.PA", 1_000_000, 600_000_000)).unwrap();
        let csv = "\u{FEFF}name;isin;quantity;buyingPrice;lastPrice;intradayVariation;amount;amountVariation;variation\nNEURONES;FR0004050250;11,00;35,83;40,10;1,13;441,10;46,94;11,91\n";
        let (dto, mut p) = plan(&db, csv, &HashMap::new());
        p.absentes.clear(); // comme l'import sans « supprimer les lignes absentes »
        assert_eq!(dto.format, "BoursoBank");
        assert_eq!(dto.lignes[0].statut, StatutPosition::MiseAJour);
        assert_eq!(dto.lignes[0].messages[0], "5 titres, PRU 30 € → 11 titres, PRU 35,83 €");
        assert_eq!(dto.absentes, vec!["MC.PA".to_string()]);
        appliquer(&mut db, &p).unwrap();
        let nro = db.positions().unwrap().into_iter().find(|l| l.symbole == "NRO.PA").unwrap();
        assert_eq!((nro.quantite_micro, nro.pru_micro, nro.pays.as_deref()), (11_000_000, 35_830_000, Some("FR")));
        assert_eq!(db.positions().unwrap().len(), 2, "la ligne absente du fichier est gardée");

        // Avec suppression des absentes.
        let (_, p) = plan(&db, csv, &HashMap::new());
        assert_eq!(p.absentes.len(), 1);
        appliquer(&mut db, &p).unwrap();
        assert_eq!(db.positions().unwrap().len(), 1);
    }

    #[test]
    fn symbole_trouve_par_isin_et_devise_controlee() {
        let mut db = Store::en_memoire().unwrap();
        // Ligne saisie avec le seul symbole : retrouvée grâce au symbole trouvé pour l'ISIN du fichier.
        db.ajouter_position(&ligne(None, "NRO.PA", 5_000_000, 30_000_000)).unwrap();
        let csv = "name;isin;quantity;buyingPrice\nNEURONES;FR0004050250;11;35,83\nAPPLE;US0378331005;2;150\n";
        let mut res = HashMap::new();
        res.insert(
            "FR0004050250".to_string(),
            Resolution { symbole: Some("NRO.PA".into()), nom: None, devise: Some("EUR".into()), par_nom: false },
        );
        res.insert(
            "US0378331005".to_string(),
            Resolution {
                symbole: Some("AAPL".into()),
                nom: Some("Apple".into()),
                devise: Some("USD".into()),
                par_nom: false,
            },
        );
        let (dto, p) = plan(&db, csv, &res);
        assert_eq!(dto.lignes[0].statut, StatutPosition::MiseAJour);
        assert_eq!(dto.lignes[1].statut, StatutPosition::Erreur);
        assert!(dto.lignes[1].messages[0].contains("USD"));
        appliquer(&mut db, &p).unwrap();
        assert_eq!(db.positions().unwrap()[0].isin.as_deref(), Some("FR0004050250"));
    }

    #[test]
    fn lignes_regroupees_et_ligne_du_journal_protegee() {
        let db = Store::en_memoire().unwrap();
        db.ajouter_position(&Position { journal: true, ..ligne(Some("FR0000120271"), "TTE.PA", 1_000_000, 1) })
            .unwrap();
        let csv = "isin;quantite;pru\nFR0000120073;10;30\nFR0000120073;30;40\nFR0000120271;5;50\n";
        let (dto, p) = plan(&db, csv, &HashMap::new());
        let statuts: Vec<StatutPosition> = dto.lignes.iter().map(|l| l.statut).collect();
        assert_eq!(statuts, vec![StatutPosition::Nouvelle, StatutPosition::Ignoree, StatutPosition::Erreur]);
        assert_eq!((p.nouvelles[0].quantite_micro, p.nouvelles[0].pru_micro), (40_000_000, 37_500_000));
        assert!(dto.absentes.is_empty(), "une ligne du journal n'est jamais « absente »");
    }

    /// Après Moning, l'export Fortuneo du même PEA : rien ne change, même pour Air Liquide dont la ligne à
    /// prime de fidélité porte un ISIN provisoire (retrouvée par son nom).
    #[test]
    fn fortuneo_apres_moning() {
        let mut db = Store::en_memoire().unwrap();
        let (_, p) = plan(&db, MONING, &HashMap::new());
        appliquer(&mut db, &p).unwrap();
        let fortuneo = "Portefeuille xxxxxxxxxx;;;;;;;;;;;\n(PEA M xxxxxxxxxxx);;;;;;;;;;;\n26/11/2026;;;;;;;;;;;\n;;;;;;;;;;;\n\
Libellé;Cours;;Dev;Var/Veille;Qté;PRU;Valorisation;+/- values;+/- values (%);Poids;ISIN\n\
AIR LIQUIDE PF28   (-);190,32;;EUR;0.99%;11;140,99;2093,52;542,63;34.99%;11.0758%;FR0014010OO5\n\
AMUNDI PEA MONDE (MSCI World) UCITS ETF - Acc EUR ACC (DCAM);6,104;;EUR;2.85%;1618;5,3164;9876,272;1274,3368;14.81%;52.2505%;FR001400U5Q4\n\
DASSAULT AVIATION (AM);301,4;;EUR;3.01%;10;156,986;3014;1444,14;91.99%;15.9456%;FR0014004L86\n\
TOTALENERGIES (TTE);78,36;;EUR;1.4%;50;35,0666;3918;2164,67;123.46%;20.7282%;FR0000120271\n";
        let mut res = HashMap::new();
        res.insert(
            "FR0014010OO5".to_string(),
            Resolution { symbole: Some("AI.PA".into()), nom: None, devise: Some("EUR".into()), par_nom: true },
        );
        let (dto, p) = plan(&db, fortuneo, &res);
        let statuts: Vec<StatutPosition> = dto.lignes.iter().map(|l| l.statut).collect();
        // Air Liquide : même quantité et même PRU ; les autres : PRU plus précis chez Fortuneo (5,3164 contre 5,32).
        assert_eq!(statuts[0], StatutPosition::Identique, "{:?}", dto.lignes[0]);
        assert!(dto.lignes[0].messages[0].contains("d'après son nom"));
        assert!(statuts[1..].iter().all(|s| *s == StatutPosition::MiseAJour), "{statuts:?}");
        assert!(p.nouvelles.is_empty());
        assert!(dto.absentes.is_empty());
        appliquer(&mut db, &p).unwrap();
        let tte = db.positions().unwrap().into_iter().find(|l| l.symbole == "TTE.PA").unwrap();
        assert_eq!(tte.pru_micro, 35_066_600);
        assert_eq!(db.positions().unwrap().len(), 4);

        // Sur un PEA vide, le mnémonique donne le symbole sans recherche.
        let vide = Store::en_memoire().unwrap();
        let (_, p) = plan(&vide, fortuneo, &HashMap::new());
        let symboles: Vec<&str> = p.nouvelles.iter().map(|n| n.symbole.as_str()).collect();
        assert_eq!(symboles, vec!["FR0014010OO5", "DCAM.PA", "AM.PA", "TTE.PA"]);
        assert_eq!(p.nouvelles[3].nom, "TOTALENERGIES");

        let pme = fortuneo.replace("(PEA M xxxxxxxxxxx)", "(PEA-PME xxxxxxxxxxx)");
        assert!(lire(&tableau_texte(pme.as_bytes())).unwrap_err().contains("PEA-PME"));
    }

    #[test]
    fn historique_d_operations_detecte() {
        let t = tableau_texte(escarcelle_core::import_csv::modele().as_bytes());
        assert!(matches!(reconnaitre(&t.lignes), Some((_, Genre::Operations))));
        assert!(lire(&tableau_texte(b"a;b\n1;2\n")).is_err());
    }
}
