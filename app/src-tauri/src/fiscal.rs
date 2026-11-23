//! Simulation fiscale : impôt, intérêt d'un versement PER, réduction pour dons.

use escarcelle_core::dons::{reduction_dons, DonsParams};
use escarcelle_core::impot::{calculer_ir, Foyer, Situation};
use escarcelle_core::params::embarque;
use escarcelle_core::per::{interet_versement, verser_jusqu_au_bas_de_tranche, Verdict};
use escarcelle_core::{Money, Rate};
use serde::{Deserialize, Serialize};

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Deserialize)]
pub struct Saisie {
    pub revenu_cents: i64,
    pub couple: bool,
    pub enfants: u32,
    pub versement_per_cents: i64,
    /// Plafond « épargne retraite » de l'avis d'imposition ; si absent, le versement est supposé entièrement déductible.
    pub plafond_per_cents: Option<i64>,
    pub tmi_retraite_micro: i64,
    pub dons_aide_personnes_cents: i64,
    pub dons_interet_general_cents: i64,
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

#[derive(Serialize)]
pub struct PerDto {
    pub deductible_cents: i64,
    pub economie_cents: i64,
    pub taux_economie_micro: i64,
    pub impot_sortie_cents: i64,
    pub gain_net_cents: i64,
    /// "aucun", "defavorable", "limite", "interessant"
    pub verdict: String,
    pub bas_de_tranche_cents: i64,
    pub economie_bas_de_tranche_cents: i64,
}

#[derive(Serialize)]
pub struct DonsDto {
    pub reduction_cents: i64,
    pub perdue_cents: i64,
    pub excedent_reportable_cents: i64,
    pub cout_reel_cents: i64,
}

#[derive(Serialize)]
pub struct SimulationDto {
    pub impot: ImpotDto,
    pub per: Option<PerDto>,
    pub dons: Option<DonsDto>,
    pub impot_apres_per_cents: i64,
    pub impot_final_cents: i64,
    pub economie_totale_cents: i64,
}

#[tauri::command]
pub fn simulation_fiscale(saisie: Saisie) -> Resultat<SimulationDto> {
    let p = embarque::ir_revenus_2025();
    let foyer = Foyer {
        situation: if saisie.couple { Situation::Couple } else { Situation::Seul },
        enfants: saisie.enfants,
        parent_isole: false,
    };
    let revenu = Money::from_cents(saisie.revenu_cents.max(0));
    let ir = calculer_ir(revenu, &foyer, &p).map_err(err)?;

    let mut revenu_apres_per = revenu;
    let mut impot_apres_per = ir.impot;
    let per = if saisie.versement_per_cents > 0 {
        let versement = Money::from_cents(saisie.versement_per_cents);
        let plafond = saisie.plafond_per_cents.map(Money::from_cents).unwrap_or(versement);
        let i = interet_versement(revenu, &foyer, &p, versement, plafond, Rate::from_micro(saisie.tmi_retraite_micro)).map_err(err)?;
        let b = verser_jusqu_au_bas_de_tranche(revenu, &foyer, &p).map_err(err)?;
        revenu_apres_per = revenu - i.economie.deductible;
        impot_apres_per = i.economie.impot_apres;
        Some(PerDto {
            deductible_cents: i.economie.deductible.cents(),
            economie_cents: i.economie.economie.cents(),
            taux_economie_micro: i.economie.taux.micro(),
            impot_sortie_cents: i.impot_sortie_estime.cents(),
            gain_net_cents: i.gain_fiscal_net.cents(),
            verdict: match i.verdict {
                Verdict::AucunInteret => "aucun",
                Verdict::Defavorable => "defavorable",
                Verdict::Limite => "limite",
                Verdict::Interessant => "interessant",
            }
            .into(),
            bas_de_tranche_cents: b.versement.cents(),
            economie_bas_de_tranche_cents: b.economie.cents(),
        })
    } else {
        None
    };

    let dons = if saisie.dons_aide_personnes_cents > 0 || saisie.dons_interet_general_cents > 0 {
        Some(reduction_dons(
            Money::from_cents(saisie.dons_aide_personnes_cents),
            Money::from_cents(saisie.dons_interet_general_cents),
            revenu_apres_per,
            impot_apres_per,
            &DonsParams::v2026(),
        ))
    } else {
        None
    };
    let impot_final = impot_apres_per - dons.map(|d| d.reduction).unwrap_or(Money::ZERO);

    Ok(SimulationDto {
        impot: ImpotDto {
            parts_demi: ir.demi_parts,
            impot_cents: ir.impot.cents(),
            decote_cents: ir.decote.cents(),
            plafonnement: ir.plafonnement_applique,
            tmi_micro: ir.tmi.micro(),
            taux_marginal_effectif_micro: ir.taux_marginal_effectif.micro(),
            taux_moyen_micro: ir.taux_moyen.micro(),
        },
        per,
        dons: dons.map(|d| DonsDto {
            reduction_cents: d.reduction.cents(),
            perdue_cents: d.perdue.cents(),
            excedent_reportable_cents: d.excedent_reportable.cents(),
            cout_reel_cents: d.cout_reel.cents(),
        }),
        impot_apres_per_cents: impot_apres_per.cents(),
        impot_final_cents: impot_final.cents(),
        economie_totale_cents: (ir.impot - impot_final).cents(),
    })
}
