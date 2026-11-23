//! Paramètres réglementaires datés.
//!
//! Le cœur reçoit des séries déjà résolues (la priorité manuel > mise à jour >
//! embarqué est gérée par la couche de stockage). Le module [`embarque`] fournit
//! le jeu initial livré avec l'application.

use crate::date::Date;
use crate::impot::{Bareme, IrParams, LepParams, Tranche};
use crate::money::Money;
use crate::rate::Rate;

/// Série de valeurs avec date d'effet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Series<T> {
    points: Vec<(Date, T)>,
}

impl<T> Series<T> {
    pub fn new(mut points: Vec<(Date, T)>) -> Self {
        points.sort_by_key(|(d, _)| *d);
        Self { points }
    }

    /// Valeur en vigueur à la date `d` : dernière date d'effet ≤ `d`.
    pub fn at(&self, d: Date) -> Option<&T> {
        self.points.iter().rev().find(|(e, _)| *e <= d).map(|(_, v)| v)
    }

    pub fn points(&self) -> &[(Date, T)] {
        &self.points
    }
}

/// Paramètres d'un livret réglementé.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LivretParams {
    pub taux: Series<Rate>,
    pub plafond: Money,
}

/// Jeu de paramètres embarqué, vérifié le 23/11/2026.
/// Sources : service-public.gouv.fr (A15818, A18045, A18261), BOFiP BOI-IR-LIQ-20-20-20/30.
pub mod embarque {
    use super::*;

    fn pct(s: &str) -> Rate {
        Rate::parse_percent(s).expect("taux codé en dur invalide")
    }

    fn taux_livret_a() -> Series<Rate> {
        Series::new(vec![
            (Date::ymd(2025, 8, 1), pct("1,70")),
            (Date::ymd(2026, 2, 1), pct("1,50")),
            (Date::ymd(2026, 8, 1), pct("1,70")),
        ])
    }

    pub fn livret_a() -> LivretParams {
        LivretParams { taux: taux_livret_a(), plafond: Money::from_euros(22_950) }
    }

    /// Le taux du LDDS est aligné sur celui du Livret A.
    pub fn ldds() -> LivretParams {
        LivretParams { taux: taux_livret_a(), plafond: Money::from_euros(12_000) }
    }

    pub fn lep() -> LivretParams {
        LivretParams {
            taux: Series::new(vec![
                (Date::ymd(2025, 8, 1), pct("2,70")),
                (Date::ymd(2026, 2, 1), pct("2,50")),
            ]),
            plafond: Money::from_euros(10_000),
        }
    }

    /// Plafonds de RFR pour le LEP en 2026 (métropole).
    pub fn lep_rfr_2026() -> LepParams {
        LepParams {
            rfr_1_part: Money::from_euros(23_028),
            rfr_demi_part_supp: Money::from_euros(6_149),
        }
    }

    /// Impôt 2026 sur les revenus 2025 (loi n° 2026-103 du 19 février 2026).
    pub fn ir_revenus_2025() -> IrParams {
        let t = |seuil: i64, taux: &str| Tranche { seuil: Money::from_euros(seuil), taux: pct(taux) };
        IrParams {
            bareme: Bareme::new(vec![
                t(0, "0"),
                t(11_600, "11"),
                t(29_579, "30"),
                t(84_577, "41"),
                t(181_917, "45"),
            ])
            .expect("barème codé en dur invalide"),
            qf_plafond_demi_part: Money::from_euros(1_807),
            // À renseigner depuis le BOFiP avant d'activer le cas « parent isolé ».
            qf_plafond_parent_isole_1er_enfant: None,
            decote_seuil_seul: Money::from_euros(1_982),
            decote_seuil_couple: Money::from_euros(3_277),
            decote_plafond_seul: Money::from_euros(897),
            decote_plafond_couple: Money::from_euros(1_483),
            decote_coeff: pct("45,25"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_par_date() {
        let p = embarque::livret_a();
        assert_eq!(p.taux.at(Date::ymd(2026, 7, 31)), Some(&Rate::from_micro(15_000)));
        assert_eq!(p.taux.at(Date::ymd(2026, 8, 1)), Some(&Rate::from_micro(17_000)));
        assert_eq!(p.taux.at(Date::ymd(2024, 1, 1)), None);
    }
}
