//! Livret A, LDDS, LEP : intérêts par quinzaines.
//!
//! - Un dépôt est rémunéré à partir de la quinzaine **suivante**.
//! - Un retrait cesse d'être rémunéré dès le début de la quinzaine **en cours**.
//! - Chaque quinzaine est rémunérée au taux en vigueur à son premier jour.
//! - Les intérêts sont arrondis au centime une seule fois et capitalisés au 31 décembre.
//! - Le plafond limite les dépôts ; seule la capitalisation des intérêts peut le dépasser.

use std::collections::BTreeMap;
use std::fmt;

use crate::date::Date;
use crate::money::Money;
use crate::params::Series;
use crate::rate::{Rate, SCALE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sens {
    Depot,
    Retrait,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mouvement {
    pub date: Date,
    pub sens: Sens,
    /// Montant strictement positif.
    pub montant: Money,
}

impl Mouvement {
    pub fn depot(date: Date, montant: Money) -> Self {
        Self { date, sens: Sens::Depot, montant }
    }
    pub fn retrait(date: Date, montant: Money) -> Self {
        Self { date, sens: Sens::Retrait, montant }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnneeLivret {
    pub annee: i32,
    /// Intérêts de l'année, capitalisés au 31 décembre.
    pub interets: Money,
    /// Solde au 31 décembre, intérêts inclus.
    pub solde_fin: Money,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LivretError {
    MontantInvalide(Mouvement),
    TauxManquant(Date),
}

impl fmt::Display for LivretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MontantInvalide(m) => write!(f, "montant non positif au {}", m.date),
            Self::TauxManquant(d) => write!(f, "aucun taux en vigueur au {d}"),
        }
    }
}
impl std::error::Error for LivretError {}

/// Numéro de quinzaine dans l'année (1 à 24).
pub fn quinzaine(d: Date) -> u8 {
    (d.month() - 1) * 2 + if d.day() <= 15 { 1 } else { 2 }
}

/// (année, quinzaine) à partir de laquelle un mouvement compte.
pub fn quinzaine_effet(d: Date, sens: Sens) -> (i32, u8) {
    let q = quinzaine(d);
    match sens {
        Sens::Retrait => (d.year(), q),
        Sens::Depot if q == 24 => (d.year() + 1, 1),
        Sens::Depot => (d.year(), q + 1),
    }
}

/// Premier jour de la quinzaine `q` de l'année.
pub fn debut_quinzaine(annee: i32, q: u8) -> Date {
    assert!((1..=24).contains(&q), "quinzaine hors bornes");
    let mois = q.div_ceil(2);
    let jour = if q % 2 == 1 { 1 } else { 16 };
    Date::ymd(annee, mois, jour)
}

/// Calcule les intérêts année par année, de la première année concernée jusqu'à `jusqu_a`.
pub fn simuler(mouvements: &[Mouvement], taux: &Series<Rate>, jusqu_a: i32) -> Result<Vec<AnneeLivret>, LivretError> {
    let mut effets: BTreeMap<(i32, u8), i64> = BTreeMap::new();
    for m in mouvements {
        if m.montant.cents() <= 0 {
            return Err(LivretError::MontantInvalide(*m));
        }
        let delta = match m.sens {
            Sens::Depot => m.montant.cents(),
            Sens::Retrait => -m.montant.cents(),
        };
        *effets.entry(quinzaine_effet(m.date, m.sens)).or_default() += delta;
    }
    let Some(&(debut, _)) = effets.keys().next() else {
        return Ok(Vec::new());
    };

    let mut solde: i64 = 0;
    let mut annees = Vec::new();
    for annee in debut..=jusqu_a {
        let mut cumul: i128 = 0;
        for q in 1..=24u8 {
            solde += effets.get(&(annee, q)).copied().unwrap_or(0);
            if solde > 0 {
                let d = debut_quinzaine(annee, q);
                let t = taux.at(d).ok_or(LivretError::TauxManquant(d))?;
                cumul += solde as i128 * t.micro() as i128;
            }
        }
        let interets = crate::div_round(cumul, 24 * SCALE as i128);
        solde += interets;
        annees.push(AnneeLivret { annee, interets: Money::from_cents(interets), solde_fin: Money::from_cents(solde) });
    }
    Ok(annees)
}

/// Dépôt refusé : il ferait dépasser le plafond.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlafondDepasse {
    /// Montant maximum encore déposable (zéro si le solde dépasse déjà le plafond).
    pub marge: Money,
}

impl fmt::Display for PlafondDepasse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "plafond atteint, dépôt maximum possible : {}", self.marge)
    }
}
impl std::error::Error for PlafondDepasse {}

/// Vérifie qu'un dépôt est possible compte tenu du solde actuel (intérêts inclus).
pub fn verifier_depot(solde: Money, depot: Money, plafond: Money) -> Result<(), PlafondDepasse> {
    let marge = if solde >= plafond { Money::ZERO } else { plafond - solde };
    if depot > marge {
        Err(PlafondDepasse { marge })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quinzaines() {
        assert_eq!(quinzaine(Date::ymd(2026, 1, 15)), 1);
        assert_eq!(quinzaine(Date::ymd(2026, 1, 16)), 2);
        assert_eq!(quinzaine(Date::ymd(2026, 3, 10)), 5);
        assert_eq!(quinzaine(Date::ymd(2026, 12, 31)), 24);
        assert_eq!(debut_quinzaine(2026, 6), Date::ymd(2026, 3, 16));
        assert_eq!(debut_quinzaine(2026, 15), Date::ymd(2026, 8, 1));
    }

    #[test]
    fn effets() {
        let d = Date::ymd(2026, 12, 20);
        assert_eq!(quinzaine_effet(d, Sens::Depot), (2027, 1));
        assert_eq!(quinzaine_effet(d, Sens::Retrait), (2026, 24));
    }

    #[test]
    fn plafond() {
        let p = Money::from_euros(22_950);
        assert!(verifier_depot(Money::from_euros(22_000), Money::from_euros(950), p).is_ok());
        assert_eq!(
            verifier_depot(Money::from_euros(22_000), Money::from_euros(951), p),
            Err(PlafondDepasse { marge: Money::from_euros(950) })
        );
        // Solde au-delà du plafond grâce aux intérêts : aucun dépôt possible.
        assert_eq!(
            verifier_depot(Money::from_euros(23_100), Money::from_cents(1), p),
            Err(PlafondDepasse { marge: Money::ZERO })
        );
    }
}
