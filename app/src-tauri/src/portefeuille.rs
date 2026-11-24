//! Portefeuilles d'actions PEA et CTO : suivi de lignes (titre, quantité, PRU), cours automatiques,
//! plus ou moins-values, dividendes et fiscalité propre à chaque enveloppe.
//! Pas d'établissement ni de journal d'opérations : on saisit ce qu'on détient.

use escarcelle_core::dividendes::{self as div, Enveloppe, Fiscalite, Versement};
use escarcelle_core::{Date, Money, Price, Rate};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::cours::{coter, dividendes, eligibilite_pea, est_isin, historique, pays_par_place, rechercher, rechercher_titres, symbole_change};
use crate::store::{Position, Store};
use crate::Etat;
use escarcelle_core::analyse::{analyser, Tendance, TypeNiveau};

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn enveloppe(code: &str) -> Resultat<Enveloppe> {
    Enveloppe::depuis_code(code).ok_or_else(|| format!("enveloppe inconnue : {code}"))
}

fn maintenant() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Retenue à la source d'un pays : réglage de l'utilisateur, sinon valeur par défaut.
/// `None` si le pays est inconnu (la retenue est alors comptée à 0 et signalée).
pub fn retenue(db: &Store, pays: &str) -> Resultat<Option<Rate>> {
    if let Some(v) = db.parametre(&format!("retenue.{pays}")).map_err(err)? {
        if let Ok(micro) = v.parse::<i64>() {
            return Ok(Some(Rate::from_micro(micro)));
        }
    }
    Ok(div::retenue_par_defaut(pays))
}

fn pays_ligne(p: &Position) -> Option<String> {
    p.pays
        .clone()
        .or_else(|| p.isin.as_deref().and_then(div::pays_isin).map(String::from))
        .or_else(|| pays_par_place(&p.symbole).map(String::from))
}

#[derive(Serialize)]
pub struct LigneDto {
    pub id: i64,
    pub isin: Option<String>,
    pub symbole: String,
    pub nom: String,
    pub pays: Option<String>,
    pub quantite_micro: i64,
    /// PRU en euros.
    pub pru_micro: i64,
    /// Devise de cotation ; `cours_micro` et `div_par_action_micro` sont exprimés dans cette devise.
    pub devise: String,
    /// Unités de devise pour 1 euro (1 000 000 pour l'euro) ; absent si le taux n'est pas connu.
    pub taux_change_micro: Option<i64>,
    pub cours_micro: Option<i64>,
    /// Cours converti en euros.
    pub cours_eur_micro: Option<i64>,
    pub cours_horodatage: Option<i64>,
    pub cours_source: Option<String>,
    pub investi_cents: i64,
    /// Valeur au dernier cours, ou au PRU si aucun cours n'est connu.
    pub valeur_cents: i64,
    pub pv_cents: i64,
    pub pv_micro: i64,
    /// Impôt si la plus-value était réalisée aujourd'hui (CTO), et plus-value nette.
    pub impot_pv_cents: i64,
    pub pv_nette_cents: i64,
    /// Variation du cours sur un an (millionièmes), si connue.
    pub variation_un_an_micro: Option<i64>,
    // Dividendes
    pub div_par_action_micro: i64,
    pub div_frequence: usize,
    pub div_rendement_micro: Option<i64>,
    pub div_croissance_micro: Option<i64>,
    pub div_prochain_estime: Option<String>,
    pub div_dernier_date: Option<String>,
    pub div_dernier_micro: Option<i64>,
    pub div_brut_cents: i64,
    pub retenue_micro: i64,
    pub retenue_connue: bool,
    pub div_retenue_cents: i64,
    pub div_credit_cents: i64,
    pub div_impot_cents: i64,
    pub div_ps_cents: i64,
    pub div_net_cents: i64,
}

#[derive(Serialize)]
pub struct PortefeuilleDto {
    pub enveloppe: String,
    pub lignes: Vec<LigneDto>,
    pub investi_cents: i64,
    pub valeur_cents: i64,
    pub pv_cents: i64,
    pub pv_micro: i64,
    pub impot_pv_cents: i64,
    pub pv_nette_cents: i64,
    pub div_brut_cents: i64,
    pub div_net_cents: i64,
    /// Rendement brut des dividendes sur la valeur du portefeuille.
    pub div_rendement_micro: i64,
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

/// Calcul du portefeuille d'une enveloppe : investi = quantité × PRU, valeur = quantité × cours.
pub fn portefeuille(etat: &Etat, code: &str) -> Resultat<PortefeuilleDto> {
    let env = enveloppe(code)?;
    let fisc = Fiscalite::v2026();
    let jour = crate::commands::aujourd_hui();
    let db = etat.db.lock().map_err(err)?;
    let mut lignes = Vec::new();
    for p in db.positions(code).map_err(err)? {
        let dernier = db.dernier_cours(&p.symbole).map_err(err)?;
        // Taux de change : unités de devise pour 1 euro.
        let taux_change = if p.devise == "EUR" {
            Some(1_000_000)
        } else {
            db.dernier_cours(&symbole_change(&p.devise)).map_err(err)?.map(|c| c.cours_micro).filter(|t| *t > 0)
        };
        let en_euros = |micro: i64| taux_change.map(|t| (micro as i128 * 1_000_000 / t as i128) as i64);
        let investi = Price::from_micro(p.pru_micro).valeur(p.quantite_micro);
        let cours = dernier.as_ref().map(|c| c.cours_micro);
        let cours_eur = cours.and_then(en_euros);
        let valeur = Price::from_micro(cours_eur.unwrap_or(p.pru_micro)).valeur(p.quantite_micro);
        let pv = valeur - investi;
        let impot_pv = div::impot_plus_value(pv, env, &fisc);

        let historique: Vec<Versement> = db
            .dividendes(&p.symbole)
            .map_err(err)?
            .into_iter()
            .filter_map(|(d, m)| Some(Versement { date: d.parse::<Date>().ok()?, montant: Price::from_micro(m) }))
            .collect();
        let syn = div::synthese(&historique, jour);
        let pays = pays_ligne(&p);
        let taux = match pays.as_deref() {
            Some(code_pays) => retenue(&db, code_pays)?,
            None => None,
        };
        let brut = Price::from_micro(en_euros(syn.annuel_par_action.micro()).unwrap_or(0)).valeur(p.quantite_micro);
        let net = div::dividende_net(brut, taux.unwrap_or(Rate::ZERO), env, &fisc);
        let infos = db.infos_titre(&p.symbole).map_err(err)?;
        let variation = match (infos.and_then(|i| i.0), cours) {
            (Some(avant), Some(c)) if avant > 0 => Some(ratio(c - avant, avant)),
            _ => None,
        };

        lignes.push(LigneDto {
            id: p.id,
            isin: p.isin.clone(),
            symbole: p.symbole.clone(),
            nom: p.nom.clone(),
            pays,
            quantite_micro: p.quantite_micro,
            pru_micro: p.pru_micro,
            devise: p.devise.clone(),
            taux_change_micro: taux_change,
            cours_micro: cours,
            cours_eur_micro: cours_eur,
            cours_horodatage: dernier.as_ref().map(|c| c.horodatage),
            cours_source: dernier.map(|c| c.source),
            investi_cents: investi.cents(),
            valeur_cents: valeur.cents(),
            pv_cents: pv.cents(),
            pv_micro: ratio(pv.cents(), investi.cents()),
            impot_pv_cents: impot_pv.cents(),
            pv_nette_cents: (pv - impot_pv).cents(),
            variation_un_an_micro: variation,
            div_par_action_micro: syn.annuel_par_action.micro(),
            div_frequence: syn.frequence,
            div_rendement_micro: cours.and_then(|c| div::rendement(syn.annuel_par_action, Price::from_micro(c))).map(|r| r.micro()),
            div_croissance_micro: syn.croissance_annuelle.map(|r| r.micro()),
            div_prochain_estime: syn.prochain_estime.map(|d| d.to_string()),
            div_dernier_date: syn.dernier.map(|v| v.date.to_string()),
            div_dernier_micro: syn.dernier.map(|v| v.montant.micro()),
            div_brut_cents: brut.cents(),
            retenue_micro: taux.map(|t| t.micro()).unwrap_or(0),
            retenue_connue: taux.is_some(),
            div_retenue_cents: net.retenue_etrangere.cents(),
            div_credit_cents: net.credit_impot.cents(),
            div_impot_cents: net.impot_francais.cents(),
            div_ps_cents: net.prelevements_sociaux.cents(),
            div_net_cents: net.net.cents(),
        });
    }
    let somme = |f: fn(&LigneDto) -> i64| lignes.iter().map(f).sum::<i64>();
    let investi = somme(|l: &LigneDto| l.investi_cents);
    let valeur = somme(|l: &LigneDto| l.valeur_cents);
    let pv = valeur - investi;
    let impot_pv = somme(|l: &LigneDto| l.impot_pv_cents);
    let div_brut = somme(|l: &LigneDto| l.div_brut_cents);
    let div_net = somme(|l: &LigneDto| l.div_net_cents);
    let poids: i64 = lignes.iter().filter(|l| l.div_croissance_micro.is_some()).map(|l| l.div_brut_cents).sum();
    let div_croissance = (poids > 0).then(|| {
        let s: i128 = lignes.iter().filter_map(|l| Some(l.div_croissance_micro? as i128 * l.div_brut_cents as i128)).sum();
        (s / poids as i128) as i64
    });
    Ok(PortefeuilleDto {
        enveloppe: code.into(),
        investi_cents: investi,
        valeur_cents: valeur,
        pv_cents: pv,
        pv_micro: ratio(pv, investi),
        impot_pv_cents: impot_pv,
        pv_nette_cents: pv - impot_pv,
        div_brut_cents: div_brut,
        div_net_cents: div_net,
        div_rendement_micro: ratio(div_brut, valeur),
        div_croissance_micro: div_croissance,
        lignes,
    })
}

/// Valeur et plus-value latente d'une enveloppe, pour le tableau de bord.
pub fn totaux(etat: &Etat, code: &str) -> Resultat<(Money, Money, Money)> {
    let p = portefeuille(etat, code)?;
    Ok((Money::from_cents(p.valeur_cents), Money::from_cents(p.pv_cents), Money::from_cents(p.div_net_cents)))
}

/// Met à jour le taux de change euro → devise (cours du symbole EURUSD=X, etc.).
async fn actualiser_change(etat: &Etat, devise: &str) -> Result<(), String> {
    let symbole = symbole_change(devise);
    let c = coter(&etat.http, &symbole).await.map_err(|e| format!("taux de change {devise} : {e}"))?;
    etat.db.lock().map_err(err)?.enregistrer_cotation(&symbole, c.cours_micro, c.horodatage, "Yahoo Finance").map_err(err)?;
    Ok(())
}

/// Met à jour les dividendes et le cours d'il y a un an d'un titre, au plus une fois par jour.
async fn actualiser_dividendes(etat: &Etat, symbole: &str, forcer: bool) -> Result<(), String> {
    let recent = etat.db.lock().map_err(err)?.infos_titre(symbole).map_err(err)?.map(|i| i.1);
    if !forcer && recent.is_some_and(|maj| maintenant() - maj < 86_400) {
        return Ok(());
    }
    let infos = dividendes(&etat.http, symbole).await?;
    let mut db = etat.db.lock().map_err(err)?;
    db.remplacer_dividendes(symbole, &infos.versements).map_err(err)?;
    db.definir_infos_titre(symbole, infos.cours_un_an_micro, maintenant()).map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn positions(etat: State<'_, Etat>, enveloppe: String) -> Resultat<PortefeuilleDto> {
    portefeuille(&etat, &enveloppe)
}

#[derive(Deserialize)]
pub struct SaisieLigne {
    /// ISIN (FR001400U5Q4) ou mnémonique (DCAM, DCAM.PA).
    pub identifiant: String,
    /// Symbole choisi dans les résultats de recherche (évite une nouvelle recherche).
    #[serde(default)]
    pub symbole: Option<String>,
    pub nom: Option<String>,
    pub quantite_micro: i64,
    pub pru_micro: i64,
    #[serde(default)]
    pub pays: Option<String>,
}

/// Ajoute une ligne : retrouve le symbole de cotation, enregistre la ligne, puis récupère le cours.
#[tauri::command]
pub async fn ajouter_position(etat: State<'_, Etat>, enveloppe: String, saisie: SaisieLigne) -> Resultat<PortefeuilleDto> {
    self::enveloppe(&enveloppe)?;
    if saisie.quantite_micro <= 0 || saisie.pru_micro < 0 {
        return Err("La quantité doit être positive et le PRU ne peut pas être négatif.".into());
    }
    let id = saisie.identifiant.trim().to_uppercase();
    if id.is_empty() {
        return Err("Indiquez un ISIN ou un mnémonique.".into());
    }
    let isin = est_isin(&id).then(|| id.clone());

    // Résolution du symbole : on cherche l'ISIN ou le mnémonique ; à défaut, on garde la saisie telle quelle.
    let (symbole, nom_trouve) = match saisie.symbole.clone().filter(|s| !s.is_empty()) {
        Some(s) => (s, None),
        None => match rechercher(&etat.http, &id).await {
            Ok(Some((s, n))) => (s, n),
            _ if !id.contains('.') && isin.is_none() => (format!("{id}.PA"), None),
            _ => (id.clone(), None),
        },
    };
    let nom = saisie.nom.filter(|n| !n.trim().is_empty()).or(nom_trouve).unwrap_or_else(|| id.clone());

    // Le cours est facultatif : la ligne est gardée même si la source ne répond pas.
    let cotation = coter(&etat.http, &symbole).await.ok();
    let devise = cotation.as_ref().map(|c| c.devise.clone()).unwrap_or_else(|| "EUR".into());
    if enveloppe == "PEA" && devise != "EUR" {
        return Err(format!("{symbole} est coté en {devise} : un PEA ne contient que des titres cotés en euros. Ajoutez-le plutôt au CTO."));
    }
    etat.db
        .lock()
        .map_err(err)?
        .ajouter_position(&Position {
            id: 0,
            enveloppe: enveloppe.clone(),
            isin,
            symbole: symbole.clone(),
            nom,
            quantite_micro: saisie.quantite_micro,
            pru_micro: saisie.pru_micro,
            pays: saisie.pays.filter(|p| p.len() == 2).map(|p| p.to_uppercase()),
            devise: devise.clone(),
        })
        .map_err(err)?;
    if let Some(c) = cotation {
        etat.db.lock().map_err(err)?.enregistrer_cotation(&symbole, c.cours_micro, c.horodatage, "Yahoo Finance").map_err(err)?;
    }
    if devise != "EUR" {
        let _ = actualiser_change(&etat, &devise).await;
    }
    let _ = actualiser_dividendes(&etat, &symbole, false).await;
    portefeuille(&etat, &enveloppe)
}

#[tauri::command]
pub fn modifier_position(
    etat: State<'_, Etat>,
    enveloppe: String,
    id: i64,
    quantite_micro: i64,
    pru_micro: i64,
    pays: Option<String>,
) -> Resultat<PortefeuilleDto> {
    if quantite_micro <= 0 || pru_micro < 0 {
        return Err("La quantité doit être positive et le PRU ne peut pas être négatif.".into());
    }
    let pays = pays.map(|p| p.trim().to_uppercase()).filter(|p| p.len() == 2);
    etat.db.lock().map_err(err)?.modifier_position(id, quantite_micro, pru_micro, pays.as_deref()).map_err(err)?;
    portefeuille(&etat, &enveloppe)
}

#[tauri::command]
pub fn supprimer_position(etat: State<'_, Etat>, enveloppe: String, id: i64) -> Resultat<PortefeuilleDto> {
    etat.db.lock().map_err(err)?.supprimer_position(id).map_err(err)?;
    portefeuille(&etat, &enveloppe)
}

#[derive(Serialize)]
pub struct Actualisation {
    pub portefeuille: PortefeuilleDto,
    pub erreurs: Vec<String>,
}

/// Met à jour le cours (et, une fois par jour, les dividendes) de chaque ligne.
/// Les erreurs sont rendues sans bloquer les autres lignes.
#[tauri::command]
pub async fn actualiser_cours(etat: State<'_, Etat>, enveloppe: String) -> Resultat<Actualisation> {
    let (symboles, devises): (Vec<String>, Vec<String>) = {
        let db = etat.db.lock().map_err(err)?;
        let lignes = db.positions(&enveloppe).map_err(err)?;
        let mut s: Vec<String> = lignes.iter().map(|p| p.symbole.clone()).collect();
        s.sort();
        s.dedup();
        let mut d: Vec<String> = lignes.into_iter().map(|p| p.devise).filter(|d| d != "EUR").collect();
        d.sort();
        d.dedup();
        (s, d)
    };
    let mut erreurs = Vec::new();
    for d in devises {
        if let Err(e) = actualiser_change(&etat, &d).await {
            erreurs.push(e);
        }
    }
    for s in symboles {
        match coter(&etat.http, &s).await {
            Ok(c) if enveloppe == "PEA" && c.devise != "EUR" => {
                erreurs.push(format!("{s} : coté en {}, un PEA ne contient que des titres cotés en euros", c.devise))
            }
            Ok(c) => {
                etat.db
                    .lock()
                    .map_err(err)?
                    .enregistrer_cotation(&s, c.cours_micro, c.horodatage, "Yahoo Finance")
                    .map_err(err)?;
            }
            Err(e) => erreurs.push(e),
        }
        if let Err(e) = actualiser_dividendes(&etat, &s, false).await {
            erreurs.push(e);
        }
    }
    Ok(Actualisation { portefeuille: portefeuille(&etat, &enveloppe)?, erreurs })
}

/// Saisie manuelle d'un cours, quand la source automatique ne trouve pas le titre.
#[tauri::command]
pub fn saisir_cours(etat: State<'_, Etat>, enveloppe: String, symbole: String, cours_micro: i64) -> Resultat<PortefeuilleDto> {
    if cours_micro <= 0 {
        return Err("Le cours doit être positif.".into());
    }
    etat.db.lock().map_err(err)?.enregistrer_cotation(&symbole, cours_micro, maintenant(), "saisie manuelle").map_err(err)?;
    portefeuille(&etat, &enveloppe)
}

#[derive(Serialize)]
pub struct RetenueDto {
    pub pays: String,
    pub nom: String,
    pub taux_micro: i64,
    pub defaut_micro: i64,
}

/// Retenues à la source par pays (réglages de l'utilisateur ou valeurs par défaut).
#[tauri::command]
pub fn retenues(etat: State<'_, Etat>) -> Resultat<Vec<RetenueDto>> {
    let db = etat.db.lock().map_err(err)?;
    div::PAYS_CONNUS
        .iter()
        .map(|(code, nom)| {
            let defaut = div::retenue_par_defaut(code).map(|r| r.micro()).unwrap_or(0);
            Ok(RetenueDto {
                pays: (*code).into(),
                nom: (*nom).into(),
                taux_micro: retenue(&db, code)?.map(|r| r.micro()).unwrap_or(defaut),
                defaut_micro: defaut,
            })
        })
        .collect()
}

/// Modifie la retenue d'un pays ; `taux_micro` absent = retour à la valeur par défaut.
#[tauri::command]
pub fn definir_retenue(etat: State<'_, Etat>, pays: String, taux_micro: Option<i64>) -> Resultat<Vec<RetenueDto>> {
    if let Some(t) = taux_micro {
        if !(0..=1_000_000).contains(&t) {
            return Err("La retenue doit être comprise entre 0 et 100 %.".into());
        }
    }
    {
        let db = etat.db.lock().map_err(err)?;
        let valeur = taux_micro.or_else(|| div::retenue_par_defaut(&pays).map(|r| r.micro())).unwrap_or(0);
        db.definir_parametre(&format!("retenue.{pays}"), &valeur.to_string()).map_err(err)?;
    }
    retenues(etat)
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
    pub devise: String,
}

/// Recherche d'actions et d'ETF par nom, mnémonique ou ISIN. PEA : cotations en euros ;
/// CTO : aussi les places américaines, suisse, britannique, nordiques, etc.
#[tauri::command]
pub async fn chercher_titres(etat: State<'_, Etat>, requete: String, enveloppe: Option<String>) -> Resultat<Vec<TitreDto>> {
    let q = requete.trim().to_uppercase();
    if q.len() < 2 {
        return Ok(Vec::new());
    }
    let isin = est_isin(&q).then(|| q.clone());
    let monde = enveloppe.as_deref() == Some("CTO");
    Ok(rechercher_titres(&etat.http, requete.trim(), monde)
        .await?
        .into_iter()
        .map(|t| TitreDto {
            eligibilite_pea: eligibilite_pea(isin.as_deref(), &t.symbole, &t.type_).into(),
            devise: t.devise,
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
pub async fn analyse_titre(etat: State<'_, Etat>, symbole: String, periode: String, intervalle: Option<String>) -> Resultat<AnalyseDto> {
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
        .db
        .lock()
        .map_err(err)?
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
        signaux: a.signaux.into_iter().map(|s| SignalDto { nom: s.nom.into(), sens: sens(s.sens), explication: s.explication }).collect(),
        synthese: sens(a.synthese),
    })
}

