//! Livrets réglementés : suivi des virements, conseils de dates, répartition entre livrets.
//! Pas d'établissement : un Livret A, un LDDS et un LEP au plus.

use escarcelle_core::conseil_livret::{conseil_retrait, conseil_versement, repartir_retrait, repartir_versement, LivretDetenu};
use escarcelle_core::livret::{simuler, Mouvement};
use escarcelle_core::params::{embarque, LivretParams};
use escarcelle_core::{Date, Money};
use serde::Serialize;
use tauri::State;

use crate::commands::aujourd_hui;
use crate::store::{Compte, Operation};
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub const TYPES: [(&str, &str); 3] = [("LEP", "LEP"), ("LIVRET_A", "Livret A"), ("LDDS", "LDDS")];

fn params(type_: &str) -> LivretParams {
    match type_ {
        "LEP" => embarque::lep(),
        "LDDS" => embarque::ldds(),
        _ => embarque::livret_a(),
    }
}

#[derive(Serialize)]
pub struct OperationDto {
    pub id: i64,
    pub date: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub montant_cents: i64,
}

#[derive(Serialize)]
pub struct LivretDto {
    #[serde(rename = "type")]
    pub type_: String,
    pub nom: String,
    pub compte_id: Option<i64>,
    pub solde_cents: i64,
    pub plafond_cents: i64,
    pub taux_micro: i64,
    pub interets_estimes_cents: i64,
    pub operations: Vec<OperationDto>,
}

/// Solde au jour (versements − retraits + intérêts capitalisés) et intérêts estimés de l'année.
pub fn valoriser(type_: &str, ops: &[Operation], jour: Date) -> Resultat<(Money, Money)> {
    let taux = params(type_).taux;
    let mut mvts = Vec::new();
    let mut net = Money::ZERO;
    for o in ops {
        let d: Date = o.date.parse().map_err(err)?;
        if d > jour {
            continue;
        }
        let m = Money::from_cents(o.montant_cents.unwrap_or(0));
        match o.type_.as_str() {
            "VERSEMENT" => {
                mvts.push(Mouvement::depot(d, m));
                net += m;
            }
            "RETRAIT" => {
                mvts.push(Mouvement::retrait(d, m));
                net -= m;
            }
            _ => {}
        }
    }
    let annees = simuler(&mvts, &taux, jour.year()).map_err(err)?;
    let capitalises: Money = annees.iter().filter(|a| a.annee < jour.year()).map(|a| a.interets).sum();
    let estimes = annees.iter().find(|a| a.annee == jour.year()).map(|a| a.interets).unwrap_or(Money::ZERO);
    Ok((net + capitalises, estimes))
}

pub fn tous(etat: &Etat) -> Resultat<Vec<LivretDto>> {
    let jour = aujourd_hui();
    let db = etat.db.lock().map_err(err)?;
    let comptes = db.comptes().map_err(err)?;
    let mut res = Vec::new();
    for (t, nom) in TYPES {
        let p = params(t);
        let taux = p.taux.at(jour).copied().unwrap_or_default();
        let compte = comptes.iter().find(|c| c.type_ == t);
        let (ops, solde, interets) = match compte {
            Some(c) => {
                let ops = db.operations(c.id).map_err(err)?;
                let (s, i) = valoriser(t, &ops, jour)?;
                (ops, s, i)
            }
            None => (Vec::new(), Money::ZERO, Money::ZERO),
        };
        res.push(LivretDto {
            type_: t.into(),
            nom: nom.into(),
            compte_id: compte.map(|c| c.id),
            solde_cents: solde.cents(),
            plafond_cents: p.plafond.cents(),
            taux_micro: taux.micro(),
            interets_estimes_cents: interets.cents(),
            operations: ops
                .into_iter()
                .filter(|o| o.type_ == "VERSEMENT" || o.type_ == "RETRAIT")
                .rev()
                .map(|o| OperationDto { id: o.id, date: o.date, type_: o.type_, montant_cents: o.montant_cents.unwrap_or(0) })
                .collect(),
        });
    }
    Ok(res)
}

#[tauri::command]
pub fn livrets(etat: State<'_, Etat>) -> Resultat<Vec<LivretDto>> {
    tous(&etat)
}

#[tauri::command]
pub fn ouvrir_livret(etat: State<'_, Etat>, type_livret: String) -> Resultat<Vec<LivretDto>> {
    let nom = TYPES.iter().find(|(t, _)| *t == type_livret).map(|(_, n)| *n).ok_or("type de livret inconnu")?;
    {
        let db = etat.db.lock().map_err(err)?;
        if !db.comptes().map_err(err)?.iter().any(|c| c.type_ == type_livret) {
            db.ajouter_compte(&Compte {
                id: 0,
                nom: nom.into(),
                type_: type_livret.clone(),
                etablissement: String::new(),
                produit: None,
                ouverture: aujourd_hui().to_string(),
            })
            .map_err(err)?;
        }
    }
    tous(&etat)
}

#[derive(Serialize)]
pub struct ConseilDto {
    pub sens: String,
    pub debut_quinzaine: String,
    pub fin_quinzaine: String,
    /// Versement : premier jour rémunéré. Retrait : premier jour non rémunéré.
    pub date_effet: String,
    /// Versement : date limite sans perte. Retrait : date conseillée si l'on peut attendre.
    pub date_conseillee: String,
    pub jours: i64,
    pub gain_cents: i64,
}

#[tauri::command]
pub fn conseil_livret(type_livret: String, sens: String, date: String, montant_cents: i64) -> Resultat<ConseilDto> {
    let d: Date = date.parse().map_err(err)?;
    let taux = params(&type_livret).taux.at(d).copied().unwrap_or_default();
    Ok(if sens == "RETRAIT" {
        let c = conseil_retrait(d, Money::from_cents(montant_cents.max(0)), taux);
        ConseilDto {
            sens,
            debut_quinzaine: c.debut_quinzaine.to_string(),
            fin_quinzaine: c.fin_quinzaine.to_string(),
            date_effet: c.non_remunere_depuis.to_string(),
            date_conseillee: c.date_conseillee.to_string(),
            jours: c.jours_perdus,
            gain_cents: c.gain_en_attendant.cents(),
        }
    } else {
        let c = conseil_versement(d);
        ConseilDto {
            sens,
            debut_quinzaine: c.debut_quinzaine.to_string(),
            fin_quinzaine: c.fin_quinzaine.to_string(),
            date_effet: c.remunere_a_partir_du.to_string(),
            date_conseillee: c.verser_au_plus_tard_le.to_string(),
            jours: c.jours_en_attente,
            gain_cents: 0,
        }
    })
}

#[derive(Serialize)]
pub struct RepartitionDto {
    pub parts: Vec<(String, i64)>,
    pub reste_cents: i64,
}

/// Où verser (meilleur taux d'abord, dans la limite des plafonds) ou où retirer (taux le plus bas d'abord).
#[tauri::command]
pub fn repartir_livrets(etat: State<'_, Etat>, sens: String, montant_cents: i64) -> Resultat<RepartitionDto> {
    let detenus: Vec<(String, LivretDetenu)> = tous(&etat)?
        .into_iter()
        .filter(|l| l.compte_id.is_some())
        .map(|l| {
            (
                l.nom.clone(),
                LivretDetenu {
                    nom: l.nom,
                    solde: Money::from_cents(l.solde_cents),
                    plafond: Money::from_cents(l.plafond_cents),
                    taux: escarcelle_core::Rate::from_micro(l.taux_micro),
                },
            )
        })
        .collect();
    let liste: Vec<LivretDetenu> = detenus.iter().map(|(_, l)| l.clone()).collect();
    let montant = Money::from_cents(montant_cents);
    let (parts, reste) = if sens == "RETRAIT" { repartir_retrait(&liste, montant) } else { repartir_versement(&liste, montant) };
    Ok(RepartitionDto {
        parts: detenus.into_iter().zip(parts).filter(|(_, m)| *m > Money::ZERO).map(|((n, _), m)| (n, m.cents())).collect(),
        reste_cents: reste.cents(),
    })
}
