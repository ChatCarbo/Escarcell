//! Paramètres réglementaires datés.
//!
//! Le cœur reçoit des séries déjà résolues (la priorité manuel > mise à jour >
//! embarqué est gérée par la couche de stockage). Le module [`embarque`] fournit
//! le jeu initial livré avec l'application.

use crate::date::Date;
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

/// Jeu de paramètres embarqué, vérifié le 25/11/2026.
/// Sources : service-public.gouv.fr (F2365, A15818, A18000, A18045, A18261), Banque de France (rapports annuels
/// sur l'épargne réglementée 2022 à 2025, communiqué du 14 janvier 2022), barèmes IPP (historique du Livret A).
///
/// Les taux sont connus depuis le 1er février 2020 (entrée en vigueur de la formule de calcul actuelle) :
/// une opération plus ancienne est refusée à la saisie.
pub mod embarque {
    use super::*;

    fn pct(s: &str) -> Rate {
        Rate::parse_percent(s).expect("taux codé en dur invalide")
    }

    /// Première date à partir de laquelle les taux des trois livrets sont connus.
    pub fn debut() -> Date {
        Date::ymd(2020, 2, 1)
    }

    fn taux_livret_a() -> Series<Rate> {
        Series::new(vec![
            (Date::ymd(2020, 2, 1), pct("0,50")),
            (Date::ymd(2022, 2, 1), pct("1,00")),
            (Date::ymd(2022, 8, 1), pct("2,00")),
            (Date::ymd(2023, 2, 1), pct("3,00")),
            (Date::ymd(2025, 2, 1), pct("2,40")),
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
                (Date::ymd(2020, 2, 1), pct("1,00")),
                (Date::ymd(2022, 2, 1), pct("2,20")),
                (Date::ymd(2022, 8, 1), pct("4,60")),
                (Date::ymd(2023, 2, 1), pct("6,10")),
                (Date::ymd(2023, 8, 1), pct("6,00")),
                (Date::ymd(2024, 2, 1), pct("5,00")),
                (Date::ymd(2024, 8, 1), pct("4,00")),
                (Date::ymd(2025, 2, 1), pct("3,50")),
                (Date::ymd(2025, 8, 1), pct("2,70")),
                (Date::ymd(2026, 2, 1), pct("2,50")),
            ]),
            plafond: Money::from_euros(10_000),
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
        assert_eq!(p.taux.at(Date::ymd(2024, 1, 1)), Some(&Rate::from_micro(30_000)));
        assert_eq!(p.taux.at(Date::ymd(2020, 1, 31)), None);
    }

    #[test]
    fn taux_connus_depuis_le_debut_pour_les_trois_livrets() {
        for p in [embarque::livret_a(), embarque::ldds(), embarque::lep()] {
            assert!(p.taux.at(embarque::debut()).is_some());
            assert!(p.taux.at(Date::ymd(embarque::debut().year(), 1, 31)).is_none());
        }
        // Historique du LEP : 6,1 % au premier semestre 2023, 4 % au second semestre 2024.
        let lep = embarque::lep();
        assert_eq!(lep.taux.at(Date::ymd(2023, 5, 1)), Some(&Rate::from_micro(61_000)));
        assert_eq!(lep.taux.at(Date::ymd(2024, 9, 1)), Some(&Rate::from_micro(40_000)));
    }

    /// Intérêts maximaux de 2026 (livret au plafond toute l'année), calculés à la main :
    /// Livret A 22 950 € × (12 × 1,70 % + 12 × 1,50 %) / 24 = 367,20 € ; LDDS 12 000 € → 192,00 € ;
    /// LEP 10 000 € × (2 × 2,70 % + 22 × 2,50 %) / 24 = 251,67 €.
    #[test]
    fn interets_maximum_2026() {
        use crate::livret::interets_maximum;
        let cas = [(embarque::livret_a(), 36_720), (embarque::ldds(), 19_200), (embarque::lep(), 25_167)];
        for (p, attendu) in cas {
            assert_eq!(interets_maximum(p.plafond, &p.taux, 2026).unwrap(), crate::Money::from_cents(attendu));
        }
    }
}
