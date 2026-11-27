//! PEA : suivi de lignes (titre, quantité, PRU), cours automatiques, plus ou moins-values et dividendes.
//! Pas d'établissement : on saisit ce qu'on détient, ou on importe ses achats et ventes (lignes calculées
//! par le journal, voir `import`).
//! Seuls les titres cotés en euros sont acceptés.

use escarcelle_core::dividendes::{self as div, Versement};
use escarcelle_core::{Date, Money, Price, Rate};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::cours::{
    coter, dividendes, eligibilite_pea, est_isin, historique, pays_par_place, rechercher, rechercher_titres,
    symbole_valide,
};
use crate::store::{Position, Store};
use crate::Etat;
use escarcelle_core::analyse::{analyser, Tendance, TypeNiveau};

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn maintenant() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Quantité maximale acceptée (dix millions de titres) : avec un prix d'un million d'euros au plus, une ligne vaut
/// au plus 10¹³ €, soit 10¹⁵ centimes ; il faudrait des milliers de lignes pour approcher la limite des entiers 64 bits.
const QUANTITE_MAX_MICRO: i64 = 10_000_000 * 1_000_000;
/// PRU ou cours maximal accepté (un million d'euros par titre).
const PRIX_MAX_MICRO: i64 = 1_000_000 * 1_000_000;

fn verifier_saisie(quantite_micro: i64, pru_micro: i64) -> Resultat<()> {
    if quantite_micro <= 0 || quantite_micro > QUANTITE_MAX_MICRO {
        return Err("La quantité doit être positive (dix millions de titres au plus).".into());
    }
    if !(0..=PRIX_MAX_MICRO).contains(&pru_micro) {
        return Err("Le PRU doit être compris entre 0 et 1 000 000 €.".into());
    }
    Ok(())
}

/// Retenue à la source d'un pays : réglage de l'utilisateur, sinon valeur par défaut.
/// `None` si le pays est inconnu (la retenue est alors comptée à 0 et signalée).
pub fn retenue(db: &Store, pays: &str) -> Resultat<Option<Rate>> {
    if let Some(v) = db.parametre(&format!("retenue.{pays}")).map_err(err)? {
        if let Ok(micro) = v.parse::<i64>() {
            if (0..=1_000_000).contains(&micro) {
                return Ok(Some(Rate::from_micro(micro)));
            }
        }
    }
    Ok(div::retenue_par_defaut(pays))
}

pub fn pays_ligne(p: &Position) -> Option<String> {
    p.pays
        .clone()
        .or_else(|| p.isin.as_deref().and_then(div::pays_isin).map(String::from))
        .or_else(|| pays_par_place(&p.symbole).map(String::from))
}

#[derive(Serialize)]
pub struct LigneDto {
    pub id: i64,
    /// Quantité et PRU calculés à partir des achats et ventes importés.
    pub journal: bool,
    pub isin: Option<String>,
    pub symbole: String,
    pub nom: String,
    pub pays: Option<String>,
    pub quantite_micro: i64,
    /// PRU en euros.
    pub pru_micro: i64,
    pub cours_micro: Option<i64>,
    pub cours_horodatage: Option<i64>,
    pub cours_source: Option<String>,
    pub investi_cents: i64,
    /// Valeur au dernier cours, ou au PRU si aucun cours n'est connu.
    pub valeur_cents: i64,
    pub pv_cents: i64,
    pub pv_micro: i64,
    /// Variation du cours sur un an (millionièmes), si connue.
    pub variation_un_an_micro: Option<i64>,
    // Dividendes
    pub div_par_action_micro: i64,
    pub div_frequence: usize,
    pub div_rendement_micro: Option<i64>,
    pub div_croissance_micro: Option<i64>,
    /// Années civiles comparées pour la croissance du dividende.
    pub div_croissance_debut: Option<i32>,
    pub div_croissance_fin: Option<i32>,
    pub div_prochain_estime: Option<String>,
    pub div_dernier_date: Option<String>,
    pub div_dernier_micro: Option<i64>,
    pub div_brut_cents: i64,
    pub retenue_micro: i64,
    pub retenue_connue: bool,
    pub div_retenue_cents: i64,
    pub div_net_cents: i64,
}

#[derive(Serialize)]
pub struct PortefeuilleDto {
    pub lignes: Vec<LigneDto>,
    pub investi_cents: i64,
    pub valeur_cents: i64,
    pub pv_cents: i64,
    pub pv_micro: i64,
    pub div_brut_cents: i64,
    pub div_net_cents: i64,
    /// Rendement brut des dividendes sur la valeur du portefeuille.
    pub div_rendement_micro: i64,
    /// Rendement brut des dividendes sur le prix de revient (montant investi).
    pub div_rendement_pru_micro: i64,
    /// Croissance moyenne du dividende, pondérée par les montants.
    pub div_croissance_micro: Option<i64>,
}

fn ratio(num: i64, den: i64) -> i64 {
    if den > 0 {
        (num as i128 * 1_000_000 / den as i128) as i64
    } else {
        0
    }
}

/// Calcul d'une ligne : investi = quantité × PRU, valeur = quantité × cours.
fn ligne(db: &Store, p: Position, jour: Date) -> Resultat<LigneDto> {
    let dernier = db.dernier_cours(&p.symbole).map_err(err)?;
    let investi = Price::from_micro(p.pru_micro).valeur(p.quantite_micro);
    let cours = dernier.as_ref().map(|c| c.cours_micro).filter(|c| *c > 0);
    let valeur = Price::from_micro(cours.unwrap_or(p.pru_micro)).valeur(p.quantite_micro);
    let pv = valeur - investi;

    let historique: Vec<Versement> = db
        .dividendes(&p.symbole)
        .map_err(err)?
        .into_iter()
        .filter_map(|(d, m)| Some(Versement { date: d.parse::<Date>().ok()?, montant: Price::from_micro(m) }))
        .collect();
    let syn = div::synthese(&historique, jour);
    let pays = pays_ligne(&p);
    let taux = match pays.as_deref() {
        Some(code_pays) => retenue(db, code_pays)?,
        None => None,
    };
    let brut = syn.annuel_par_action.valeur(p.quantite_micro);
    let net = div::dividende_net(brut, taux.unwrap_or(Rate::ZERO));
    let variation = match (db.infos_titre(&p.symbole).map_err(err)?.and_then(|i| i.0), cours) {
        (Some(avant), Some(c)) if avant > 0 => Some(ratio(c - avant, avant)),
        _ => None,
    };

    Ok(LigneDto {
        id: p.id,
        journal: p.journal,
        cours_horodatage: dernier.as_ref().map(|c| c.horodatage),
        cours_source: dernier.map(|c| c.source),
        isin: p.isin,
        symbole: p.symbole,
        nom: p.nom,
        pays,
        quantite_micro: p.quantite_micro,
        pru_micro: p.pru_micro,
        cours_micro: cours,
        investi_cents: investi.cents(),
        valeur_cents: valeur.cents(),
        pv_cents: pv.cents(),
        pv_micro: ratio(pv.cents(), investi.cents()),
        variation_un_an_micro: variation,
        div_par_action_micro: syn.annuel_par_action.micro(),
        div_frequence: syn.frequence,
        div_rendement_micro: cours
            .and_then(|c| div::rendement(syn.annuel_par_action, Price::from_micro(c)))
            .map(|r| r.micro()),
        div_croissance_micro: syn.croissance_annuelle.map(|r| r.micro()),
        div_croissance_debut: syn.croissance_periode.map(|p| p.0),
        div_croissance_fin: syn.croissance_periode.map(|p| p.1),
        div_prochain_estime: syn.prochain_estime.map(|d| d.to_string()),
        div_dernier_date: syn.dernier.map(|v| v.date.to_string()),
        div_dernier_micro: syn.dernier.map(|v| v.montant.micro()),
        div_brut_cents: brut.cents(),
        retenue_micro: taux.map(|t| t.micro()).unwrap_or(0),
        retenue_connue: taux.is_some(),
        div_retenue_cents: net.retenue_etrangere.cents(),
        div_net_cents: net.net.cents(),
    })
}

/// Calcul du portefeuille à partir de la base (sans accès réseau).
pub fn calculer(db: &Store, jour: Date) -> Resultat<PortefeuilleDto> {
    let lignes =
        db.positions().map_err(err)?.into_iter().map(|p| ligne(db, p, jour)).collect::<Resultat<Vec<LigneDto>>>()?;
    let somme = |f: fn(&LigneDto) -> i64| lignes.iter().map(f).sum::<i64>();
    let investi = somme(|l: &LigneDto| l.investi_cents);
    let valeur = somme(|l: &LigneDto| l.valeur_cents);
    let pv = valeur - investi;
    let div_brut = somme(|l: &LigneDto| l.div_brut_cents);
    let div_net = somme(|l: &LigneDto| l.div_net_cents);
    let poids: i64 = lignes.iter().filter(|l| l.div_croissance_micro.is_some()).map(|l| l.div_brut_cents).sum();
    let div_croissance = (poids > 0).then(|| {
        let s: i128 =
            lignes.iter().filter_map(|l| Some(l.div_croissance_micro? as i128 * l.div_brut_cents as i128)).sum();
        (s / poids as i128) as i64
    });
    Ok(PortefeuilleDto {
        investi_cents: investi,
        valeur_cents: valeur,
        pv_cents: pv,
        pv_micro: ratio(pv, investi),
        div_brut_cents: div_brut,
        div_net_cents: div_net,
        div_rendement_micro: ratio(div_brut, valeur),
        div_rendement_pru_micro: ratio(div_brut, investi),
        div_croissance_micro: div_croissance,
        lignes,
    })
}

pub fn portefeuille(etat: &Etat) -> Resultat<PortefeuilleDto> {
    calculer(&etat.db(), crate::commands::aujourd_hui())
}

/// Valeur, plus-value latente et dividendes nets annuels du PEA, pour le tableau de bord.
pub fn totaux(etat: &Etat) -> Resultat<(Money, Money, Money)> {
    let p = portefeuille(etat)?;
    Ok((Money::from_cents(p.valeur_cents), Money::from_cents(p.pv_cents), Money::from_cents(p.div_net_cents)))
}

/// Met à jour les dividendes et le cours d'il y a un an d'un titre, au plus une fois par jour.
async fn actualiser_dividendes(etat: &Etat, symbole: &str, forcer: bool) -> Resultat<()> {
    let recent = etat.db().infos_titre(symbole).map_err(err)?.map(|i| i.1);
    if !forcer && recent.is_some_and(|maj| (0..86_400).contains(&(maintenant() - maj))) {
        return Ok(());
    }
    let infos = dividendes(&etat.http, symbole).await?;
    let mut db = etat.db();
    db.remplacer_dividendes(symbole, &infos.versements).map_err(err)?;
    db.definir_infos_titre(symbole, infos.cours_un_an_micro, maintenant()).map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn positions(etat: State<'_, Etat>) -> Resultat<PortefeuilleDto> {
    portefeuille(&etat)
}

#[derive(Deserialize)]
pub struct SaisieLigne {
    /// ISIN (FR001400U5Q4) ou mnémonique (DCAM, DCAM.PA).
    pub identifiant: String,
    /// Symbole choisi dans les résultats de recherche (évite une nouvelle recherche).
    #[serde(default)]
    pub symbole: Option<String>,
    #[serde(default)]
    pub nom: Option<String>,
    pub quantite_micro: i64,
    pub pru_micro: i64,
    #[serde(default)]
    pub pays: Option<String>,
}

/// Code pays saisi : deux lettres, en majuscules.
fn code_pays(p: Option<String>) -> Option<String> {
    p.map(|p| p.trim().to_ascii_uppercase()).filter(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_uppercase()))
}

/// Ajoute une ligne : retrouve le symbole de cotation, vérifie qu'il est coté en euros, enregistre la ligne,
/// puis récupère le cours et les dividendes.
#[tauri::command]
pub async fn ajouter_position(etat: State<'_, Etat>, saisie: SaisieLigne) -> Resultat<PortefeuilleDto> {
    verifier_saisie(saisie.quantite_micro, saisie.pru_micro)?;
    let id = saisie.identifiant.trim().to_uppercase();
    if id.is_empty() {
        return Err("Indiquez un ISIN ou un mnémonique.".into());
    }
    let isin = est_isin(&id).then(|| id.clone());
    if let Some(i) = &isin {
        if etat.db().positions().map_err(err)?.iter().any(|p| p.journal && p.isin.as_deref() == Some(i.as_str())) {
            return Err("Ce titre est suivi par les opérations importées : ajoutez l'achat par un import CSV.".into());
        }
    }

    // Résolution du symbole : on cherche l'ISIN ou le mnémonique ; à défaut, on garde la saisie telle quelle.
    let (symbole, nom_trouve) = match saisie.symbole.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        Some(s) => (s, None),
        None => match rechercher(&etat.http, &id).await {
            Ok(Some((s, n))) => (s, n),
            _ if !id.contains('.') && isin.is_none() => (format!("{id}.PA"), None),
            _ => (id.clone(), None),
        },
    };
    if !symbole_valide(&symbole) {
        return Err(format!("« {symbole} » n'est pas un symbole de cotation valide."));
    }
    let nom =
        saisie.nom.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).or(nom_trouve).unwrap_or_else(|| id.clone());

    // Le cours est facultatif : la ligne est gardée même si Yahoo ne répond pas.
    let cotation = coter(&etat.http, &symbole).await.ok();
    if let Some(c) = &cotation {
        if c.devise != "EUR" {
            return Err(format!(
                "{symbole} est coté en {} : un PEA ne contient que des titres cotés en euros.",
                c.devise
            ));
        }
    }
    {
        let db = etat.db();
        db.ajouter_position(&Position {
            id: 0,
            isin,
            symbole: symbole.clone(),
            nom,
            quantite_micro: saisie.quantite_micro,
            pru_micro: saisie.pru_micro,
            pays: code_pays(saisie.pays),
            journal: false,
        })
        .map_err(err)?;
        if let Some(c) = cotation {
            db.enregistrer_cotation(&symbole, c.cours_micro, c.horodatage, c.source).map_err(err)?;
        }
    }
    let _ = actualiser_dividendes(&etat, &symbole, false).await;
    portefeuille(&etat)
}

#[tauri::command]
pub fn modifier_position(
    etat: State<'_, Etat>,
    id: i64,
    quantite_micro: i64,
    pru_micro: i64,
    pays: Option<String>,
) -> Resultat<PortefeuilleDto> {
    verifier_saisie(quantite_micro, pru_micro)?;
    {
        let db = etat.db();
        let p = db.positions().map_err(err)?.into_iter().find(|p| p.id == id).ok_or("Ligne introuvable.")?;
        // Ligne calculée par le journal : seul le pays se modifie, la quantité et le PRU viennent des opérations.
        if p.journal && (p.quantite_micro != quantite_micro || p.pru_micro != pru_micro) {
            return Err("Cette ligne est calculée à partir des opérations importées : sa quantité et son PRU \
                        changent par un nouvel import d'opérations."
                .into());
        }
        db.modifier_position(id, quantite_micro, pru_micro, code_pays(pays).as_deref()).map_err(err)?;
    }
    portefeuille(&etat)
}

#[tauri::command]
pub fn supprimer_position(etat: State<'_, Etat>, id: i64) -> Resultat<PortefeuilleDto> {
    {
        let mut db = etat.db();
        let journal = db.positions().map_err(err)?.into_iter().any(|p| p.id == id && p.journal);
        // Ligne du journal : ses achats et ventes sont supprimés avec elle (sinon elle reviendrait au prochain calcul).
        if journal {
            db.supprimer_ligne_journal(id).map_err(err)?;
        } else {
            db.supprimer_position(id).map_err(err)?;
        }
    }
    portefeuille(&etat)
}

#[derive(Serialize)]
pub struct Actualisation {
    pub portefeuille: PortefeuilleDto,
    pub erreurs: Vec<String>,
}

/// Met à jour le cours (et, une fois par jour, les dividendes) de chaque ligne.
/// Les erreurs sont rendues sans bloquer les autres lignes.
#[tauri::command]
pub async fn actualiser_cours(etat: State<'_, Etat>) -> Resultat<Actualisation> {
    let mut symboles: Vec<String> = etat.db().positions().map_err(err)?.into_iter().map(|p| p.symbole).collect();
    symboles.sort();
    symboles.dedup();
    let mut erreurs = Vec::new();
    for s in symboles {
        match coter(&etat.http, &s).await {
            Ok(c) if c.devise != "EUR" => {
                erreurs.push(format!("{s} : coté en {}, un PEA ne contient que des titres cotés en euros", c.devise))
            }
            Ok(c) => {
                if let Err(e) = etat.db().enregistrer_cotation(&s, c.cours_micro, c.horodatage, c.source) {
                    erreurs.push(format!("{s} : {e}"));
                }
            }
            Err(e) => erreurs.push(e),
        }
        if let Err(e) = actualiser_dividendes(&etat, &s, false).await {
            erreurs.push(e);
        }
    }
    Ok(Actualisation { portefeuille: portefeuille(&etat)?, erreurs })
}

/// Saisie manuelle d'un cours, quand Yahoo Finance ne trouve pas le titre.
#[tauri::command]
pub fn saisir_cours(etat: State<'_, Etat>, symbole: String, cours_micro: i64) -> Resultat<PortefeuilleDto> {
    if cours_micro <= 0 || cours_micro > PRIX_MAX_MICRO {
        return Err("Le cours doit être compris entre 0 et 1 000 000 €.".into());
    }
    etat.db().enregistrer_cotation(&symbole, cours_micro, maintenant(), "saisie manuelle").map_err(err)?;
    portefeuille(&etat)
}

#[derive(Serialize)]
pub struct RetenueDto {
    pub pays: String,
    pub nom: String,
    pub taux_micro: i64,
    pub defaut_micro: i64,
}

fn liste_retenues(db: &Store) -> Resultat<Vec<RetenueDto>> {
    div::PAYS_CONNUS
        .iter()
        .map(|(code, nom)| {
            let defaut = div::retenue_par_defaut(code).map(|r| r.micro()).unwrap_or(0);
            Ok(RetenueDto {
                pays: (*code).into(),
                nom: (*nom).into(),
                taux_micro: retenue(db, code)?.map(|r| r.micro()).unwrap_or(defaut),
                defaut_micro: defaut,
            })
        })
        .collect()
}

/// Retenues à la source par pays (réglages de l'utilisateur ou valeurs par défaut).
#[tauri::command]
pub fn retenues(etat: State<'_, Etat>) -> Resultat<Vec<RetenueDto>> {
    liste_retenues(&etat.db())
}

/// Modifie la retenue d'un pays ; `taux_micro` absent = retour à la valeur par défaut.
#[tauri::command]
pub fn definir_retenue(etat: State<'_, Etat>, pays: String, taux_micro: Option<i64>) -> Resultat<Vec<RetenueDto>> {
    let pays = pays.trim().to_uppercase();
    if !div::PAYS_CONNUS.iter().any(|(code, _)| *code == pays) {
        return Err(format!("pays inconnu : {pays}"));
    }
    if let Some(t) = taux_micro {
        if !(0..=1_000_000).contains(&t) {
            return Err("La retenue doit être comprise entre 0 et 100 %.".into());
        }
    }
    let db = etat.db();
    let valeur = taux_micro.or_else(|| div::retenue_par_defaut(&pays).map(|r| r.micro())).unwrap_or(0);
    db.definir_parametre(&format!("retenue.{pays}"), &valeur.to_string()).map_err(err)?;
    liste_retenues(&db)
}

#[derive(Serialize)]
pub struct TitreDto {
    pub symbole: String,
    pub nom: String,
    pub place: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub isin: Option<String>,
    pub eligibilite_pea: String,
}

/// Recherche d'actions et d'ETF cotés en euros, par nom, mnémonique ou ISIN.
#[tauri::command]
pub async fn chercher_titres(etat: State<'_, Etat>, requete: String) -> Resultat<Vec<TitreDto>> {
    let requete = requete.trim();
    if requete.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let q = requete.to_uppercase();
    let isin = est_isin(&q).then(|| q.clone());
    Ok(rechercher_titres(&etat.http, requete)
        .await?
        .into_iter()
        .map(|t| TitreDto {
            eligibilite_pea: eligibilite_pea(isin.as_deref(), &t.symbole, &t.type_).into(),
            symbole: t.symbole,
            nom: t.nom,
            place: t.place,
            type_: t.type_,
            isin: isin.clone(),
        })
        .collect())
}

#[derive(Serialize)]
pub struct PointDto {
    pub t: i64,
    pub ouverture: f64,
    pub cloture: f64,
    pub haut: f64,
    pub bas: f64,
    pub volume: f64,
    pub sma20: Option<f64>,
    pub sma50: Option<f64>,
    pub sma200: Option<f64>,
    pub bol_bas: Option<f64>,
    pub bol_haut: Option<f64>,
    pub rsi: Option<f64>,
    pub macd: Option<f64>,
    pub macd_signal: Option<f64>,
}

#[derive(Serialize)]
pub struct NiveauDto {
    pub prix: f64,
    pub touches: usize,
    /// "support" ou "resistance"
    #[serde(rename = "type")]
    pub type_: String,
}

#[derive(Serialize)]
pub struct SignalDto {
    pub nom: String,
    /// "haussier", "baissier" ou "neutre"
    pub sens: String,
    pub explication: String,
}

#[derive(Serialize)]
pub struct AnalyseDto {
    pub symbole: String,
    /// "1d" (journalier) ou "1wk" (hebdomadaire).
    pub intervalle: String,
    /// Dividendes détachés pendant la période affichée (horodatage, montant par action).
    pub dividendes: Vec<(i64, f64)>,
    pub points: Vec<PointDto>,
    pub niveaux: Vec<NiveauDto>,
    pub signaux: Vec<SignalDto>,
    pub synthese: String,
}

fn sens(t: Tendance) -> String {
    match t {
        Tendance::Haussier => "haussier",
        Tendance::Baissier => "baissier",
        Tendance::Neutre => "neutre",
    }
    .into()
}

/// Historique et analyse technique d'un titre. `periode` : 3mo, 6mo, 1y, 2y ou 5y ;
/// `intervalle` : "1d" (bougies journalières) ou "1wk" (hebdomadaires, indicateurs en semaines).
/// Les indicateurs sont calculés sur un historique plus long que la période affichée
/// (la moyenne 200 a besoin de 200 bougies).
#[tauri::command]
pub async fn analyse_titre(
    etat: State<'_, Etat>,
    symbole: String,
    periode: String,
    intervalle: Option<String>,
) -> Resultat<AnalyseDto> {
    if !symbole_valide(&symbole) {
        return Err(format!("« {symbole} » n'est pas un symbole de cotation valide."));
    }
    let hebdo = intervalle.as_deref() == Some("1wk");
    let (telecharge, mut seances) = match periode.as_str() {
        "3mo" => ("2y", 63),
        "6mo" => ("2y", 126),
        "1y" => ("2y", 252),
        "2y" => ("5y", 504),
        _ => ("5y", 1_260),
    };
    let mut b = historique(&etat.http, &symbole, if hebdo { "10y" } else { telecharge }).await?;
    if hebdo {
        b = escarcelle_core::chandeliers::hebdomadaire(&b);
        seances /= 5;
    }
    if b.len() < 30 {
        return Err(format!("{symbole} : historique trop court pour une analyse"));
    }
    let a = analyser(&b);
    let debut = b.len().saturating_sub(seances);
    let points = (debut..b.len())
        .map(|i| PointDto {
            t: b[i].t,
            ouverture: b[i].ouverture,
            cloture: b[i].cloture,
            haut: b[i].haut,
            bas: b[i].bas,
            volume: b[i].volume,
            sma20: a.sma20[i],
            sma50: a.sma50[i],
            sma200: a.sma200[i],
            bol_bas: a.bollinger[i].map(|x| x.bas),
            bol_haut: a.bollinger[i].map(|x| x.haut),
            rsi: a.rsi[i],
            macd: a.macd[i].map(|x| x.macd),
            macd_signal: a.macd[i].map(|x| x.signal),
        })
        .collect();
    let t0 = b[debut].t;
    let dividendes = etat
        .db()
        .dividendes(&symbole)
        .map_err(err)?
        .into_iter()
        .filter_map(|(d, m)| {
            let d: Date = d.parse().ok()?;
            let t = d.jours_unix() * 86_400;
            (t >= t0).then_some((t, m as f64 / 1_000_000.0))
        })
        .collect();
    Ok(AnalyseDto {
        symbole,
        intervalle: if hebdo { "1wk" } else { "1d" }.into(),
        dividendes,
        points,
        niveaux: a
            .niveaux
            .iter()
            .map(|n| NiveauDto {
                prix: n.prix,
                touches: n.touches,
                type_: if n.type_ == TypeNiveau::Support { "support" } else { "resistance" }.into(),
            })
            .collect(),
        signaux: a
            .signaux
            .into_iter()
            .map(|s| SignalDto { nom: s.nom.into(), sens: sens(s.sens), explication: s.explication })
            .collect(),
        synthese: sens(a.synthese),
    })
}
