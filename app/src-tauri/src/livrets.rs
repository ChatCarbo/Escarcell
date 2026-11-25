//! Livrets réglementés : suivi des virements, conseils de dates, répartition entre livrets.
//! Pas d'établissement : un Livret A, un LDDS et un LEP au plus.

use escarcelle_core::conseil_livret::{
    conseil_retrait, conseil_versement, repartir_retrait, repartir_versement, LivretDetenu,
};
use escarcelle_core::livret::{controler, simuler, Anomalie, Mouvement, Sens};
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

fn date_fr(d: Date) -> String {
    format!("{:02}/{:02}/{}", d.day(), d.month(), d.year())
}

/// Mouvements d'une liste d'opérations, avec l'identifiant de l'opération correspondante.
fn mouvements(ops: &[Operation]) -> Resultat<Vec<(i64, Mouvement)>> {
    let mut res = Vec::new();
    for o in ops {
        let d: Date = o.date.parse().map_err(err)?;
        let m = Money::from_cents(o.montant_cents.unwrap_or(0));
        match o.type_.as_str() {
            "VERSEMENT" => res.push((o.id, Mouvement::depot(d, m))),
            "RETRAIT" => res.push((o.id, Mouvement::retrait(d, m))),
            _ => {}
        }
    }
    Ok(res)
}

/// Anomalies (plafond dépassé, retrait supérieur au solde), chacune jugée à la date de son opération.
fn anomalies(type_: &str, ops: &[Operation]) -> Resultat<Vec<(i64, Mouvement, Anomalie)>> {
    let p = params(type_);
    let mvts = mouvements(ops)?;
    let seuls: Vec<Mouvement> = mvts.iter().map(|(_, m)| *m).collect();
    Ok(controler(&seuls, &p.taux, p.plafond)
        .map_err(err)?
        .into_iter()
        .map(|(i, a)| (mvts[i].0, mvts[i].1, a))
        .collect())
}

/// Vérifie qu'une modification (ajout ou suppression d'une opération) ne crée pas d'anomalie.
/// `nouvelle` est l'identifiant de l'opération ajoutée (0) ou `None` pour une suppression.
/// Les anomalies déjà présentes avant la modification ne bloquent pas : seules les nouvelles sont refusées.
pub fn verifier_modification(type_: &str, avant: &[Operation], apres: &[Operation], ajout: bool) -> Resultat<()> {
    let deja: Vec<i64> = anomalies(type_, avant)?.into_iter().map(|(id, _, _)| id).collect();
    let plafond = params(type_).plafond;
    for (id, m, a) in anomalies(type_, apres)? {
        if deja.contains(&id) {
            continue;
        }
        let quand = date_fr(m.date);
        let cause = match a {
            Anomalie::PlafondDepasse { marge } => format!(
                "le plafond de {plafond} serait dépassé au {quand} (versement maximum possible ce jour-là : {marge}, intérêts capitalisés compris)"
            ),
            Anomalie::SoldeInsuffisant { solde } => {
                format!("le solde au {quand} ({solde}) serait insuffisant pour ce retrait")
            }
        };
        return Err(if ajout && id == 0 {
            format!("Opération refusée : {cause}.")
        } else {
            let quoi = if m.sens == Sens::Depot { "versement" } else { "retrait" };
            let action = if ajout { "Avec cette opération" } else { "Sans cette opération" };
            format!("{action}, le {quoi} de {} du {quand} ne serait plus possible : {cause}.", m.montant)
        });
    }
    Ok(())
}

pub fn tous(etat: &Etat) -> Resultat<Vec<LivretDto>> {
    let jour = aujourd_hui();
    let db = etat.db();
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
                .map(|o| OperationDto {
                    id: o.id,
                    date: o.date,
                    type_: o.type_,
                    montant_cents: o.montant_cents.unwrap_or(0),
                })
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
        let db = etat.db();
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
    if montant_cents <= 0 {
        return Ok(RepartitionDto { parts: Vec::new(), reste_cents: 0 });
    }
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
    let (parts, reste) =
        if sens == "RETRAIT" { repartir_retrait(&liste, montant) } else { repartir_versement(&liste, montant) };
    Ok(RepartitionDto {
        parts: detenus
            .into_iter()
            .zip(parts)
            .filter(|(_, m)| *m > Money::ZERO)
            .map(|((n, _), m)| (n, m.cents()))
            .collect(),
        reste_cents: reste.cents(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(id: i64, date: &str, type_: &str, euros: i64) -> Operation {
        Operation {
            id,
            compte_id: 1,
            date: date.into(),
            type_: type_.into(),
            montant_cents: Some(euros * 100),
            isin: None,
            quantite_micro: None,
            cours_micro: None,
            frais_cents: 0,
            valeur_plan_cents: None,
            libelle: None,
        }
    }

    /// Cas signalé : 2 000 € versés le 03/05/2021, le solde d'aujourd'hui dépasse 2 200 € avec les intérêts.
    /// Un versement de 20 940 € daté du 18/05/2021 doit passer : le plafond se juge sur le solde de mai 2021.
    #[test]
    fn versement_antidate_juge_sur_le_solde_de_sa_date() {
        let avant = vec![op(1, "2021-05-03", "VERSEMENT", 2_000)];
        let mut apres = avant.clone();
        apres.push(op(0, "2021-05-18", "VERSEMENT", 20_940));
        assert!(verifier_modification("LIVRET_A", &avant, &apres, true).is_ok());
        apres[1].montant_cents = Some(20_951 * 100);
        let e = verifier_modification("LIVRET_A", &avant, &apres, true).unwrap_err();
        assert!(e.contains("plafond"), "{e}");
    }

    #[test]
    fn suppression_qui_rendrait_un_retrait_impossible() {
        let avant = vec![op(1, "2025-03-03", "VERSEMENT", 1_000), op(2, "2025-06-03", "RETRAIT", 800)];
        let apres = vec![avant[1].clone()];
        let e = verifier_modification("LDDS", &avant, &apres, false).unwrap_err();
        assert!(e.starts_with("Sans cette opération, le retrait"), "{e}");
        // Une anomalie déjà présente avant la modification ne bloque pas les autres saisies.
        let mut encore = apres.clone();
        encore.push(op(0, "2025-09-01", "VERSEMENT", 100));
        assert!(verifier_modification("LDDS", &apres, &encore, true).is_ok());
    }
}
