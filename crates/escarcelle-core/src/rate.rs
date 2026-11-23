//! Taux exprimés en millionièmes (1,70 % = 17 000).

use crate::money::Money;
use std::fmt;

/// Échelle des taux : 1,0 = 1 000 000.
pub const SCALE: i64 = 1_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Rate(i64);

impl Rate {
    pub const ZERO: Rate = Rate(0);

    pub const fn from_micro(micro: i64) -> Self {
        Rate(micro)
    }
    pub const fn micro(self) -> i64 {
        self.0
    }

    /// Lit un taux en fraction : `"0.017"` → 1,70 %.
    pub fn parse_fraction(s: &str) -> Option<Self> {
        parse_decimal(s, 6).map(Rate)
    }

    /// Lit un taux en pourcentage : `"1,70"` ou `"1,70 %"` → 1,70 %.
    pub fn parse_percent(s: &str) -> Option<Self> {
        let s = s.trim().trim_end_matches('%').trim();
        parse_decimal(s, 4).map(Rate)
    }

    /// Applique le taux à un montant, arrondi au centime.
    pub fn apply(self, m: Money) -> Money {
        Money::from_cents(crate::div_round(m.cents() as i128 * self.0 as i128, SCALE as i128))
    }

    /// Produit de deux taux.
    pub fn times(self, o: Rate) -> Rate {
        Rate(crate::div_round(self.0 as i128 * o.0 as i128, SCALE as i128))
    }
}

impl fmt::Display for Rate {
    /// `1,70 %` (deux décimales).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let centiemes = crate::div_round(self.0 as i128, 100); // 1e-4 → 1e-2 %
        let signe = if centiemes < 0 { "-" } else { "" };
        let a = centiemes.unsigned_abs();
        write!(f, "{signe}{},{:02}\u{A0}%", a / 100, a % 100)
    }
}

/// Lit un décimal positif (`.` ou `,`) en entier à `scale` chiffres après la virgule.
/// Refuse toute perte de précision.
pub(crate) fn parse_decimal(s: &str, scale: u32) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (ent, frac) = match s.find(['.', ',']) {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, ""),
    };
    if !ent.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if ent.is_empty() && frac.is_empty() {
        return None;
    }
    if frac.len() > scale as usize {
        return None;
    }
    let ent: i64 = if ent.is_empty() { 0 } else { ent.parse().ok()? };
    let mut f: i64 = if frac.is_empty() { 0 } else { frac.parse().ok()? };
    for _ in frac.len()..scale as usize {
        f *= 10;
    }
    ent.checked_mul(10_i64.pow(scale))?.checked_add(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lecture_et_affichage() {
        let r = Rate::parse_percent("1,70 %").unwrap();
        assert_eq!(r, Rate::parse_fraction("0.017").unwrap());
        assert_eq!(r.micro(), 17_000);
        assert_eq!(r.to_string(), "1,70\u{A0}%");
        assert_eq!(Rate::parse_percent("45,25").unwrap().micro(), 452_500);
    }

    #[test]
    fn application() {
        let r = Rate::parse_percent("30").unwrap();
        assert_eq!(r.apply(Money::from_euros(5421)), Money::from_cents(162630));
    }
}
