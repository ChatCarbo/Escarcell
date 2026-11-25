//! Livret A, LDDS, LEP : intérêts par quinzaines.
//!
//! - Un dépôt est rémunéré à partir de la quinzaine **suivante**.
//! - Un retrait cesse d'être rémunéré dès le début de la quinzaine **en cours**.
//! - Chaque quinzaine est rémunérée au taux en vigueur à son premier jour.
//! - Les intérêts sont arrondis au centime une seule fois et capitalisés au 31 décembre.
//! - Le plafond limite les dépôts ; seule la capitalisation des intérêts peut le dépasser.
//! - Un **solde de départ** (lu sur un relevé, intérêts compris) remplace l'historique antérieur et n'est pas
//!   soumis au plafond. Daté d'un 1er ou d'un 16, il est rémunéré dès cette quinzaine ; sinon, à partir de la
//!   suivante, comme un versement : un solde au 31 décembre comprend déjà les intérêts de l'année, il ne doit pas
//!   rapporter une seconde fois la quinzaine du 16 au 31 décembre.

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
    /// Solde constaté sur un relevé, point de départ du calcul.
    Depart,
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
    pub fn depart(date: Date, montant: Money) -> Self {
        Self { date, sens: Sens::Depart, montant }
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
        // Solde de départ daté d'un début de quinzaine : il rapporte dès cette quinzaine.
        Sens::Depart if d.day() == 1 || d.day() == 16 => (d.year(), q),
        Sens::Depart | Sens::Depot if q == 24 => (d.year() + 1, 1),
        Sens::Depart | Sens::Depot => (d.year(), q + 1),
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
            Sens::Depot | Sens::Depart => m.montant.cents(),
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

/// Mouvement impossible à la date où il est daté.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anomalie {
    /// Dépôt au-delà du plafond : `marge` est le dépôt maximum possible ce jour-là.
    PlafondDepasse { marge: Money },
    /// Retrait supérieur au solde du jour.
    SoldeInsuffisant { solde: Money },
    /// Mouvement daté avant le solde de départ (déjà compris dans ce solde).
    AvantSoldeDepart { depart: Date },
    /// Second solde de départ pour le même livret.
    SoldeDepartEnDouble,
}

/// Contrôle chaque mouvement **à sa date** : un dépôt ne doit pas faire dépasser le plafond, un retrait
/// ne doit pas dépasser le solde. Le solde d'un jour comprend les mouvements des jours précédents et les
/// intérêts capitalisés aux 31 décembre des années précédentes.
///
/// Mouvements d'un même jour : l'ordre de saisie ne compte pas. Un dépôt tient compte des retraits du jour
/// (retirer 12 000 € puis les reverser le même jour est possible au plafond), et un retrait tient compte des
/// dépôts du jour.
///
/// C'est le solde à la date du mouvement qui compte, pas le solde d'aujourd'hui : un versement daté de 2021
/// se juge sur le solde de 2021, sans les intérêts capitalisés depuis.
///
/// Rend la liste des anomalies, avec l'indice du mouvement dans `mouvements`.
pub fn controler(
    mouvements: &[Mouvement],
    taux: &Series<Rate>,
    plafond: Money,
) -> Result<Vec<(usize, Anomalie)>, LivretError> {
    let mut ordre: Vec<usize> = (0..mouvements.len()).collect();
    ordre.sort_by_key(|&i| mouvements[i].date); // tri stable : ordre de saisie conservé pour un même jour
    let derniere = match ordre.last() {
        Some(&i) => mouvements[i].date.year(),
        None => return Ok(Vec::new()),
    };
    // Intérêts capitalisés au 31 décembre de chaque année (ceux d'une année ne dépendent que des mouvements
    // datés de cette année ou d'avant).
    let interets: BTreeMap<i32, i64> =
        simuler(mouvements, taux, derniere)?.into_iter().map(|a| (a.annee, a.interets.cents())).collect();

    let mut anomalies = Vec::new();
    // Le premier solde de départ saisi fait foi ; les autres sont en double.
    let depart = mouvements.iter().position(|m| m.sens == Sens::Depart);
    if let Some(d) = depart {
        for (i, m) in mouvements.iter().enumerate() {
            if i == d {
                continue;
            }
            if m.sens == Sens::Depart {
                anomalies.push((i, Anomalie::SoldeDepartEnDouble));
            } else if m.date < mouvements[d].date {
                anomalies.push((i, Anomalie::AvantSoldeDepart { depart: mouvements[d].date }));
            }
        }
    }
    let mut operations: i64 = 0; // somme des mouvements des jours précédents
    let mut k = 0;
    while k < ordre.len() {
        // Mouvements du même jour.
        let jour = mouvements[ordre[k]].date;
        let fin = ordre[k..].iter().position(|&i| mouvements[i].date != jour).map_or(ordre.len(), |n| k + n);
        let du_jour = &ordre[k..fin];
        let capitalises: i64 = interets.range(..jour.year()).map(|(_, v)| v).sum();
        let somme = |sens: Sens| -> i64 {
            du_jour.iter().filter(|&&i| mouvements[i].sens == sens).map(|&i| mouvements[i].montant.cents()).sum()
        };
        let (depots, retraits, departs) = (somme(Sens::Depot), somme(Sens::Retrait), somme(Sens::Depart));
        let debut = operations + capitalises + departs;
        // Retraits : sur le solde du début du jour plus les dépôts du jour, dans l'ordre de saisie.
        let mut disponible = debut + depots;
        for &i in du_jour.iter().filter(|&&i| mouvements[i].sens == Sens::Retrait) {
            let m = mouvements[i].montant.cents();
            if m > disponible {
                anomalies.push((i, Anomalie::SoldeInsuffisant { solde: Money::from_cents(disponible.max(0)) }));
            }
            disponible -= m;
        }
        // Dépôts : sur le solde du début du jour moins les retraits du jour, dans l'ordre de saisie.
        let mut solde = debut - retraits;
        for &i in du_jour.iter().filter(|&&i| mouvements[i].sens == Sens::Depot) {
            let m = mouvements[i].montant.cents();
            let marge = (plafond.cents() - solde).max(0);
            if m > marge {
                anomalies.push((i, Anomalie::PlafondDepasse { marge: Money::from_cents(marge) }));
            }
            solde += m;
        }
        operations += departs + depots - retraits;
        k = fin;
    }
    anomalies.sort_by_key(|(i, _)| *i);
    Ok(anomalies)
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

    fn taux_fixe(pct: i64) -> Series<Rate> {
        Series::new(vec![(Date::ymd(2000, 1, 1), Rate::from_micro(pct * 10_000))])
    }

    /// Cas signalé : 2 000 € versés en mai 2021, le solde d'aujourd'hui (intérêts compris) est de 2 202,48 €.
    /// Un versement daté du 18/05/2021 se juge sur le solde de mai 2021 (2 000 €), pas sur celui d'aujourd'hui.
    #[test]
    fn plafond_juge_a_la_date_du_versement() {
        let p = Money::from_euros(22_950);
        let t = taux_fixe(2);
        let mut m = vec![Mouvement::depot(Date::ymd(2021, 5, 3), Money::from_euros(2_000))];
        m.push(Mouvement::depot(Date::ymd(2021, 5, 18), Money::from_euros(20_940)));
        assert_eq!(controler(&m, &t, p).unwrap(), vec![]);
        // 20 951 € dépasse de 1 € : marge de 20 950 € ce jour-là.
        m[1].montant = Money::from_euros(20_951);
        assert_eq!(
            controler(&m, &t, p).unwrap(),
            vec![(1, Anomalie::PlafondDepasse { marge: Money::from_euros(20_950) })]
        );
    }

    #[test]
    fn interets_capitalises_comptes_a_partir_de_l_annee_suivante() {
        let p = Money::from_euros(1_000);
        let t = taux_fixe(10);
        // 900 € versés le 1er janvier 2024 rapportent des intérêts, capitalisés au 31/12/2024.
        let base = vec![Mouvement::depot(Date::ymd(2024, 1, 1), Money::from_euros(900))];
        let interets = simuler(&base, &t, 2024).unwrap()[0].interets;
        assert!(interets > Money::ZERO);
        // En décembre 2024, la marge est de 100 € ; en janvier 2025, elle a diminué des intérêts.
        let mut m = base.clone();
        m.push(Mouvement::depot(Date::ymd(2024, 12, 31), Money::from_euros(100)));
        assert_eq!(controler(&m, &t, p).unwrap(), vec![]);
        let mut m = base.clone();
        m.push(Mouvement::depot(Date::ymd(2025, 1, 2), Money::from_euros(100)));
        assert_eq!(
            controler(&m, &t, p).unwrap(),
            vec![(1, Anomalie::PlafondDepasse { marge: Money::from_euros(100) - interets })]
        );
    }

    #[test]
    fn versement_anterieur_qui_bloque_un_versement_posterieur() {
        let p = Money::from_euros(10_000);
        let t = taux_fixe(0);
        let m = vec![
            Mouvement::depot(Date::ymd(2025, 6, 1), Money::from_euros(9_000)),
            // Saisi après coup mais daté avant : à sa date, il passe ; c'est le versement de juin qui dépasse.
            Mouvement::depot(Date::ymd(2025, 3, 1), Money::from_euros(2_000)),
        ];
        assert_eq!(
            controler(&m, &t, p).unwrap(),
            vec![(0, Anomalie::PlafondDepasse { marge: Money::from_euros(8_000) })]
        );
    }

    #[test]
    fn retrait_superieur_au_solde_du_jour() {
        let p = Money::from_euros(22_950);
        let t = taux_fixe(0);
        let m = vec![
            Mouvement::depot(Date::ymd(2025, 6, 1), Money::from_euros(500)),
            Mouvement::retrait(Date::ymd(2025, 3, 1), Money::from_euros(100)),
        ];
        assert_eq!(controler(&m, &t, p).unwrap(), vec![(1, Anomalie::SoldeInsuffisant { solde: Money::ZERO })]);
        assert_eq!(controler(&[], &t, p).unwrap(), vec![]);
    }

    /// Livret plein depuis longtemps : 23 100 € au 31/12/2025, au-delà du plafond grâce aux intérêts.
    /// Le solde de départ est accepté, et rapporte dès la première quinzaine de 2026.
    #[test]
    fn solde_de_depart_au_dela_du_plafond() {
        let p = Money::from_euros(22_950);
        let t = taux_fixe(2);
        let m = vec![Mouvement::depart(Date::ymd(2026, 1, 1), Money::from_euros(23_100))];
        assert_eq!(controler(&m, &t, p).unwrap(), vec![]);
        // 23 100 € × 2 % sur l'année entière = 462 € (24 quinzaines).
        assert_eq!(simuler(&m, &t, 2026).unwrap()[0].interets, Money::from_euros(462));
        // Un versement de 1 € reste refusé : le solde dépasse déjà le plafond.
        let mut m2 = m.clone();
        m2.push(Mouvement::depot(Date::ymd(2026, 3, 1), Money::from_euros(1)));
        assert_eq!(controler(&m2, &t, p).unwrap(), vec![(1, Anomalie::PlafondDepasse { marge: Money::ZERO })]);
    }

    #[test]
    fn solde_de_depart_unique_et_premier() {
        let p = Money::from_euros(22_950);
        let t = taux_fixe(2);
        let m = vec![
            Mouvement::depart(Date::ymd(2025, 1, 1), Money::from_euros(5_000)),
            Mouvement::depot(Date::ymd(2024, 6, 1), Money::from_euros(100)),
            Mouvement::depart(Date::ymd(2025, 6, 1), Money::from_euros(6_000)),
        ];
        let a = controler(&m, &t, p).unwrap();
        assert!(a.contains(&(1, Anomalie::AvantSoldeDepart { depart: Date::ymd(2025, 1, 1) })));
        assert!(a.contains(&(2, Anomalie::SoldeDepartEnDouble)));
    }

    /// Cas signalé : solde de départ de 22 950 € au 31/12/2025, puis le 04/05/2026 un retrait de 12 000 € et un
    /// versement de 12 000 €. Le solde du 31/12 comprend déjà les intérêts de 2025 : il ne doit pas rapporter la
    /// quinzaine du 16 au 31 décembre, et le versement du même jour que le retrait doit passer.
    #[test]
    fn retrait_et_versement_le_meme_jour_au_plafond() {
        let p = Money::from_euros(22_950);
        let t = taux_fixe(2);
        let m = vec![
            Mouvement::depart(Date::ymd(2025, 12, 31), Money::from_euros(22_950)),
            Mouvement::retrait(Date::ymd(2026, 5, 4), Money::from_euros(12_000)),
            Mouvement::depot(Date::ymd(2026, 5, 4), Money::from_euros(12_000)),
        ];
        // Aucun intérêt en 2025 : le solde ne rapporte qu'à partir de la 1re quinzaine de 2026.
        assert_eq!(simuler(&m[..1], &t, 2026).unwrap()[0].annee, 2026);
        assert_eq!(controler(&m, &t, p).unwrap(), vec![]);
        // Saisis dans l'autre ordre (versement d'abord), c'est la même chose.
        let inverse = vec![m[0], m[2], m[1]];
        assert_eq!(controler(&inverse, &t, p).unwrap(), vec![]);
        // Un euro de plus reste refusé.
        let mut trop = m.clone();
        trop[2].montant = Money::from_euros(12_001);
        assert_eq!(
            controler(&trop, &t, p).unwrap(),
            vec![(2, Anomalie::PlafondDepasse { marge: Money::from_euros(12_000) })]
        );
    }

    #[test]
    fn solde_de_depart_date_d_un_debut_de_quinzaine() {
        assert_eq!(quinzaine_effet(Date::ymd(2026, 1, 1), Sens::Depart), (2026, 1));
        assert_eq!(quinzaine_effet(Date::ymd(2026, 3, 16), Sens::Depart), (2026, 6));
        assert_eq!(quinzaine_effet(Date::ymd(2026, 3, 10), Sens::Depart), (2026, 6));
        assert_eq!(quinzaine_effet(Date::ymd(2025, 12, 31), Sens::Depart), (2026, 1));
    }
}
