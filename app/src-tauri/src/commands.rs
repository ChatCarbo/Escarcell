//! Commandes générales appelées par l'interface : virements des livrets et tableau de bord.
//! Tous les calculs passent par `escarcelle-core`. Montants en centimes, taux en millionièmes.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use escarcelle_core::{Date, Money};
use serde::Serialize;
use tauri::State;

use crate::store::Operation;
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Date du jour (UTC), sans dépendance externe.
pub fn aujourd_hui() -> Date {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    Date::depuis_jours_unix(secs.div_euclid(86_400))
}

fn date(s: &str) -> Resultat<Date> {
    s.parse().map_err(err)
}

// ---------- Virements des livrets ----------

#[tauri::command]
pub fn ajouter_operation(etat: State<'_, Etat>, mut operation: Operation) -> Resultat<i64> {
    let d = date(&operation.date)?;
    let debut = escarcelle_core::params::embarque::debut();
    if d < debut {
        return Err(format!(
            "Escarcelle connaît les taux des livrets depuis le {:02}/{:02}/{} : saisissez le solde à cette date plutôt que les opérations plus anciennes.",
            debut.day(),
            debut.month(),
            debut.year()
        ));
    }
    if d.jours_unix() > aujourd_hui().jours_unix() + 366 {
        return Err("La date est à plus d'un an dans le futur.".into());
    }
    operation.date = d.to_string();
    if operation.type_ != "VERSEMENT" && operation.type_ != "RETRAIT" {
        return Err(format!("type d'opération inconnu : {}", operation.type_));
    }
    if !operation.montant_cents.is_some_and(|m| m > 0) {
        return Err("Le montant doit être positif.".into());
    }
    etat.db().ajouter_operation(&operation).map_err(err)
}

#[tauri::command]
pub fn supprimer_operation(etat: State<'_, Etat>, id: i64) -> Resultat<()> {
    etat.db().supprimer_operation(id).map_err(err)
}

// ---------- Tableau de bord ----------

#[derive(Serialize)]
pub struct LigneCompte {
    pub id: i64,
    pub nom: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub valeur_cents: i64,
    /// Intérêts estimés de l'année (livrets) ou plus-value latente (PEA).
    pub complement_cents: i64,
}

#[derive(Serialize)]
pub struct Part {
    pub cle: String,
    pub libelle: String,
    pub montant_cents: i64,
    pub couleur: Option<String>,
}

#[derive(Serialize)]
pub struct TableauDeBord {
    pub date: String,
    pub total_cents: i64,
    pub livrets_cents: i64,
    pub interets_estimes_cents: i64,
    pub pv_latente_cents: i64,
    /// Dividendes annuels estimés du PEA, nets de retenues.
    pub dividendes_nets_cents: i64,
    pub par_enveloppe: Vec<Part>,
    pub comptes: Vec<LigneCompte>,
    pub alertes: Vec<crate::alertes::AlerteDto>,
}

fn libelle_enveloppe(t: &str) -> &'static str {
    match t {
        "LIVRET_A" => "Livret A",
        "LDDS" => "LDDS",
        "LEP" => "LEP",
        "PEA" => "PEA",
        _ => "Autre",
    }
}

fn couleur_enveloppe(t: &str) -> &'static str {
    match t {
        "LIVRET_A" => "#38BDF8",
        "LDDS" => "#0284C7",
        "LEP" => "#34D399",
        "PEA" => "#10B981",
        _ => "#94A3B8",
    }
}

#[tauri::command]
pub fn tableau_de_bord(etat: State<'_, Etat>) -> Resultat<TableauDeBord> {
    let jour = aujourd_hui();
    let comptes = etat.db().comptes().map_err(err)?;
    let mut lignes = Vec::new();
    let (mut livrets, mut interets, mut pv_latente) = (Money::ZERO, Money::ZERO, Money::ZERO);
    let mut par_env: BTreeMap<&'static str, (Money, &'static str)> = BTreeMap::new();

    for c in &comptes {
        let ops = etat.db().operations(c.id).map_err(err)?;
        let (valeur, complement) = match c.type_.as_str() {
            "LIVRET_A" | "LDDS" | "LEP" => {
                let (v, i) = crate::livrets::valoriser(&c.type_, &ops, jour)?;
                livrets += v;
                interets += i;
                (v, i)
            }
            _ => continue,
        };
        let e = par_env.entry(libelle_enveloppe(&c.type_)).or_insert((Money::ZERO, couleur_enveloppe(&c.type_)));
        e.0 += valeur;
        lignes.push(LigneCompte {
            id: c.id,
            nom: c.nom.clone(),
            type_: c.type_.clone(),
            valeur_cents: valeur.cents(),
            complement_cents: complement.cents(),
        });
    }

    // PEA : suivi de lignes, sans établissement.
    let (valeur_pea, pv_pea, dividendes_nets) = crate::portefeuille::totaux(&etat)?;
    if valeur_pea > Money::ZERO {
        pv_latente += pv_pea;
        par_env.insert(libelle_enveloppe("PEA"), (valeur_pea, couleur_enveloppe("PEA")));
        lignes.push(LigneCompte {
            // Identifiant négatif : il ne peut pas se confondre avec un compte de livret (l'interface s'en sert
            // comme clé de liste).
            id: -1,
            nom: "PEA".into(),
            type_: "PEA".into(),
            valeur_cents: valeur_pea.cents(),
            complement_cents: pv_pea.cents(),
        });
    }
    let alertes = crate::alertes::calculer(&etat)?;

    let total: Money = lignes.iter().map(|l| Money::from_cents(l.valeur_cents)).sum();
    let mut par_enveloppe: Vec<Part> = par_env
        .into_iter()
        .map(|(lib, (m, col))| Part {
            cle: lib.into(),
            libelle: lib.into(),
            montant_cents: m.cents(),
            couleur: Some(col.into()),
        })
        .collect();
    par_enveloppe.sort_by_key(|p| std::cmp::Reverse(p.montant_cents));

    Ok(TableauDeBord {
        date: jour.to_string(),
        total_cents: total.cents(),
        livrets_cents: livrets.cents(),
        interets_estimes_cents: interets.cents(),
        pv_latente_cents: pv_latente.cents(),
        dividendes_nets_cents: dividendes_nets.cents(),
        par_enveloppe,
        comptes: lignes,
        alertes,
    })
}
