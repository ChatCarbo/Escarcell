//! PER : plafond de déduction, économie d'impôt exacte, « verser jusqu'au bas de ma tranche »,
//! imposition d'une sortie en capital.
//!
//! L'économie d'impôt est calculée avec le moteur d'impôt complet (parts,
//! plafonnement du quotient familial, décote), jamais par simple multiplication par la TMI.

use crate::impot::{calculer_ir, Foyer, ImpotError, IrParams};
use crate::money::Money;
use crate::rate::{Rate, SCALE};

/// Plafond de déduction d'un salarié pour les versements de l'année N :
/// 10 % des revenus professionnels de N-1, entre 10 % du PASS N-1 et 10 % de 8 PASS N-1.
pub fn plafond_deduction_salarie(revenus_pro_n1: Money, pass_n1: Money) -> Money {
    let dix = Rate::from_micro(100_000);
    let plancher = dix.apply(pass_n1);
    let plafond = dix.apply(Money::from_cents(pass_n1.cents() * 8));
    dix.apply(revenus_pro_n1.max(Money::ZERO)).max(plancher).min(plafond)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EconomiePer {
    pub versement: Money,
    pub deductible: Money,
    pub impot_avant: Money,
    pub impot_apres: Money,
    pub economie: Money,
    /// Économie / versement.
    pub taux: Rate,
}

/// Économie d'impôt d'un versement, dans la limite du plafond disponible.
pub fn economie_impot(
    revenu_imposable: Money,
    foyer: &Foyer,
    p: &IrParams,
    versement: Money,
    plafond_disponible: Money,
) -> Result<EconomiePer, ImpotError> {
    let deductible = versement.min(plafond_disponible).min(revenu_imposable).max(Money::ZERO);
    let avant = calculer_ir(revenu_imposable, foyer, p)?.impot;
    let apres = calculer_ir(revenu_imposable - deductible, foyer, p)?.impot;
    let economie = avant - apres;
    let taux = if versement.cents() > 0 {
        Rate::from_micro(crate::div_round(economie.cents() as i128 * SCALE as i128, versement.cents() as i128))
    } else {
        Rate::ZERO
    };
    Ok(EconomiePer { versement, deductible, impot_avant: avant, impot_apres: apres, economie, taux })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BasDeTranche {
    pub tmi_actuelle: Rate,
    /// Plus petit versement qui fait passer sous la tranche actuelle.
    pub versement: Money,
    pub economie: Money,
}

/// Versement qui ramène le foyer juste sous sa tranche marginale actuelle.
/// Recherche dichotomique sur le moteur d'impôt : gère parts, plafonnement et décote.
pub fn verser_jusqu_au_bas_de_tranche(revenu: Money, foyer: &Foyer, p: &IrParams) -> Result<BasDeTranche, ImpotError> {
    let tmi0 = calculer_ir(revenu, foyer, p)?.tmi;
    if tmi0 == Rate::ZERO {
        return Ok(BasDeTranche { tmi_actuelle: tmi0, versement: Money::ZERO, economie: Money::ZERO });
    }
    let (mut lo, mut hi) = (0i64, revenu.cents());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if calculer_ir(revenu - Money::from_cents(mid), foyer, p)?.tmi != tmi0 {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let versement = Money::from_cents(lo);
    let avant = calculer_ir(revenu, foyer, p)?.impot;
    let apres = calculer_ir(revenu - versement, foyer, p)?.impot;
    Ok(BasDeTranche { tmi_actuelle: tmi0, versement, economie: avant - apres })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortieCapital {
    /// Supplément d'impôt sur le revenu dû aux versements déduits, imposés au barème.
    pub impot_versements: Money,
    /// PFU (part impôt) + prélèvements sociaux sur les gains.
    pub impot_gains: Money,
    pub total: Money,
}

/// Imposition d'une sortie en capital (versements déductibles) l'année de la sortie.
pub fn sortie_capital(
    revenu_annee_sortie: Money,
    foyer: &Foyer,
    p: &IrParams,
    versements_deduits: Money,
    gains: Money,
    pfu_ir: Rate,
    prelevements_sociaux: Rate,
) -> Result<SortieCapital, ImpotError> {
    let sans = calculer_ir(revenu_annee_sortie, foyer, p)?.impot;
    let avec = calculer_ir(revenu_annee_sortie + versements_deduits, foyer, p)?.impot;
    let g = gains.max(Money::ZERO);
    let impot_gains = pfu_ir.apply(g) + prelevements_sociaux.apply(g);
    let impot_versements = avec - sans;
    Ok(SortieCapital { impot_versements, impot_gains, total: impot_versements + impot_gains })
}

/// Verdict sur l'intérêt fiscal d'un versement sur un PER.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Pas d'impôt aujourd'hui : la déduction ne rapporte rien.
    AucunInteret,
    /// La sortie sera taxée autant ou plus que l'économie d'aujourd'hui.
    Defavorable,
    /// Gain faible (moins de 10 points d'écart) au regard du blocage jusqu'à la retraite.
    Limite,
    Interessant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InteretPer {
    pub economie: EconomiePer,
    /// Impôt sur le revenu estimé à la sortie en capital sur la part déduite (TMI retraite × montant déduit).
    pub impot_sortie_estime: Money,
    /// Économie d'aujourd'hui − impôt estimé à la sortie (hors gains, taxés au PFU dans tous les cas).
    pub gain_fiscal_net: Money,
    pub verdict: Verdict,
}

/// Faut-il verser sur un PER ? Compare l'économie d'impôt d'aujourd'hui à l'impôt estimé à la sortie.
pub fn interet_versement(
    revenu_imposable: Money,
    foyer: &Foyer,
    p: &IrParams,
    versement: Money,
    plafond_disponible: Money,
    tmi_retraite: Rate,
) -> Result<InteretPer, ImpotError> {
    let economie = economie_impot(revenu_imposable, foyer, p, versement, plafond_disponible)?;
    let impot_sortie_estime = tmi_retraite.apply(economie.deductible);
    let gain_fiscal_net = economie.economie - impot_sortie_estime;
    let ecart = economie.taux.micro() - tmi_retraite.micro();
    let verdict = if economie.impot_avant == Money::ZERO || economie.economie == Money::ZERO {
        Verdict::AucunInteret
    } else if gain_fiscal_net <= Money::ZERO {
        Verdict::Defavorable
    } else if ecart < 100_000 {
        Verdict::Limite
    } else {
        Verdict::Interessant
    };
    Ok(InteretPer { economie, impot_sortie_estime, gain_fiscal_net, verdict })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::embarque;

    fn eur(e: i64) -> Money {
        Money::from_euros(e)
    }

    #[test]
    fn plafonds_2026() {
        let pass_2025 = eur(47_100);
        assert_eq!(plafond_deduction_salarie(eur(50_000), pass_2025), eur(5_000));
        assert_eq!(plafond_deduction_salarie(eur(20_000), pass_2025), eur(4_710));
        assert_eq!(plafond_deduction_salarie(eur(500_000), pass_2025), eur(37_680));
    }

    #[test]
    fn economie_seul_35000() {
        let p = embarque::ir_revenus_2025();
        let e = economie_impot(eur(35_000), &Foyer::seul(), &p, eur(3_000), eur(5_000)).unwrap();
        assert_eq!((e.impot_avant, e.impot_apres, e.economie), (eur(3_604), eur(2_704), eur(900)));
        assert_eq!(e.taux, Rate::from_micro(300_000));
        // Versement au-delà du plafond : seule la part déductible compte.
        let e = economie_impot(eur(35_000), &Foyer::seul(), &p, eur(3_000), eur(1_000)).unwrap();
        assert_eq!(e.deductible, eur(1_000));
        assert_eq!(e.economie, eur(300));
    }

    #[test]
    fn bas_de_tranche() {
        let p = embarque::ir_revenus_2025();
        let b = verser_jusqu_au_bas_de_tranche(eur(35_000), &Foyer::seul(), &p).unwrap();
        assert_eq!(b.tmi_actuelle, Rate::from_micro(300_000));
        assert_eq!(b.versement, eur(5_421)); // ramène à 29 579 €, borne de la tranche à 30 %
        assert_eq!(b.economie, eur(1_628)); // 3 604 − 1 976 (décote incluse)
        let zero = verser_jusqu_au_bas_de_tranche(eur(10_000), &Foyer::seul(), &p).unwrap();
        assert_eq!(zero.versement, Money::ZERO);
    }

    #[test]
    fn verdicts() {
        let p = embarque::ir_revenus_2025();
        let tmi = |x: i64| Rate::from_micro(x * 10_000);
        // TMI 30 % aujourd'hui, 11 % à la retraite : intéressant.
        let i = interet_versement(eur(35_000), &Foyer::seul(), &p, eur(3_000), eur(5_000), tmi(11)).unwrap();
        assert_eq!(i.verdict, Verdict::Interessant);
        assert_eq!(i.gain_fiscal_net, eur(570)); // 900 − 330
        // Même TMI à la retraite : défavorable (simple report, épargne bloquée).
        let i = interet_versement(eur(35_000), &Foyer::seul(), &p, eur(3_000), eur(5_000), tmi(30)).unwrap();
        assert_eq!(i.verdict, Verdict::Defavorable);
        // Pas d'impôt : aucun intérêt.
        let i = interet_versement(eur(10_000), &Foyer::seul(), &p, eur(1_000), eur(4_710), tmi(0)).unwrap();
        assert_eq!(i.verdict, Verdict::AucunInteret);
    }

    #[test]
    fn sortie_en_capital() {
        let p = embarque::ir_revenus_2025();
        let s = sortie_capital(
            eur(20_000),
            &Foyer::seul(),
            &p,
            eur(10_000),
            eur(2_000),
            Rate::from_micro(128_000),
            Rate::from_micro(186_000),
        )
        .unwrap();
        assert_eq!(s.impot_versements, eur(1_659)); // 2 104 − 445 (décote sur 20 000 €)
        assert_eq!(s.impot_gains, eur(628)); // 31,4 % de 2 000 €
        assert_eq!(s.total, eur(2_287));
    }
}
