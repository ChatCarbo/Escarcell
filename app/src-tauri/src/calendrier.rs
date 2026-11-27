//! Calendrier des dividendes du PEA (0.4, lot 4) : dividendes encaissés (opérations), détachements à venir
//! (annoncés ou estimés) et détachements à rapprocher, c'est-à-dire déjà détachés mais sans opération
//! enregistrée.

use std::collections::BTreeSet;

use escarcell_core::calendrier::{non_rapproches, par_mois, prevus, Categorie, Statut};
use escarcell_core::dividendes::{dividende_net, Versement};
use escarcell_core::livret::MONTANT_MAX;
use escarcell_core::{Date, Money, Price, Rate};
use serde::Serialize;
use tauri::State;

use crate::commands::aujourd_hui;
use crate::portefeuille::{pays_ligne, retenue};
use crate::store::{DividendePercu, Position, Store};
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Un détachement est proposé à l'enregistrement 15 jours après sa date (le temps du paiement), et au plus
/// 90 jours après (au-delà, on ne sait plus combien de titres étaient détenus).
const RAPPROCHEMENT_APRES: i64 = 15;
const RAPPROCHEMENT_JUSQU_A: i64 = 90;

#[derive(Clone, Debug, Serialize)]
pub struct AVenirDto {
    pub date: String,
    pub symbole: String,
    pub nom: String,
    /// « annonce » ou « estime ».
    pub statut: &'static str,
    pub par_action_micro: i64,
    pub quantite_micro: i64,
    pub brut_cents: i64,
    pub retenue_cents: i64,
    pub net_cents: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct PercuDto {
    pub id: i64,
    pub date: String,
    pub nom: String,
    pub net_cents: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct MoisDto {
    pub mois: u8,
    pub percu_cents: i64,
    pub annonce_cents: i64,
    pub estime_cents: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct CalendrierDto {
    /// Détachements des 12 prochains mois.
    pub a_venir: Vec<AVenirDto>,
    pub total_a_venir_net_cents: i64,
    /// Détachements passés sans opération : montant net prérempli (quantité détenue aujourd'hui).
    pub a_rapprocher: Vec<AVenirDto>,
    /// Dividendes encaissés (opérations), du plus récent au plus ancien.
    pub percus: Vec<PercuDto>,
    pub annee: i32,
    pub annees: Vec<i32>,
    /// Par mois : perçu (dividendes enregistrés, et détachements des 12 derniers mois pas encore enregistrés),
    /// annoncé, estimé.
    pub mois: Vec<MoisDto>,
}

/// Le dividende enregistré correspond-il à cette ligne (même ISIN ou même symbole) ?
fn de_la_ligne(d: &DividendePercu, p: &Position) -> bool {
    (p.isin.is_some() && d.isin == p.isin) || d.symbole.as_deref() == Some(p.symbole.as_str())
}

fn historique(db: &Store, symbole: &str) -> Resultat<Vec<Versement>> {
    Ok(db
        .dividendes(symbole)
        .map_err(err)?
        .into_iter()
        .filter_map(|(d, m)| Some(Versement { date: d.parse::<Date>().ok()?, montant: Price::from_micro(m) }))
        .collect())
}

fn element(p: &Position, date: Date, par_action: Price, statut: &'static str, taux: Rate) -> AVenirDto {
    let brut = par_action.valeur(p.quantite_micro);
    let net = dividende_net(brut, taux);
    AVenirDto {
        date: date.to_string(),
        symbole: p.symbole.clone(),
        nom: p.nom.clone(),
        statut,
        par_action_micro: par_action.micro(),
        quantite_micro: p.quantite_micro,
        brut_cents: brut.cents(),
        retenue_cents: net.retenue_etrangere.cents(),
        net_cents: net.net.cents(),
    }
}

/// Calendrier à la date `jour`, avec la répartition par mois de l'année `annee`.
pub fn calculer(db: &Store, jour: Date, annee: Option<i32>) -> Resultat<CalendrierDto> {
    let positions: Vec<Position> = db.positions().map_err(err)?.into_iter().filter(|p| p.quantite_micro > 0).collect();
    let percus = db.dividendes_percus().map_err(err)?;
    let ignores: BTreeSet<(String, String)> = db.dividendes_ignores().map_err(err)?.into_iter().collect();
    let horizon = jour.plus_annees(1);
    let (mut a_venir, mut a_rapprocher) = (Vec::new(), Vec::new());
    // Détachements des 12 derniers mois sans opération enregistrée : comptés comme perçus dans la vue par
    // mois (quantité détenue aujourd'hui, comme le total « 12 derniers mois » de la page PEA).
    let mut detaches_12_mois: Vec<(Date, Money)> = Vec::new();
    for p in &positions {
        let h = historique(db, &p.symbole)?;
        let taux = match pays_ligne(p) {
            Some(c) => retenue(db, &c)?.unwrap_or(Rate::ZERO),
            None => Rate::ZERO,
        };
        for v in prevus(&h, jour, horizon) {
            let statut = match v.statut {
                Statut::Annonce => "annonce",
                Statut::Estime => "estime",
            };
            a_venir.push(element(p, v.date, v.par_action, statut, taux));
        }
        // Détachements de 15 à 90 jours, sans opération de dividende de cette ligne qui les explique.
        let fenetre = |v: &&Versement| {
            let j = jour.jours_unix() - v.date.jours_unix();
            (RAPPROCHEMENT_APRES..=RAPPROCHEMENT_JUSQU_A).contains(&j)
        };
        let detaches: Vec<&Versement> = h.iter().filter(fenetre).filter(|v| v.montant.micro() > 0).collect();
        let operations: Vec<Date> =
            percus.iter().filter(|d| de_la_ligne(d, p)).filter_map(|d| d.date.parse().ok()).collect();
        let recents: Vec<&Versement> =
            h.iter().filter(|v| v.date > jour.plus_annees(-1) && v.date <= jour && v.montant.micro() > 0).collect();
        let dates_recentes: Vec<Date> = recents.iter().map(|v| v.date).collect();
        for d in non_rapproches(&dates_recentes, &operations) {
            if let Some(v) = recents.iter().find(|v| v.date == d) {
                detaches_12_mois.push((d, Money::from_cents(element(p, d, v.montant, "", taux).net_cents)));
            }
        }
        let dates: Vec<Date> = detaches.iter().map(|v| v.date).collect();
        for d in non_rapproches(&dates, &operations) {
            if ignores.contains(&(p.symbole.clone(), d.to_string())) {
                continue;
            }
            if let Some(v) = detaches.iter().find(|v| v.date == d) {
                a_rapprocher.push(element(p, d, v.montant, "a_rapprocher", taux));
            }
        }
    }
    a_venir.sort_by(|a, b| (&a.date, &a.nom).cmp(&(&b.date, &b.nom)));
    a_rapprocher.sort_by(|a, b| (&a.date, &a.nom).cmp(&(&b.date, &b.nom)));

    let nom_de = |d: &DividendePercu| {
        positions
            .iter()
            .find(|p| de_la_ligne(d, p))
            .map(|p| p.nom.clone())
            .or(d.libelle.clone())
            .or(d.isin.clone())
            .unwrap_or_else(|| "Dividende".into())
    };
    let annee = annee.unwrap_or(jour.year());
    let mut montants = Vec::new();
    for d in &percus {
        if let Ok(date) = d.date.parse::<Date>() {
            montants.push((date, Money::from_cents(d.montant_cents), Categorie::Percu));
        }
    }
    for (d, m) in &detaches_12_mois {
        montants.push((*d, *m, Categorie::Percu));
    }
    for a in &a_venir {
        if let Ok(date) = a.date.parse::<Date>() {
            let c = if a.statut == "annonce" { Categorie::Annonce } else { Categorie::Estime };
            montants.push((date, Money::from_cents(a.net_cents), c));
        }
    }
    let mois = par_mois(annee, &montants)
        .iter()
        .enumerate()
        .map(|(i, m)| MoisDto {
            mois: i as u8 + 1,
            percu_cents: m[0].cents(),
            annonce_cents: m[1].cents(),
            estime_cents: m[2].cents(),
        })
        .collect();
    let mut annees: BTreeSet<i32> = montants.iter().map(|(d, _, _)| d.year()).collect();
    annees.insert(jour.year());
    annees.insert(annee);
    Ok(CalendrierDto {
        total_a_venir_net_cents: a_venir.iter().map(|a| a.net_cents).sum(),
        a_venir,
        a_rapprocher,
        percus: percus
            .iter()
            .rev()
            .map(|d| PercuDto { id: d.id, date: d.date.clone(), nom: nom_de(d), net_cents: d.montant_cents })
            .collect(),
        annee,
        annees: annees.into_iter().collect(),
        mois,
    })
}

#[tauri::command]
pub fn calendrier_dividendes(etat: State<'_, Etat>, annee: Option<i32>) -> Resultat<CalendrierDto> {
    calculer(&etat.db(), aujourd_hui(), annee)
}

/// Enregistre un dividende encaissé sur une ligne du PEA (montant net crédité, modifiable).
#[tauri::command]
pub fn enregistrer_dividende(etat: State<'_, Etat>, symbole: String, date: String, montant_cents: i64) -> Resultat<()> {
    enregistrer(&mut etat.db(), &symbole, &date, montant_cents, aujourd_hui())
}

fn enregistrer(db: &mut Store, symbole: &str, date: &str, montant_cents: i64, jour: Date) -> Resultat<()> {
    let d: Date = date.parse().map_err(|_| "Date invalide.".to_string())?;
    if d > jour {
        return Err("Un dividende encaissé ne peut pas être daté dans le futur.".into());
    }
    if montant_cents <= 0 || montant_cents > MONTANT_MAX.cents() {
        return Err("Le montant doit être positif (10 000 000 € au plus).".into());
    }
    let p =
        db.positions().map_err(err)?.into_iter().find(|p| p.symbole == symbole).ok_or("Ligne du PEA introuvable.")?;
    db.ajouter_dividende(&d.to_string(), p.isin.as_deref(), &p.symbole, montant_cents, &p.nom).map_err(err)?;
    Ok(())
}

/// Ne plus proposer ce détachement.
#[tauri::command]
pub fn ignorer_dividende(etat: State<'_, Etat>, symbole: String, date: String) -> Resultat<()> {
    let d: Date = date.parse().map_err(|_| "Date invalide.".to_string())?;
    etat.db().ignorer_dividende(&symbole, &d.to_string()).map_err(err)
}

#[tauri::command]
pub fn supprimer_dividende(etat: State<'_, Etat>, id: i64) -> Resultat<()> {
    etat.db().supprimer_dividende(id).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jour() -> Date {
        "2026-11-27".parse().unwrap()
    }

    fn base() -> Store {
        let db = Store::en_memoire().unwrap();
        db.ajouter_position(&Position {
            id: 0,
            isin: Some("FR0000120271".into()),
            symbole: "TTE.PA".into(),
            nom: "TotalEnergies".into(),
            quantite_micro: 50_000_000,
            pru_micro: 35_070_000,
            pays: Some("FR".into()),
            journal: false,
        })
        .unwrap();
        db
    }

    fn historique_tte(db: &mut Store) {
        let h: Vec<(String, i64)> = [
            ("2025-09-30", 850_000),
            ("2025-12-31", 850_000),
            ("2026-03-31", 850_000),
            ("2026-06-30", 850_000),
            ("2026-09-30", 900_000),
        ]
        .iter()
        .map(|(d, m)| (d.to_string(), *m))
        .collect();
        db.remplacer_dividendes("TTE.PA", &h).unwrap();
    }

    #[test]
    fn a_venir_rapprochement_puis_enregistrement() {
        let mut db = base();
        historique_tte(&mut db);
        let c = calculer(&db, jour(), None).unwrap();
        // Quatre acomptes estimés à 0,90 € × 50 = 45 € (société française : pas de retenue).
        assert_eq!(c.a_venir.len(), 4);
        assert!(c.a_venir.iter().all(|a| a.statut == "estime" && a.net_cents == 4_500));
        assert_eq!(c.a_venir[0].date, "2026-12-31");
        assert_eq!(c.total_a_venir_net_cents, 18_000);
        // Détachement du 30/09/2026 (58 jours) : à rapprocher. Celui du 30/06 (150 jours) : trop ancien.
        assert_eq!(c.a_rapprocher.len(), 1);
        assert_eq!((c.a_rapprocher[0].date.as_str(), c.a_rapprocher[0].net_cents), ("2026-09-30", 4_500));
        assert_eq!(c.mois[11].estime_cents, 4_500);
        // Détachés en 2026 (mars, juin, septembre) : perçus, même sans opération enregistrée.
        assert_eq!((c.mois[2].percu_cents, c.mois[5].percu_cents, c.mois[8].percu_cents), (4_250, 4_250, 4_500));

        enregistrer(&mut db, "TTE.PA", "2026-10-02", 4_500, jour()).unwrap();
        let c = calculer(&db, jour(), None).unwrap();
        assert!(c.a_rapprocher.is_empty());
        assert_eq!(c.percus.len(), 1);
        assert_eq!(c.percus[0].nom, "TotalEnergies");
        // L'opération du 02/10 remplace le détachement du 30/09 (pas de double compte).
        assert_eq!((c.mois[8].percu_cents, c.mois[9].percu_cents), (0, 4_500));

        db.supprimer_dividende(c.percus[0].id).unwrap();
        db.ignorer_dividende("TTE.PA", "2026-09-30").unwrap();
        let c = calculer(&db, jour(), None).unwrap();
        assert!(c.a_rapprocher.is_empty() && c.percus.is_empty());
        let c = calculer(&db, jour(), Some(2027)).unwrap();
        assert_eq!(c.mois[2].estime_cents, 4_500);
        // 2025 : acompte détaché le 31/12/2025, dans les 12 derniers mois.
        assert_eq!(c.annees, vec![2025, 2026, 2027]);
    }

    #[test]
    fn controles_d_enregistrement() {
        let mut db = base();
        assert!(enregistrer(&mut db, "TTE.PA", "2026-12-01", 100, jour()).is_err());
        assert!(enregistrer(&mut db, "TTE.PA", "2026-10-01", 0, jour()).is_err());
        assert!(enregistrer(&mut db, "AI.PA", "2026-10-01", 100, jour()).is_err());
        assert!(enregistrer(&mut db, "TTE.PA", "01/10/2026", 100, jour()).is_err());
        enregistrer(&mut db, "TTE.PA", "2026-10-01", 100, jour()).unwrap();
        // Le compte PEA est ouvert au besoin ; le dividende ne touche pas aux lignes.
        assert_eq!(db.comptes().unwrap().iter().filter(|c| c.type_ == "PEA").count(), 1);
        assert_eq!(db.positions().unwrap()[0].quantite_micro, 50_000_000);
    }

    #[test]
    fn retenue_etrangere_deduite() {
        let mut db = Store::en_memoire().unwrap();
        db.ajouter_position(&Position {
            id: 0,
            isin: Some("NL0000235190".into()),
            symbole: "AIR.PA".into(),
            nom: "Airbus".into(),
            quantite_micro: 10_000_000,
            pru_micro: 100_000_000,
            pays: Some("NL".into()),
            journal: false,
        })
        .unwrap();
        db.remplacer_dividendes("AIR.PA", &[("2026-04-20".into(), 2_000_000)]).unwrap();
        let c = calculer(&db, jour(), None).unwrap();
        // 10 × 2 € = 20 € brut, retenue néerlandaise 15 % : 17 € net.
        assert_eq!((c.a_venir[0].brut_cents, c.a_venir[0].retenue_cents, c.a_venir[0].net_cents), (2_000, 300, 1_700));
    }
}
