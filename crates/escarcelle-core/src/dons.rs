//! Réduction d'impôt pour dons (article 200 du CGI), règles 2026.
//!
//! - 75 % pour les organismes d'aide aux personnes en difficulté ou aux victimes de violence
//!   (« dispositif Coluche »), dans la limite de 2 000 € de dons par an (dons depuis le 14/10/2025) ;
//!   cette fraction n'entre pas dans la limite des 20 % ;
//! - 66 % pour les autres organismes d'intérêt général et pour la fraction des dons « Coluche »
//!   au-delà de 2 000 €, dans la limite de 20 % du revenu imposable ; l'excédent est reportable 5 ans ;
//! - c'est une réduction d'impôt : elle ne peut pas dépasser l'impôt dû (pas de remboursement).

use crate::money::Money;
use crate::rate::{Rate, SCALE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DonsParams {
    pub taux_majore: Rate,
    pub plafond_majore: Money,
    pub taux_normal: Rate,
    pub limite_revenu: Rate,
}

impl DonsParams {
    pub fn v2026() -> Self {
        Self {
            taux_majore: Rate::from_micro(750_000),
            plafond_majore: Money::from_euros(2_000),
            taux_normal: Rate::from_micro(660_000),
            limite_revenu: Rate::from_micro(200_000),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReductionDons {
    /// Réduction théorique, avant limitation à l'impôt dû.
    pub reduction_theorique: Money,
    /// Réduction effectivement imputée sur l'impôt.
    pub reduction: Money,
    /// Dons au-delà de 20 % du revenu, reportables sur les 5 années suivantes.
    pub excedent_reportable: Money,
    /// Part de la réduction perdue faute d'impôt suffisant.
    pub perdue: Money,
    /// Coût réel des dons après réduction.
    pub cout_reel: Money,
}

pub fn reduction_dons(
    dons_aide_personnes: Money,
    dons_interet_general: Money,
    revenu_imposable: Money,
    impot_avant_reduction: Money,
    p: &DonsParams,
) -> ReductionDons {
    let majore = dons_aide_personnes.max(Money::ZERO).min(p.plafond_majore);
    let au_dela = dons_aide_personnes.max(Money::ZERO) - majore;
    let base_normale = au_dela + dons_interet_general.max(Money::ZERO);
    let limite = p.limite_revenu.apply(revenu_imposable.max(Money::ZERO));
    let retenu = base_normale.min(limite);
    let theorique = p.taux_majore.apply(majore) + p.taux_normal.apply(retenu);
    let reduction = theorique.min(impot_avant_reduction.max(Money::ZERO));
    let total = dons_aide_personnes.max(Money::ZERO) + dons_interet_general.max(Money::ZERO);
    ReductionDons {
        reduction_theorique: theorique,
        reduction,
        excedent_reportable: base_normale - retenu,
        perdue: theorique - reduction,
        cout_reel: total - reduction,
    }
}

/// Taux effectif de réduction (réduction / dons).
pub fn taux_effectif(r: &ReductionDons, total_dons: Money) -> Rate {
    if total_dons.cents() <= 0 {
        return Rate::ZERO;
    }
    Rate::from_micro(crate::div_round(r.reduction.cents() as i128 * SCALE as i128, total_dons.cents() as i128))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eur(e: i64) -> Money {
        Money::from_euros(e)
    }

    #[test]
    fn exemple_service_public() {
        // 2 200 € à une association d'aide aux personnes : 75 % × 2 000 + 66 % × 200 = 1 632 €.
        let r = reduction_dons(eur(2_200), Money::ZERO, eur(50_000), eur(5_000), &DonsParams::v2026());
        assert_eq!(r.reduction, eur(1_632));
        assert_eq!(r.cout_reel, eur(568));
    }

    #[test]
    fn exemple_bofip_limite_20_pourcent() {
        // Revenu 50 000 € : 75 % × 2 000 + 66 % × 10 000 = 8 100 € ; 2 000 € reportables.
        let r = reduction_dons(eur(2_000), eur(12_000), eur(50_000), eur(20_000), &DonsParams::v2026());
        assert_eq!(r.reduction, eur(8_100));
        assert_eq!(r.excedent_reportable, eur(2_000));
    }

    #[test]
    fn limitee_a_l_impot_du() {
        let r = reduction_dons(Money::ZERO, eur(1_000), eur(30_000), eur(400), &DonsParams::v2026());
        assert_eq!(r.reduction_theorique, eur(660));
        assert_eq!(r.reduction, eur(400));
        assert_eq!(r.perdue, eur(260));
    }

    #[test]
    fn don_classique_de_100_euros_coute_34() {
        let r = reduction_dons(Money::ZERO, eur(100), eur(30_000), eur(2_000), &DonsParams::v2026());
        assert_eq!(r.cout_reel, eur(34));
        assert_eq!(taux_effectif(&r, eur(100)), Rate::from_micro(660_000));
    }
}
