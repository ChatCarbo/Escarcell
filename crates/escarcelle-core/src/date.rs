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

    /// Date d'un nombre de jours depuis le 1er janvier 1970 (algorithme de H. Hinnant).
    pub fn depuis_jours_unix(jours: i64) -> Self {
        let z = jours + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u8;
        let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
        Date { year: y, month: m, day: d }
    }

    /// Nombre de jours depuis le 1er janvier 1970.
    pub fn jours_unix(self) -> i64 {
        let (m, j) = (self.month as i64, self.day as i64);
        let y = self.year as i64 - if m <= 2 { 1 } else { 0 };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + j - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
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

    #[test]
    fn jours_unix_aller_retour() {
        assert_eq!(Date::ymd(1970, 1, 1).jours_unix(), 0);
        assert_eq!(Date::ymd(2026, 1, 5).jours_unix() * 86_400, 1_767_571_200);
        assert_eq!(Date::ymd(2024, 2, 29).jours_unix() * 86_400, 1_709_164_800);
        for j in [-1000, 0, 10_957, 20_000, 20_516] {
            assert_eq!(Date::depuis_jours_unix(j).jours_unix(), j);
        }
        assert_eq!(Date::depuis_jours_unix(20_416), Date::ymd(2025, 11, 24));
    }
}
