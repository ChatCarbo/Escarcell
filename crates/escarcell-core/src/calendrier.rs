//! Calendrier des dividendes du PEA : détachements à venir (annoncés ou estimés), dividendes à rapprocher
//! des opérations enregistrées, et répartition par mois.
//!
//! Estimation d'un détachement à venir :
//! - **date** : chaque détachement des 12 derniers mois, décalé d'un an (même rythme que l'an dernier) ;
//! - **montant par action** : celui du même coupon l'an dernier. Si le dernier dividende détaché a changé par
//!   rapport à son équivalent un an plus tôt, le même changement est appliqué aux coupons qui le précèdent
//!   (TotalEnergies : acomptes de 0,85 € puis dernier acompte à 0,90 € au lieu de 0,85 € → les acomptes suivants
//!   sont estimés à 0,90 €). Un changement de plus de 50 % (dividende exceptionnel, division d'actions) n'est
//!   pas appliqué ;
//! - un détachement déjà connu de la source (date future dans l'historique) est « annoncé » et remplace
//!   l'estimation de la même période.

use crate::dividendes::Versement;
use crate::{Date, Money, Price};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Statut {
    /// Date et montant publiés par la source.
    Annonce,
    /// Déduit des détachements de l'an dernier.
    Estime,
}

/// Détachement à venir d'un titre (montant par action).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prevu {
    pub date: Date,
    pub par_action: Price,
    pub statut: Statut,
}

/// Écart maximal (en jours) entre un détachement estimé et un détachement annoncé de la même période.
const MEME_PERIODE_JOURS: i64 = 45;

fn ecart(a: Date, b: Date) -> i64 {
    (a.jours_unix() - b.jours_unix()).abs()
}

/// Changement du dernier dividende par rapport au même coupon un an plus tôt : (nouveau, ancien) en
/// millionièmes d'euro, ou `None` s'il n'y a pas d'équivalent, pas de changement, ou un changement hors de
/// ±50 %.
fn changement(historique: &[Versement], dernier: Versement) -> Option<(i64, i64)> {
    let un_an_avant = dernier.date.plus_annees(-1);
    let ancien = historique
        .iter()
        .filter(|v| v.date < dernier.date && ecart(v.date, un_an_avant) <= MEME_PERIODE_JOURS)
        .min_by_key(|v| ecart(v.date, un_an_avant))?;
    let (n, a) = (dernier.montant.micro(), ancien.montant.micro());
    (a > 0 && n != a && n * 2 >= a && n * 2 <= a * 3).then_some((n, a))
}

/// Détachements à venir d'un titre, de demain jusqu'à `horizon` inclus (au plus un an après aujourd'hui),
/// triés par date.
pub fn prevus(historique: &[Versement], aujourd_hui: Date, horizon: Date) -> Vec<Prevu> {
    let horizon = horizon.min(aujourd_hui.plus_annees(1));
    let mut res: Vec<Prevu> = historique
        .iter()
        .filter(|v| v.date > aujourd_hui && v.date <= horizon && v.montant.micro() > 0)
        .map(|v| Prevu { date: v.date, par_action: v.montant, statut: Statut::Annonce })
        .collect();
    let il_y_a_un_an = aujourd_hui.plus_annees(-1);
    let recents: Vec<Versement> = historique
        .iter()
        .filter(|v| v.date > il_y_a_un_an && v.date <= aujourd_hui && v.montant.micro() > 0)
        .copied()
        .collect();
    let dernier = recents.iter().max_by_key(|v| v.date).copied();
    let hausse = dernier.and_then(|d| changement(historique, d));
    for v in &recents {
        let date = v.date.plus_annees(1);
        if date > horizon
            || res.iter().any(|p| p.statut == Statut::Annonce && ecart(p.date, date) <= MEME_PERIODE_JOURS)
        {
            continue;
        }
        let montant = match (hausse, dernier) {
            (Some((n, a)), Some(d)) if v.date < d.date => {
                crate::div_round(v.montant.micro() as i128 * n as i128, a as i128)
            }
            _ => v.montant.micro(),
        };
        res.push(Prevu { date, par_action: Price::from_micro(montant), statut: Statut::Estime });
    }
    res.sort_by_key(|p| (p.date, p.statut));
    res
}

/// Détachements à rapprocher : ceux de `detaches` (dates de détachement) qu'aucune opération de dividende
/// n'explique. Une opération explique un détachement si elle est datée de 7 jours avant à 60 jours après ;
/// chaque opération n'en explique qu'un (le plus proche, dans l'ordre des dates).
pub fn non_rapproches(detaches: &[Date], operations: &[Date]) -> Vec<Date> {
    let mut libres: Vec<Date> = operations.to_vec();
    libres.sort();
    let mut tries = detaches.to_vec();
    tries.sort();
    let mut res = Vec::new();
    for d in tries {
        let candidat = libres
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                let j = o.jours_unix() - d.jours_unix();
                (-7..=60).contains(&j)
            })
            .min_by_key(|(_, o)| ecart(**o, d))
            .map(|(i, _)| i);
        match candidat {
            Some(i) => {
                libres.remove(i);
            }
            None => res.push(d),
        }
    }
    res
}

/// Catégorie d'un montant dans la vue par mois.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Categorie {
    Percu,
    Annonce,
    Estime,
}

/// Totaux par mois d'une année : `[perçu, annoncé, estimé]` pour janvier (indice 0) à décembre.
pub fn par_mois(annee: i32, montants: &[(Date, Money, Categorie)]) -> [[Money; 3]; 12] {
    let mut res = [[Money::ZERO; 3]; 12];
    for (d, m, c) in montants {
        if d.year() != annee {
            continue;
        }
        let k = match c {
            Categorie::Percu => 0,
            Categorie::Annonce => 1,
            Categorie::Estime => 2,
        };
        res[(d.month() - 1) as usize][k] += *m;
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: i32, m: u8, j: u8, micro: i64) -> Versement {
        Versement { date: Date::ymd(a, m, j), montant: Price::from_micro(micro) }
    }

    fn p(a: i32, m: u8, j: u8, micro: i64, statut: Statut) -> Prevu {
        Prevu { date: Date::ymd(a, m, j), par_action: Price::from_micro(micro), statut }
    }

    /// TotalEnergies fin novembre 2026 : trois acomptes de 0,85 € puis un de 0,90 €.
    #[test]
    fn trimestriel_avec_hausse() {
        let h = [
            v(2025, 9, 30, 850_000),
            v(2025, 12, 31, 850_000),
            v(2026, 3, 31, 850_000),
            v(2026, 6, 30, 850_000),
            v(2026, 9, 30, 900_000),
        ];
        let aujourd_hui = Date::ymd(2026, 11, 27);
        assert_eq!(
            prevus(&h, aujourd_hui, aujourd_hui.plus_annees(1)),
            vec![
                p(2026, 12, 31, 900_000, Statut::Estime),
                p(2027, 3, 31, 900_000, Statut::Estime),
                p(2027, 6, 30, 900_000, Statut::Estime),
                p(2027, 9, 30, 900_000, Statut::Estime),
            ]
        );
    }

    /// Acompte et solde différents (LVMH) : chaque coupon garde son montant.
    #[test]
    fn acompte_et_solde_differents() {
        let h = [v(2025, 4, 24, 7_500_000), v(2025, 12, 4, 5_500_000), v(2026, 4, 23, 7_500_000)];
        let aujourd_hui = Date::ymd(2026, 11, 27);
        let r = prevus(&h, aujourd_hui, aujourd_hui.plus_annees(1));
        assert_eq!(r, vec![p(2026, 12, 4, 5_500_000, Statut::Estime), p(2027, 4, 23, 7_500_000, Statut::Estime)]);
    }

    #[test]
    fn dividende_exceptionnel_non_extrapole() {
        let h = [v(2025, 6, 1, 1_000_000), v(2025, 12, 1, 1_000_000), v(2026, 6, 1, 3_000_000)];
        let r = prevus(&h, Date::ymd(2026, 7, 1), Date::ymd(2027, 7, 1));
        assert_eq!(r, vec![p(2026, 12, 1, 1_000_000, Statut::Estime), p(2027, 6, 1, 3_000_000, Statut::Estime)]);
    }

    #[test]
    fn annonce_remplace_l_estimation() {
        let h = [v(2025, 12, 31, 850_000), v(2026, 9, 30, 900_000), v(2026, 12, 31, 920_000)];
        let aujourd_hui = Date::ymd(2026, 11, 27);
        let r = prevus(&h, aujourd_hui, Date::ymd(2027, 1, 31));
        assert_eq!(r, vec![p(2026, 12, 31, 920_000, Statut::Annonce)]);
    }

    #[test]
    fn horizon_et_historique_ancien() {
        let h = [v(2024, 5, 1, 1_000_000), v(2025, 5, 2, 1_100_000)];
        // Aucun détachement dans les 12 derniers mois : rien à estimer.
        assert!(prevus(&h, Date::ymd(2026, 11, 27), Date::ymd(2027, 11, 27)).is_empty());
        // Horizon court : le coupon de mai n'y est pas.
        assert!(prevus(&h, Date::ymd(2025, 11, 27), Date::ymd(2026, 3, 1)).is_empty());
        let r = prevus(&h, Date::ymd(2025, 11, 27), Date::ymd(2026, 12, 31));
        assert_eq!(r, vec![p(2026, 5, 2, 1_100_000, Statut::Estime)]);
    }

    #[test]
    fn rapprochement() {
        let d = |a, m, j| Date::ymd(a, m, j);
        // Deux détachements, une seule opération 3 jours après le premier.
        assert_eq!(non_rapproches(&[d(2026, 6, 30), d(2026, 9, 30)], &[d(2026, 7, 3)]), vec![d(2026, 9, 30)]);
        // Opération trop tardive (plus de 60 jours) : non rapprochée.
        assert_eq!(non_rapproches(&[d(2026, 6, 30)], &[d(2026, 9, 1)]), vec![d(2026, 6, 30)]);
        // Une opération n'explique qu'un détachement.
        assert_eq!(non_rapproches(&[d(2026, 6, 30), d(2026, 7, 2)], &[d(2026, 7, 1)]), vec![d(2026, 7, 2)]);
        assert!(non_rapproches(&[d(2026, 6, 30)], &[d(2026, 6, 25)]).is_empty());
    }

    #[test]
    fn totaux_par_mois() {
        let m = par_mois(
            2026,
            &[
                (Date::ymd(2026, 3, 31), Money::from_euros(10), Categorie::Percu),
                (Date::ymd(2026, 3, 2), Money::from_euros(5), Categorie::Percu),
                (Date::ymd(2026, 12, 31), Money::from_euros(45), Categorie::Estime),
                (Date::ymd(2026, 12, 15), Money::from_euros(1), Categorie::Annonce),
                (Date::ymd(2027, 1, 1), Money::from_euros(99), Categorie::Estime),
            ],
        );
        assert_eq!(m[2], [Money::from_euros(15), Money::ZERO, Money::ZERO]);
        assert_eq!(m[11], [Money::ZERO, Money::from_euros(1), Money::from_euros(45)]);
        assert_eq!(m.iter().flatten().map(|x| x.cents()).sum::<i64>(), 6_100);
    }

    /// Historiques générés : les détachements prévus sont bien datés, triés, positifs, au plus un par
    /// détachement des 12 derniers mois (plus les annonces), et ne s'écartent jamais de plus de 50 % du coupon
    /// de l'an dernier.
    #[test]
    fn proprietes_sur_500_historiques() {
        let mut x: u64 = 0xD1F1_DE4D_E5CA_1E00;
        let mut alea = |n: u64| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x % n
        };
        for _ in 0..500 {
            let aujourd_hui = Date::depuis_jours_unix(Date::ymd(2024, 1, 1).jours_unix() + alea(1_000) as i64);
            let debut = aujourd_hui.plus_annees(-3).jours_unix();
            let n = alea(16) as usize;
            let mut h: Vec<Versement> = (0..n)
                .map(|_| Versement {
                    date: Date::depuis_jours_unix(debut + alea(1_300) as i64),
                    montant: Price::from_micro(1 + alea(5_000_000) as i64),
                })
                .collect();
            h.sort_by_key(|v| v.date);
            let horizon = aujourd_hui.plus_annees(1);
            let r = prevus(&h, aujourd_hui, horizon);
            let recents = h.iter().filter(|v| v.date > aujourd_hui.plus_annees(-1) && v.date <= aujourd_hui).count();
            let annonces = h.iter().filter(|v| v.date > aujourd_hui && v.date <= horizon).count();
            assert!(r.len() <= recents + annonces);
            assert!(r.windows(2).all(|w| w[0].date <= w[1].date));
            for p in &r {
                assert!(p.date > aujourd_hui && p.date <= horizon);
                assert!(p.par_action.micro() > 0);
                if p.statut == Statut::Estime {
                    // Coupon d'origine (plusieurs détachements peuvent tomber le même jour).
                    let e = p.par_action.micro();
                    assert!(
                        h.iter().filter(|v| v.date.plus_annees(1) == p.date).any(|v| {
                            let a = v.montant.micro();
                            e * 2 >= a && e <= a * 3 / 2 + 1
                        }),
                        "{e} sans coupon d'origine compatible"
                    );
                }
            }
        }
    }
}
