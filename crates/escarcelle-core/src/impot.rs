//! Impôt sur le revenu : parts, barème, plafonnement du quotient familial, décote, TMI.
//!
//! Tous les calculs sont entiers. Pour éviter de diviser le revenu par un nombre
//! de parts fractionnaire, on travaille en **demi-parts** :
//! `impôt = Σ taux × max(0, min(2R, plafond × n) − seuil × n) / 2`, avec `n` le nombre de demi-parts.

use std::fmt;

use crate::money::Money;
use crate::rate::{Rate, SCALE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tranche {
    /// Borne basse de la tranche, pour une part.
    pub seuil: Money,
    pub taux: Rate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bareme {
    tranches: Vec<Tranche>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImpotError {
    BaremeInvalide,
    RevenuNegatif,
    ParentIsoleInvalide,
    ParametreManquant(&'static str),
}

impl fmt::Display for ImpotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BaremeInvalide => write!(f, "barème invalide (seuils croissants à partir de 0 attendus)"),
            Self::RevenuNegatif => write!(f, "le revenu net imposable ne peut pas être négatif"),
            Self::ParentIsoleInvalide => write!(f, "« parent isolé » exige une personne seule avec au moins un enfant"),
            Self::ParametreManquant(k) => write!(f, "paramètre réglementaire manquant : {k}"),
        }
    }
}
impl std::error::Error for ImpotError {}

impl Bareme {
    pub fn new(tranches: Vec<Tranche>) -> Result<Self, ImpotError> {
        let ok = tranches.first().is_some_and(|t| t.seuil == Money::ZERO)
            && tranches.windows(2).all(|w| w[0].seuil < w[1].seuil);
        if ok {
            Ok(Self { tranches })
        } else {
            Err(ImpotError::BaremeInvalide)
        }
    }

    pub fn tranches(&self) -> &[Tranche] {
        &self.tranches
    }

    /// Impôt issu du barème pour `demi_parts`, arrondi au centime.
    pub fn impot(&self, revenu: Money, demi_parts: u32) -> Money {
        let r2 = 2 * revenu.cents() as i128;
        let n = demi_parts as i128;
        let mut num: i128 = 0;
        for (i, t) in self.tranches.iter().enumerate() {
            let bas = t.seuil.cents() as i128 * n;
            let assiette = match self.tranches.get(i + 1) {
                Some(suiv) => r2.min(suiv.seuil.cents() as i128 * n) - bas,
                None => r2 - bas,
            };
            num += assiette.max(0) * t.taux.micro() as i128;
        }
        Money::from_cents(crate::div_round(num, 2 * SCALE as i128))
    }

    /// Taux de la tranche atteinte par le revenu par part.
    pub fn taux_marginal(&self, revenu: Money, demi_parts: u32) -> Rate {
        let r2 = 2 * revenu.cents() as i128;
        let n = demi_parts as i128;
        self.tranches
            .iter()
            .rev()
            .find(|t| (t.seuil.cents() as i128) * n < r2)
            .unwrap_or(&self.tranches[0])
            .taux
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IrParams {
    pub bareme: Bareme,
    pub qf_plafond_demi_part: Money,
    /// Plafond spécifique des deux demi-parts du premier enfant d'un parent isolé.
    pub qf_plafond_parent_isole_1er_enfant: Option<Money>,
    pub decote_seuil_seul: Money,
    pub decote_seuil_couple: Money,
    pub decote_plafond_seul: Money,
    pub decote_plafond_couple: Money,
    pub decote_coeff: Rate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Situation {
    Seul,
    /// Marié ou pacsé, imposition commune.
    Couple,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Foyer {
    pub situation: Situation,
    pub enfants: u32,
    /// Case T : personne seule élevant seule au moins un enfant.
    pub parent_isole: bool,
}

impl Foyer {
    pub fn seul() -> Self {
        Self { situation: Situation::Seul, enfants: 0, parent_isole: false }
    }
    pub fn couple(enfants: u32) -> Self {
        Self { situation: Situation::Couple, enfants, parent_isole: false }
    }

    pub fn demi_parts_base(&self) -> u32 {
        match self.situation {
            Situation::Seul => 2,
            Situation::Couple => 4,
        }
    }

    /// +1 demi-part pour chacun des deux premiers enfants, +2 à partir du troisième,
    /// +1 pour un parent isolé.
    pub fn demi_parts(&self) -> u32 {
        let e = self.enfants;
        let enfants = e.min(2) + e.saturating_sub(2) * 2;
        let isole = u32::from(self.parent_isole && e >= 1);
        self.demi_parts_base() + enfants + isole
    }

    /// Nombre de parts sous forme décimale (affichage uniquement).
    pub fn parts(&self) -> f64 {
        f64::from(self.demi_parts()) / 2.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResultatIr {
    pub demi_parts: u32,
    /// Impôt après plafonnement, avant décote (au centime).
    pub impot_brut: Money,
    pub plafonnement_applique: bool,
    pub decote: Money,
    /// Impôt final arrondi à l'euro.
    pub impot: Money,
    pub tmi: Rate,
    /// En zone de décote : TMI × (1 + coefficient de décote).
    pub taux_marginal_effectif: Rate,
    pub taux_moyen: Rate,
}

pub fn calculer_ir(revenu: Money, foyer: &Foyer, p: &IrParams) -> Result<ResultatIr, ImpotError> {
    if revenu.is_negative() {
        return Err(ImpotError::RevenuNegatif);
    }
    if foyer.parent_isole && (foyer.situation != Situation::Seul || foyer.enfants == 0) {
        return Err(ImpotError::ParentIsoleInvalide);
    }

    let n = foyer.demi_parts();
    let nb = foyer.demi_parts_base();
    let i_complet = p.bareme.impot(revenu, n);
    let i_base = p.bareme.impot(revenu, nb);

    let supp = i64::from(n - nb);
    let plafond = if supp == 0 {
        Money::ZERO
    } else if foyer.parent_isole {
        let pi = p
            .qf_plafond_parent_isole_1er_enfant
            .ok_or(ImpotError::ParametreManquant("ir.qf_plafond_parent_isole_1er_enfant"))?;
        pi + Money::from_cents(p.qf_plafond_demi_part.cents() * (supp - 2))
    } else {
        Money::from_cents(p.qf_plafond_demi_part.cents() * supp)
    };

    let (impot_brut, plafonnement_applique, n_tmi) = if i_base - i_complet > plafond {
        (i_base - plafond, true, nb)
    } else {
        (i_complet, false, n)
    };
    let tmi = p.bareme.taux_marginal(revenu, n_tmi);

    let (seuil, plafond_decote) = match foyer.situation {
        Situation::Seul => (p.decote_seuil_seul, p.decote_plafond_seul),
        Situation::Couple => (p.decote_seuil_couple, p.decote_plafond_couple),
    };
    let decote = if impot_brut < seuil {
        (plafond_decote - p.decote_coeff.apply(impot_brut)).max(Money::ZERO).min(impot_brut)
    } else {
        Money::ZERO
    };

    let impot = (impot_brut - decote).round_to_euro();
    let taux_marginal_effectif = if decote > Money::ZERO {
        tmi.times(Rate::from_micro(SCALE + p.decote_coeff.micro()))
    } else {
        tmi
    };
    let taux_moyen = if revenu.cents() > 0 {
        Rate::from_micro(crate::div_round(
            impot.cents() as i128 * SCALE as i128,
            revenu.cents() as i128,
        ))
    } else {
        Rate::ZERO
    };

    Ok(ResultatIr {
        demi_parts: n,
        impot_brut,
        plafonnement_applique,
        decote,
        impot,
        tmi,
        taux_marginal_effectif,
        taux_moyen,
    })
}

/// Plafonds de revenu fiscal de référence pour le LEP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LepParams {
    pub rfr_1_part: Money,
    pub rfr_demi_part_supp: Money,
}

pub fn seuil_rfr_lep(demi_parts: u32, p: &LepParams) -> Money {
    p.rfr_1_part + Money::from_cents(p.rfr_demi_part_supp.cents() * i64::from(demi_parts.saturating_sub(2)))
}

pub fn eligible_lep(rfr: Money, demi_parts: u32, p: &LepParams) -> bool {
    rfr <= seuil_rfr_lep(demi_parts, p)
}
