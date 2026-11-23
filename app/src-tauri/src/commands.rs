//! Commandes appelées par l'interface. Tous les calculs passent par `escarcelle-core`.
//! Les montants échangés avec l'interface sont en centimes, les taux en millionièmes.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use escarcelle_core::comparateur::{comparer, Hypotheses};
use escarcelle_core::impot::{calculer_ir, Foyer, Situation};
use escarcelle_core::params::embarque;
use escarcelle_core::per::economie_impot;
use escarcelle_core::{Date, Money, Rate};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::catalog::Catalogue;
use crate::store::{Compte, Operation};
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Date du jour (UTC), sans dépendance externe.
pub fn aujourd_hui() -> Date {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    Date::new(y as i32, m as u8, d as u8).expect("date système valide")
}

fn date(s: &str) -> Resultat<Date> {
    s.parse().map_err(err)
}

// ---------- Catalogue, comptes, opérations ----------

#[tauri::command]
pub fn etablissements(etat: State<'_, Etat>) -> Catalogue {
    etat.catalogue.clone()
}

#[tauri::command]
pub fn comptes(etat: State<'_, Etat>) -> Resultat<Vec<Compte>> {
    etat.db.lock().map_err(err)?.comptes().map_err(err)
}

#[tauri::command]
pub fn ajouter_compte(etat: State<'_, Etat>, compte: Compte) -> Resultat<i64> {
    date(&compte.ouverture)?;
    etat.db.lock().map_err(err)?.ajouter_compte(&compte).map_err(err)
}

#[tauri::command]
pub fn operations(etat: State<'_, Etat>, compte_id: i64) -> Resultat<Vec<Operation>> {
    etat.db.lock().map_err(err)?.operations(compte_id).map_err(err)
}

#[tauri::command]
pub fn ajouter_operation(etat: State<'_, Etat>, operation: Operation) -> Resultat<i64> {
    date(&operation.date)?;
    etat.db.lock().map_err(err)?.ajouter_operation(&operation).map_err(err)
}

#[tauri::command]
pub fn supprimer_operation(etat: State<'_, Etat>, id: i64) -> Resultat<()> {
    etat.db.lock().map_err(err)?.supprimer_operation(id).map_err(err)
}

#[tauri::command]
pub fn enregistrer_cours(etat: State<'_, Etat>, isin: String, date_cours: String, cours_micro: i64, source: String) -> Resultat<()> {
    date(&date_cours)?;
    etat.db.lock().map_err(err)?.enregistrer_cours(&isin, &date_cours, cours_micro, &source).map_err(err)
}

// ---------- Valorisation des comptes ----------

#[derive(Serialize)]
pub struct LigneCompte {
    pub id: i64,
    pub nom: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub etablissement: String,
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
    pub par_enveloppe: Vec<Part>,
    pub par_etablissement: Vec<Part>,
    pub comptes: Vec<LigneCompte>,
    pub alertes: Vec<String>,
}

fn libelle_enveloppe(t: &str) -> &'static str {
    match t {
        "LIVRET_A" => "Livret A",
        "LDDS" => "LDDS",
        "LEP" => "LEP",
        "PEA" | "PEA_PME" => "PEA",
        "PER" => "PER",
        _ => "Autre",
    }
}

fn couleur_enveloppe(t: &str) -> &'static str {
    match t {
        "LIVRET_A" => "#38BDF8",
        "LDDS" => "#0284C7",
        "LEP" => "#34D399",
        "PEA" | "PEA_PME" => "#10B981",
        "PER" => "#FBBF24",
        _ => "#94A3B8",
    }
}

#[tauri::command]
pub fn tableau_de_bord(etat: State<'_, Etat>) -> Resultat<TableauDeBord> {
    let jour = aujourd_hui();
    let comptes = etat.db.lock().map_err(err)?.comptes().map_err(err)?;
    let alertes: Vec<String> = Vec::new();
    let mut lignes = Vec::new();
    let (mut livrets, mut interets, mut pv_latente) = (Money::ZERO, Money::ZERO, Money::ZERO);
    let mut par_env: BTreeMap<&'static str, (Money, &'static str)> = BTreeMap::new();
    let mut par_etab: BTreeMap<String, Money> = BTreeMap::new();

    for c in &comptes {
        let ops = etat.db.lock().map_err(err)?.operations(c.id).map_err(err)?;
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
        *par_etab.entry(c.etablissement.clone()).or_insert(Money::ZERO) += valeur;
        lignes.push(LigneCompte {
            id: c.id,
            nom: c.nom.clone(),
            type_: c.type_.clone(),
            etablissement: c.etablissement.clone(),
            valeur_cents: valeur.cents(),
            complement_cents: complement.cents(),
        });
    }

    // PEA : suivi de lignes, sans établissement.
    let (valeur_pea, pv_pea) = crate::pea::totaux(&etat)?;
    if valeur_pea > Money::ZERO {
        pv_latente += pv_pea;
        par_env.insert("PEA", (valeur_pea, couleur_enveloppe("PEA")));
        *par_etab.entry("non_precise".into()).or_insert(Money::ZERO) += valeur_pea;
        lignes.push(LigneCompte {
            id: -1,
            nom: "PEA".into(),
            type_: "PEA".into(),
            etablissement: String::new(),
            valeur_cents: valeur_pea.cents(),
            complement_cents: pv_pea.cents(),
        });
    }

    let total: Money = lignes.iter().map(|l| Money::from_cents(l.valeur_cents)).sum();
    let mut par_enveloppe: Vec<Part> = par_env
        .into_iter()
        .map(|(lib, (m, col))| Part { cle: lib.into(), libelle: lib.into(), montant_cents: m.cents(), couleur: Some(col.into()) })
        .collect();
    par_enveloppe.sort_by(|a, b| b.montant_cents.cmp(&a.montant_cents));
    let mut par_etablissement: Vec<Part> = par_etab
        .into_iter()
        .map(|(id, m)| Part {
            libelle: etat
                .catalogue
                .etablissements
                .iter()
                .find(|e| e.id == id)
                .map(|e| e.nom.clone())
                .unwrap_or_else(|| if id == "non_precise" { "Non précisé (PEA)".into() } else { id.clone() }),
            couleur: etat.catalogue.couleur(&id).map(String::from),
            cle: id,
            montant_cents: m.cents(),
        })
        .collect();
    par_etablissement.sort_by(|a, b| b.montant_cents.cmp(&a.montant_cents));

    Ok(TableauDeBord {
        date: jour.to_string(),
        total_cents: total.cents(),
        livrets_cents: livrets.cents(),
        interets_estimes_cents: interets.cents(),
        pv_latente_cents: pv_latente.cents(),
        par_enveloppe,
        par_etablissement,
        comptes: lignes,
        alertes,
    })
}

// ---------- Impôt et PER ----------

#[derive(Deserialize)]
pub struct FoyerDto {
    pub couple: bool,
    pub enfants: u32,
}

impl FoyerDto {
    fn foyer(&self) -> Foyer {
        Foyer {
            situation: if self.couple { Situation::Couple } else { Situation::Seul },
            enfants: self.enfants,
            parent_isole: false,
        }
    }
}

#[derive(Serialize)]
pub struct ImpotDto {
    pub parts_demi: u32,
    pub impot_cents: i64,
    pub decote_cents: i64,
    pub plafonnement: bool,
    pub tmi_micro: i64,
    pub taux_marginal_effectif_micro: i64,
    pub taux_moyen_micro: i64,
}

#[tauri::command]
pub fn impot(revenu_cents: i64, foyer: FoyerDto) -> Resultat<ImpotDto> {
    let r = calculer_ir(Money::from_cents(revenu_cents), &foyer.foyer(), &embarque::ir_revenus_2025()).map_err(err)?;
    Ok(ImpotDto {
        parts_demi: r.demi_parts,
        impot_cents: r.impot.cents(),
        decote_cents: r.decote.cents(),
        plafonnement: r.plafonnement_applique,
        tmi_micro: r.tmi.micro(),
        taux_marginal_effectif_micro: r.taux_marginal_effectif.micro(),
        taux_moyen_micro: r.taux_moyen.micro(),
    })
}

#[derive(Serialize)]
pub struct EconomieDto {
    pub deductible_cents: i64,
    pub economie_cents: i64,
    pub taux_micro: i64,
    pub bas_de_tranche_cents: i64,
    pub economie_bas_de_tranche_cents: i64,
}

#[tauri::command]
pub fn economie_per(revenu_cents: i64, foyer: FoyerDto, versement_cents: i64, plafond_cents: i64) -> Resultat<EconomieDto> {
    let p = embarque::ir_revenus_2025();
    let f = foyer.foyer();
    let revenu = Money::from_cents(revenu_cents);
    let e = economie_impot(revenu, &f, &p, Money::from_cents(versement_cents), Money::from_cents(plafond_cents)).map_err(err)?;
    let b = escarcelle_core::per::verser_jusqu_au_bas_de_tranche(revenu, &f, &p).map_err(err)?;
    Ok(EconomieDto {
        deductible_cents: e.deductible.cents(),
        economie_cents: e.economie.cents(),
        taux_micro: e.taux.micro(),
        bas_de_tranche_cents: b.versement.cents(),
        economie_bas_de_tranche_cents: b.economie.cents(),
    })
}

#[derive(Deserialize)]
pub struct HypothesesDto {
    pub versement_initial_cents: i64,
    pub versement_mensuel_cents: i64,
    pub annees: u32,
    pub part_fonds_euros_micro: i64,
    pub rendement_brut_uc_micro: i64,
    pub ter_uc_micro: i64,
}

#[derive(Serialize)]
pub struct ResultatComparaisonDto {
    pub nom: String,
    pub capital_final_cents: i64,
    pub versements_cents: i64,
    pub frais_totaux_cents: i64,
    pub ecart_cents: i64,
    pub plafonnee: bool,
    pub part_fonds_euros_micro: i64,
}

#[tauri::command]
pub fn comparer_per(etat: State<'_, Etat>, hypotheses: HypothesesDto) -> Vec<ResultatComparaisonDto> {
    let contrats: Vec<_> = etat.catalogue.produits("PER").filter_map(|(_, p)| p.contrat_comparable("2025")).collect();
    let h = Hypotheses {
        versement_initial: Money::from_cents(hypotheses.versement_initial_cents),
        versement_mensuel: Money::from_cents(hypotheses.versement_mensuel_cents),
        annees: hypotheses.annees.clamp(1, 50),
        part_fonds_euros: Rate::from_micro(hypotheses.part_fonds_euros_micro),
        rendement_brut_uc: Rate::from_micro(hypotheses.rendement_brut_uc_micro),
        ter_uc: Rate::from_micro(hypotheses.ter_uc_micro),
    };
    comparer(&contrats, &h)
        .into_iter()
        .map(|r| ResultatComparaisonDto {
            nom: r.nom,
            capital_final_cents: r.capital_final.cents(),
            versements_cents: r.versements.cents(),
            frais_totaux_cents: r.frais_totaux.cents(),
            ecart_cents: r.ecart_avec_meilleur.cents(),
            plafonnee: r.plafonnee,
            part_fonds_euros_micro: r.part_fonds_euros.micro(),
        })
        .collect()
}
