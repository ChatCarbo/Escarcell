//! Montants en centimes d'euro.

use std::fmt;
use std::iter::Sum;
use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};
use std::str::FromStr;

/// Montant en centimes. Jamais de `f64` pour de l'argent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Money(i64);

impl Money {
    pub const ZERO: Money = Money(0);

    pub const fn from_cents(cents: i64) -> Self {
        Money(cents)
    }
    pub const fn from_euros(euros: i64) -> Self {
        Money(euros * 100)
    }
    pub const fn cents(self) -> i64 {
        self.0
    }
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Arrondi à l'euro le plus proche (règle de l'impôt sur le revenu).
    pub fn round_to_euro(self) -> Money {
        Money(crate::div_round(self.0 as i128, 100) * 100)
    }
}

impl Add for Money {
    type Output = Money;
    fn add(self, o: Money) -> Money {
        Money(self.0 + o.0)
    }
}
impl Sub for Money {
    type Output = Money;
    fn sub(self, o: Money) -> Money {
        Money(self.0 - o.0)
    }
}
impl Neg for Money {
    type Output = Money;
    fn neg(self) -> Money {
        Money(-self.0)
    }
}
impl AddAssign for Money {
    fn add_assign(&mut self, o: Money) {
        self.0 += o.0;
    }
}
impl SubAssign for Money {
    fn sub_assign(&mut self, o: Money) {
        self.0 -= o.0;
    }
}
impl Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::ZERO, Add::add)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseMoneyError(pub String);

impl fmt::Display for ParseMoneyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "montant invalide : {}", self.0)
    }
}
impl std::error::Error for ParseMoneyError {}

impl FromStr for Money {
    type Err = ParseMoneyError;

    /// Accepte `1234`, `1234,5`, `1 234,56 €`, `-12.30`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || ParseMoneyError(s.to_string());
        let clean: String = s
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '€' && *c != '\u{202F}' && *c != '\u{A0}')
            .collect();
        let (neg, body) = match clean.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, clean.as_str()),
        };
        let cents = crate::rate::parse_decimal(body, 2).ok_or_else(err)?;
        Ok(Money(if neg { -cents } else { cents }))
    }
}

impl fmt::Display for Money {
    /// Format français : `12 345,67 €` (espace fine insécable pour les milliers).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let abs = self.0.unsigned_abs();
        let euros = (abs / 100).to_string();
        let mut groupe = String::new();
        for (i, c) in euros.chars().enumerate() {
            if i > 0 && (euros.len() - i) % 3 == 0 {
                groupe.push('\u{202F}');
            }
            groupe.push(c);
        }
        let signe = if self.0 < 0 { "-" } else { "" };
        write!(f, "{signe}{groupe},{:02}\u{A0}€", abs % 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affichage_francais() {
        assert_eq!(Money::from_cents(1234567).to_string(), "12\u{202F}345,67\u{A0}€");
        assert_eq!(Money::from_cents(-5).to_string(), "-0,05\u{A0}€");
        assert_eq!(Money::from_euros(100).to_string(), "100,00\u{A0}€");
    }

    #[test]
    fn lecture() {
        assert_eq!("1 234,5 €".parse::<Money>().unwrap(), Money::from_cents(123450));
        assert_eq!("-12.30".parse::<Money>().unwrap(), Money::from_cents(-1230));
        assert!("12,345".parse::<Money>().is_err());
    }

    #[test]
    fn arrondi_euro() {
        assert_eq!(Money::from_cents(360399).round_to_euro(), Money::from_euros(3604));
        assert_eq!(Money::from_cents(254333).round_to_euro(), Money::from_euros(2543));
        assert_eq!(Money::from_cents(150).round_to_euro(), Money::from_euros(2));
    }
}
