//! Cours unitaire d'un titre, en millionièmes d'unité monétaire
//! (les cours ont souvent plus de deux décimales).

use crate::money::Money;
use crate::rate::Rate;
use std::fmt;

const SCALE: i64 = 1_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Price(i64);

impl Price {
    pub const fn from_micro(micro: i64) -> Self {
        Price(micro)
    }
    pub const fn micro(self) -> i64 {
        self.0
    }

    /// Lit `"512.3456"` ou `"512,3456"` (6 décimales au plus).
    pub fn parse(s: &str) -> Option<Self> {
        let clean: String = s.chars().filter(|c| !c.is_whitespace() && *c != '\u{202F}' && *c != '\u{A0}').collect();
        crate::rate::parse_decimal(&clean, 6).map(Price)
    }

    /// Valeur de `quantite` titres (quantité en millionièmes de part), arrondie au centime.
    pub fn valeur(self, quantite_micro: i64) -> Money {
        Money::from_cents(crate::div_round(
            self.0 as i128 * quantite_micro as i128 * 100,
            SCALE as i128 * SCALE as i128,
        ))
    }

    /// Écart relatif |a − b| / b.
    pub fn ecart_relatif(self, reference: Price) -> Rate {
        if reference.0 == 0 {
            return Rate::from_micro(i64::MAX);
        }
        Rate::from_micro(crate::div_round((self.0 - reference.0).abs() as i128 * 1_000_000, reference.0.abs() as i128))
    }
}

impl fmt::Display for Price {
    /// Quatre décimales, format français : `512,3456`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d4 = crate::div_round(self.0 as i128, 100);
        let signe = if d4 < 0 { "-" } else { "" };
        let a = d4.unsigned_abs();
        write!(f, "{signe}{},{:04}", a / 10_000, a % 10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lecture_valeur_ecart() {
        let p = Price::parse("512,3456").unwrap();
        assert_eq!(p.to_string(), "512,3456");
        // 3 parts × 512,3456 = 1 537,0368 → 1 537,04 €
        assert_eq!(p.valeur(3_000_000), Money::from_cents(153_704));
        let q = Price::parse("522.5925").unwrap();
        assert_eq!(q.ecart_relatif(p), Rate::from_micro(20_000)); // 2,00 %
    }
}
