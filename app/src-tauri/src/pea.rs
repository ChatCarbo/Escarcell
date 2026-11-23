//! PEA en V1 : suivi de lignes (titre, quantité, PRU), cours automatiques, gain ou perte.
//! Pas d'établissement ni de journal d'opérations : on saisit ce qu'on détient.

use escarcelle_core::{Money, Price};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::cours::{coter, eligibilite_pea, est_isin, historique, rechercher, rechercher_titres};
use escarcelle_core::analyse::{analyser, Tendance, TypeNiveau};
use crate::store::Position;
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
pub struct LigneDto {
    pub id: i64,
    pub isin: Option<String>,
    pub symbole: String,
    pub nom: String,
    pub quantite_micro: i64,
    pub pru_micro: i64,
    pub cours_micro: Option<i64>,
    pub cours_horodatage: Option<i64>,
    pub cours_source: Option<String>,
    pub investi_cents: i64,
    /// Valeur au dernier cours, ou au PRU si aucun cours n'est connu.
    pub valeur_cents: i64,
    pub pv_cents: i64,
    pub pv_micro: i64,
}

#[derive(Serialize)]
pub struct PortefeuilleDto {
    pub lignes: Vec<LigneDto>,
    pub investi_cents: i64,
    pub valeur_cents: i64,
    pub pv_cents: i64,
    pub pv_micro: i64,
}

fn rendement(pv: Money, investi: Money) -> i64 {
    if investi.cents() > 0 {
        (pv.cents() as i128 * 1_000_000 / investi.cents() as i128) as i64
    } else {
        0
    }
}

/// Calcul du portefeuille : investi = quantité × PRU, valeur = quantité × cours.
pub fn portefeuille(etat: &Etat) -> Resultat<PortefeuilleDto> {
    let db = etat.db.lock().map_err(err)?;
    let mut lignes = Vec::new();
    for p in db.positions("PEA").map_err(err)? {
        let dernier = db.dernier_cours(&p.symbole).map_err(err)?;
        let investi = Price::from_micro(p.pru_micro).valeur(p.quantite_micro);
        let cours = dernier.as_ref().map(|c| c.cours_micro);
        let valeur = Price::from_micro(cours.unwrap_or(p.pru_micro)).valeur(p.quantite_micro);
        let pv = valeur - investi;
        lignes.push(LigneDto {
            id: p.id,
            isin: p.isin,
            symbole: p.symbole,
            nom: p.nom,
            quantite_micro: p.quantite_micro,
            pru_micro: p.pru_micro,
            cours_micro: cours,
            cours_horodatage: dernier.as_ref().map(|c| c.horodatage),
            cours_source: dernier.map(|c| c.source),
            investi_cents: investi.cents(),
            valeur_cents: valeur.cents(),
            pv_cents: pv.cents(),
            pv_micro: rendement(pv, investi),
        });
    }
    let investi: Money = lignes.iter().map(|l| Money::from_cents(l.investi_cents)).sum();
    let valeur: Money = lignes.iter().map(|l| Money::from_cents(l.valeur_cents)).sum();
    let pv = valeur - investi;
    Ok(PortefeuilleDto {
        lignes,
        investi_cents: investi.cents(),
        valeur_cents: valeur.cents(),
        pv_cents: pv.cents(),
        pv_micro: rendement(pv, investi),
    })
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
    pub nom: Option<String>,
    pub quantite_micro: i64,
    pub pru_micro: i64,
}

/// Ajoute une ligne : retrouve le symbole de cotation, enregistre la ligne, puis récupère le cours.
#[tauri::command]
pub async fn ajouter_position(etat: State<'_, Etat>, saisie: SaisieLigne) -> Resultat<PortefeuilleDto> {
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

    etat.db
        .lock()
        .map_err(err)?
        .ajouter_position(&Position {
            id: 0,
            enveloppe: "PEA".into(),
            isin,
            symbole: symbole.clone(),
            nom,
            quantite_micro: saisie.quantite_micro,
            pru_micro: saisie.pru_micro,
        })
        .map_err(err)?;

    // Le cours est facultatif : la ligne est gardée même si la source ne répond pas.
    if let Ok(c) = coter(&etat.http, &symbole).await {
        if c.devise == "EUR" {
            etat.db.lock().map_err(err)?.enregistrer_cotation(&symbole, c.cours_micro, c.horodatage, "Yahoo Finance").map_err(err)?;
        }
    }
    portefeuille(&etat)
}

#[tauri::command]
pub fn modifier_position(etat: State<'_, Etat>, id: i64, quantite_micro: i64, pru_micro: i64) -> Resultat<PortefeuilleDto> {
    if quantite_micro <= 0 || pru_micro < 0 {
        return Err("La quantité doit être positive et le PRU ne peut pas être négatif.".into());
    }
    etat.db.lock().map_err(err)?.modifier_position(id, quantite_micro, pru_micro).map_err(err)?;
    portefeuille(&etat)
}

#[tauri::command]
pub fn supprimer_position(etat: State<'_, Etat>, id: i64) -> Resultat<PortefeuilleDto> {
    etat.db.lock().map_err(err)?.supprimer_position(id).map_err(err)?;
    portefeuille(&etat)
}

#[derive(Serialize)]
pub struct Actualisation {
    pub portefeuille: PortefeuilleDto,
    pub erreurs: Vec<String>,
}

/// Met à jour le cours de chaque ligne. Les erreurs sont rendues sans bloquer les autres lignes.
#[tauri::command]
pub async fn actualiser_cours(etat: State<'_, Etat>) -> Resultat<Actualisation> {
    let symboles: Vec<String> = {
        let db = etat.db.lock().map_err(err)?;
        let mut s: Vec<String> = db.positions("PEA").map_err(err)?.into_iter().map(|p| p.symbole).collect();
        s.sort();
        s.dedup();
        s
    };
    let mut erreurs = Vec::new();
    for s in symboles {
        match coter(&etat.http, &s).await {
            Ok(c) if c.devise != "EUR" => erreurs.push(format!("{s} : cotée en {}, seules les cotations en euros sont gérées", c.devise)),
            Ok(c) => {
                etat.db
                    .lock()
                    .map_err(err)?
                    .enregistrer_cotation(&s, c.cours_micro, c.horodatage, "Yahoo Finance")
                    .map_err(err)?;
            }
            Err(e) => erreurs.push(e),
        }
    }
    Ok(Actualisation { portefeuille: portefeuille(&etat)?, erreurs })
}

/// Saisie manuelle d'un cours, quand la source automatique ne trouve pas le titre.
#[tauri::command]
pub fn saisir_cours(etat: State<'_, Etat>, symbole: String, cours_micro: i64) -> Resultat<PortefeuilleDto> {
    if cours_micro <= 0 {
        return Err("Le cours doit être positif.".into());
    }
    let maintenant = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    etat.db.lock().map_err(err)?.enregistrer_cotation(&symbole, cours_micro, maintenant, "saisie manuelle").map_err(err)?;
    portefeuille(&etat)
}

/// Utilisé par le tableau de bord.
pub fn totaux(etat: &Etat) -> Resultat<(Money, Money)> {
    let p = portefeuille(etat)?;
    Ok((Money::from_cents(p.valeur_cents), Money::from_cents(p.pv_cents)))
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

/// Recherche d'actions et d'ETF européens cotés en euros (par nom, mnémonique ou ISIN).
#[tauri::command]
pub async fn chercher_titres(etat: State<'_, Etat>, requete: String) -> Resultat<Vec<TitreDto>> {
    let q = requete.trim().to_uppercase();
    if q.len() < 2 {
        return Ok(Vec::new());
    }
    let isin = est_isin(&q).then(|| q.clone());
    Ok(rechercher_titres(&etat.http, requete.trim())
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

/// Historique et analyse technique d'un titre. `periode` : 3mo, 6mo, 1y, 2y ou 5y.
/// Les indicateurs sont calculés sur un historique plus long que la période affichée
/// (la moyenne 200 jours a besoin de 200 séances).
#[tauri::command]
pub async fn analyse_titre(etat: State<'_, Etat>, symbole: String, periode: String) -> Resultat<AnalyseDto> {
    let (telecharge, seances) = match periode.as_str() {
        "3mo" => ("2y", 63),
        "6mo" => ("2y", 126),
        "1y" => ("2y", 252),
        "2y" => ("5y", 504),
        _ => ("5y", 1_260),
    };
    let b = historique(&etat.http, &symbole, telecharge).await?;
    if b.len() < 30 {
        return Err(format!("{symbole} : historique trop court pour une analyse"));
    }
    let a = analyser(&b);
    let debut = b.len().saturating_sub(seances);
    let points = (debut..b.len())
        .map(|i| PointDto {
            t: b[i].t,
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
    Ok(AnalyseDto {
        symbole,
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
