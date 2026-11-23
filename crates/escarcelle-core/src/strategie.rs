//! Stratégies : écart à l'allocation cible et rééquilibrage par les versements
//! (orienter le versement vers les lignes sous-pondérées, sans vendre).

use std::fmt;

use crate::money::Money;
use crate::rate::{Rate, SCALE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrategieError {
    TaillesDifferentes,
    CiblesInvalides,
    VersementNegatif,
}

impl fmt::Display for StrategieError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaillesDifferentes => write!(f, "autant de cibles que de lignes attendues"),
            Self::CiblesInvalides => write!(f, "les poids cibles doivent être positifs et totaliser 100 %"),
            Self::VersementNegatif => write!(f, "versement négatif"),
        }
    }
}
impl std::error::Error for StrategieError {}

fn verifier(actuel: &[Money], cibles: &[Rate]) -> Result<(), StrategieError> {
    if actuel.len() != cibles.len() || actuel.is_empty() {
        return Err(StrategieError::TaillesDifferentes);
    }
    if cibles.iter().any(|c| c.micro() < 0) || cibles.iter().map(|c| c.micro()).sum::<i64>() != SCALE {
        return Err(StrategieError::CiblesInvalides);
    }
    Ok(())
}

/// Poids actuels de chaque ligne.
pub fn poids(actuel: &[Money]) -> Vec<Rate> {
    let total: i64 = actuel.iter().map(|m| m.cents()).sum();
    actuel
        .iter()
        .map(|m| if total > 0 { Rate::from_micro(crate::div_round(m.cents() as i128 * SCALE as i128, total as i128)) } else { Rate::ZERO })
        .collect()
}

/// Répartit `versement` pour rapprocher l'allocation de la cible.
/// La somme des montants rendus vaut exactement `versement`.
pub fn reequilibrer_par_versement(actuel: &[Money], cibles: &[Rate], versement: Money) -> Result<Vec<Money>, StrategieError> {
    verifier(actuel, cibles)?;
    if versement.is_negative() {
        return Err(StrategieError::VersementNegatif);
    }
    let v = versement.cents() as i128;
    let total_final = actuel.iter().map(|m| m.cents() as i128).sum::<i128>() + v;
    let deficits: Vec<i128> = actuel
        .iter()
        .zip(cibles)
        .map(|(a, c)| (total_final * c.micro() as i128 / SCALE as i128 - a.cents() as i128).max(0))
        .collect();
    let somme: i128 = deficits.iter().sum();

    let mut parts: Vec<i128> = if somme == 0 {
        cibles.iter().map(|c| v * c.micro() as i128 / SCALE as i128).collect()
    } else if somme >= v {
        deficits.iter().map(|d| v * d / somme).collect()
    } else {
        let reste = v - somme;
        deficits.iter().zip(cibles).map(|(d, c)| d + reste * c.micro() as i128 / SCALE as i128).collect()
    };
    // Les arrondis par défaut vont à la ligne la plus sous-pondérée (ou de plus gros poids cible).
    let ecart = v - parts.iter().sum::<i128>();
    let idx = (0..parts.len())
        .max_by_key(|&i| (deficits[i], cibles[i].micro()))
        .expect("au moins une ligne");
    parts[idx] += ecart;
    Ok(parts.into_iter().map(|p| Money::from_cents(p as i64)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eur(e: i64) -> Money {
        Money::from_euros(e)
    }
    fn pct(p: i64) -> Rate {
        Rate::from_micro(p * 10_000)
    }

    #[test]
    fn versement_insuffisant_va_au_plus_en_retard() {
        let r = reequilibrer_par_versement(&[eur(6_000), eur(4_000)], &[pct(70), pct(30)], eur(1_000)).unwrap();
        assert_eq!(r, vec![eur(1_000), eur(0)]);
    }

    #[test]
    fn versement_exact_retablit_la_cible() {
        let r = reequilibrer_par_versement(&[eur(7_000), eur(3_000)], &[pct(70), pct(30)], eur(1_000)).unwrap();
        assert_eq!(r, vec![eur(700), eur(300)]);
    }

    #[test]
    fn portefeuille_vide_monde_emergents() {
        let r = reequilibrer_par_versement(&[eur(0), eur(0)], &[pct(88), pct(12)], eur(1_000)).unwrap();
        assert_eq!(r, vec![eur(880), eur(120)]);
    }

    #[test]
    fn somme_exacte_malgre_les_arrondis() {
        let cibles = [Rate::from_micro(333_333), Rate::from_micro(333_333), Rate::from_micro(333_334)];
        let r = reequilibrer_par_versement(&[eur(10), eur(20), eur(30)], &cibles, Money::from_cents(100_001)).unwrap();
        assert_eq!(r.iter().copied().sum::<Money>(), Money::from_cents(100_001));
    }

    #[test]
    fn controles() {
        assert!(reequilibrer_par_versement(&[eur(1)], &[pct(90)], eur(1)).is_err());
        assert!(reequilibrer_par_versement(&[eur(1), eur(1)], &[pct(100)], eur(1)).is_err());
        assert_eq!(poids(&[eur(75), eur(25)]), vec![pct(75), pct(25)]);
    }
}
