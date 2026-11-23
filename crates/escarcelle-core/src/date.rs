//! Date civile minimale (année, mois, jour), suffisante pour les règles métier.

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    year: i32,
    month: u8,
    day: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateError(pub String);

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "date invalide : {}", self.0)
    }
}
impl std::error::Error for DateError {}

pub fn est_bissextile(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

pub fn jours_dans_mois(year: i32, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if est_bissextile(year) => 29,
        2 => 28,
        _ => 0,
    }
}

impl Date {
    pub fn new(year: i32, month: u8, day: u8) -> Result<Self, DateError> {
        if !(1..=12).contains(&month) || day == 0 || day > jours_dans_mois(year, month) {
            return Err(DateError(format!("{year:04}-{month:02}-{day:02}")));
        }
        Ok(Self { year, month, day })
    }

    /// Constructeur pour les dates codées en dur (paniquer = bug du programme).
    pub(crate) fn ymd(year: i32, month: u8, day: u8) -> Self {
        Self::new(year, month, day).expect("date codée en dur invalide")
    }

    /// Même jour `annees` plus tard (un 29 février devient 28 février si besoin).
    pub fn plus_annees(self, annees: i32) -> Self {
        let y = self.year + annees;
        let d = self.day.min(jours_dans_mois(y, self.month));
        Self { year: y, month: self.month, day: d }
    }

    pub fn year(self) -> i32 {
        self.year
    }
    pub fn month(self) -> u8 {
        self.month
    }
    pub fn day(self) -> u8 {
        self.day
    }
}

impl FromStr for Date {
    type Err = DateError;

    /// Format ISO `AAAA-MM-JJ`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let err = || DateError(s.to_string());
        let mut it = s.trim().splitn(3, '-');
        let y = it.next().and_then(|v| v.parse().ok()).ok_or_else(err)?;
        let m = it.next().and_then(|v| v.parse().ok()).ok_or_else(err)?;
        let d = it.next().and_then(|v| v.parse().ok()).ok_or_else(err)?;
        Date::new(y, m, d)
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        assert!(Date::new(2024, 2, 29).is_ok());
        assert!(Date::new(2026, 2, 29).is_err());
        assert!(Date::new(2026, 13, 1).is_err());
        assert_eq!("2026-08-01".parse::<Date>().unwrap(), Date::ymd(2026, 8, 1));
    }
}
