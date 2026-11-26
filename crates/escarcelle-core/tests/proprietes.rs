//! Fiabilité du moteur des livrets : des centaines de scénarios générés automatiquement.
//!
//! Chaque scénario (dates, montants, sens, taux) est tiré par un générateur pseudo-aléatoire à graine fixe : les
//! tests sont reproductibles, et un échec affiche la graine et le scénario. Trois familles de vérifications :
//!
//! 1. **Oracle indépendant** : un second calcul, écrit autrement (soldes recalculés de zéro à chaque quinzaine
//!    au lieu d'être cumulés), doit donner exactement les mêmes intérêts et soldes au centime près.
//! 2. **Propriétés** : invariance par l'ordre de saisie, par le déplacement d'un versement dans sa quinzaine, par
//!    le découpage d'un versement ; monotonie (verser plus ou à un taux plus élevé ne rapporte jamais moins) ;
//!    passage d'année (solde au 31/12 = solde de l'an passé + intérêts + mouvements de l'année).
//! 3. **Historique officiel des taux** : à chaque changement de taux depuis 2020, la quinzaine qui commence le jour
//!    du changement est payée au nouveau taux, celle d'avant à l'ancien.

use escarcelle_core::livret::{controler, simuler, AnneeLivret, Mouvement, Sens};
use escarcelle_core::params::{embarque, Series};
use escarcelle_core::{Date, Money, Rate};

// ---------- Générateur pseudo-aléatoire (xorshift64*), sans dépendance ----------

struct Alea(u64);

impl Alea {
    fn new(graine: u64) -> Self {
        Alea(graine.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn suivant(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Entier dans [a, b].
    fn entre(&mut self, a: i64, b: i64) -> i64 {
        a + (self.suivant() % (b - a + 1) as u64) as i64
    }
}

fn fin_de_mois(a: i32, m: u8) -> u8 {
    escarcelle_core::date::jours_dans_mois(a, m)
}

/// Date tirée entre février 2020 et juin 2027, avec une forte proportion de jours charnières
/// (1er, 2, 14, 15, 16, 17, fin de mois, 31 décembre, 1er janvier).
fn date(r: &mut Alea) -> Date {
    loop {
        let a = r.entre(2020, 2027) as i32;
        let m = r.entre(1, 12) as u8;
        if (a == 2020 && m == 1) || (a == 2027 && m > 6) {
            continue;
        }
        let fin = fin_de_mois(a, m);
        let j = match r.entre(0, 9) {
            0 => 1,
            1 => 2,
            2 => 14,
            3 => 15,
            4 => 16,
            5 => 17,
            6 => fin,
            _ => r.entre(1, fin as i64) as u8,
        };
        return Date::new(a, m, j).unwrap();
    }
}

fn montant(r: &mut Alea) -> Money {
    match r.entre(0, 4) {
        0 => Money::from_cents(r.entre(1, 99)),        // quelques centimes : arrondis
        1 => Money::from_cents(r.entre(100, 100_000)), // jusqu'à 1 000 €
        _ => Money::from_cents(r.entre(100_000, 3_000_000)),
    }
}

fn scenario(r: &mut Alea) -> Vec<Mouvement> {
    let n = r.entre(1, 12);
    (0..n)
        .map(|_| {
            let d = date(r);
            let m = montant(r);
            match r.entre(0, 99) {
                0..=59 => Mouvement::depot(d, m),
                60..=84 => Mouvement::retrait(d, m),
                _ => Mouvement::depart(d, m),
            }
        })
        .collect()
}

/// Taux : ceux du Livret A ou du LEP, ou une série inventée (changements à des dates quelconques, même en
/// milieu de quinzaine, taux de 0 à 8 %).
fn taux(r: &mut Alea) -> Series<Rate> {
    match r.entre(0, 2) {
        0 => embarque::livret_a().taux,
        1 => embarque::lep().taux,
        _ => {
            let mut points = vec![(d(2019, 1, 1), Rate::from_micro(r.entre(0, 80_000)))];
            for _ in 0..r.entre(0, 8) {
                points.push((date(r), Rate::from_micro(r.entre(0, 80_000))));
            }
            Series::new(points)
        }
    }
}

const JUSQU_A: i32 = 2027;

fn d(a: i32, m: u8, j: u8) -> Date {
    Date::new(a, m, j).unwrap()
}

// ---------- Oracle : calcul indépendant ----------

/// Rang absolu d'une quinzaine : 24 par an.
fn rang(annee: i32, q0: i64) -> i64 {
    annee as i64 * 24 + q0
}

/// Rang de la quinzaine contenant la date.
fn rang_de(d: Date) -> i64 {
    rang(d.year(), (d.month() as i64 - 1) * 2 + if d.day() >= 16 { 1 } else { 0 })
}

/// Premier rang de quinzaine où le mouvement compte dans le solde rémunéré (règles F2365) :
/// versement → quinzaine suivante ; retrait → retiré dès sa propre quinzaine ;
/// solde de départ → sa quinzaine s'il est daté d'un 1er ou d'un 16, sinon la suivante.
fn rang_effet(m: &Mouvement) -> i64 {
    let r = rang_de(m.date);
    match m.sens {
        Sens::Depot => r + 1,
        Sens::Retrait => r,
        Sens::Depart if m.date.day() == 1 || m.date.day() == 16 => r,
        Sens::Depart => r + 1,
    }
}

fn signe(m: &Mouvement) -> i64 {
    match m.sens {
        Sens::Retrait => -m.montant.cents(),
        _ => m.montant.cents(),
    }
}

/// Historique retenu : le solde de départ le plus ancien (puis le plus petit) et les mouvements datés à partir
/// de lui ; sans solde de départ, tout.
fn retenus(mvts: &[Mouvement]) -> Vec<Mouvement> {
    let depart = mvts.iter().filter(|m| m.sens == Sens::Depart).min_by_key(|m| (m.date, m.montant.cents()));
    match depart {
        None => mvts.to_vec(),
        Some(d) => {
            let mut v = vec![*d];
            v.extend(mvts.iter().filter(|m| m.sens != Sens::Depart && m.date >= d.date).copied());
            v
        }
    }
}

fn oracle(mvts: &[Mouvement], taux: &Series<Rate>, jusqu_a: i32) -> Vec<AnneeLivret> {
    let mvts = retenus(mvts);
    let Some(premier) = mvts.iter().map(rang_effet).min() else { return Vec::new() };
    let mut res = Vec::new();
    let mut capitalises: i64 = 0;
    for annee in premier.div_euclid(24) as i32..=jusqu_a {
        let mut cumul: i128 = 0;
        for q0 in 0..24 {
            let g = rang(annee, q0);
            // Solde rémunéré de la quinzaine, recalculé de zéro.
            let solde: i64 = mvts.iter().filter(|m| rang_effet(m) <= g).map(signe).sum::<i64>() + capitalises;
            if solde > 0 {
                let debut = Date::new(annee, (q0 / 2 + 1) as u8, if q0 % 2 == 0 { 1 } else { 16 }).unwrap();
                cumul += solde as i128 * taux.at(debut).unwrap().micro() as i128;
            }
        }
        // Arrondi au centime le plus proche, une fois par an (cumul toujours positif ici).
        let interets = ((cumul + 12_000_000) / 24_000_000) as i64;
        capitalises += interets;
        let fin: i64 = mvts.iter().filter(|m| rang_effet(m) <= rang(annee, 23)).map(signe).sum::<i64>() + capitalises;
        res.push(AnneeLivret { annee, interets: Money::from_cents(interets), solde_fin: Money::from_cents(fin) });
    }
    res
}

const SCENARIOS: u64 = 600;

fn pour_chaque_scenario(mut verifier: impl FnMut(u64, &mut Alea, Vec<Mouvement>, Series<Rate>)) {
    for graine in 1..=SCENARIOS {
        let mut r = Alea::new(graine);
        let t = taux(&mut r);
        let m = scenario(&mut r);
        verifier(graine, &mut r, m, t);
    }
}

fn calcul(m: &[Mouvement], t: &Series<Rate>) -> Vec<AnneeLivret> {
    simuler(m, t, JUSQU_A).unwrap_or_else(|e| panic!("erreur {e} pour {m:?}"))
}

// ---------- 1. Oracle ----------

#[test]
fn moteur_identique_a_l_oracle() {
    pour_chaque_scenario(|graine, _, m, t| {
        assert_eq!(calcul(&m, &t), oracle(&m, &t, JUSQU_A), "graine {graine} : {m:?}");
    });
}

// ---------- 2. Propriétés ----------

#[test]
fn ordre_de_saisie_sans_effet() {
    pour_chaque_scenario(|graine, r, m, t| {
        let reference = calcul(&m, &t);
        let mut melange = m.clone();
        for i in (1..melange.len()).rev() {
            let j = r.entre(0, i as i64) as usize;
            melange.swap(i, j);
        }
        assert_eq!(calcul(&melange, &t), reference, "graine {graine}");
    });
}

#[test]
fn interets_jamais_negatifs() {
    pour_chaque_scenario(|graine, _, m, t| {
        assert!(calcul(&m, &t).iter().all(|a| a.interets >= Money::ZERO), "graine {graine}");
    });
}

#[test]
fn passage_d_annee() {
    // Solde au 31/12 = solde au 31/12 précédent + intérêts de l'année + mouvements qui prennent effet dans l'année.
    pour_chaque_scenario(|graine, _, m, t| {
        let annees = calcul(&m, &t);
        let retenus = retenus(&m);
        let mut precedent = Money::ZERO;
        for a in &annees {
            let effet: i64 = retenus.iter().filter(|x| rang_effet(x).div_euclid(24) == a.annee as i64).map(signe).sum();
            assert_eq!(a.solde_fin, precedent + a.interets + Money::from_cents(effet), "graine {graine}, {}", a.annee);
            precedent = a.solde_fin;
        }
    });
}

/// Un versement de plus (daté après le solde de départ éventuel) ne fait jamais baisser les intérêts d'une année.
#[test]
fn verser_plus_ne_rapporte_jamais_moins() {
    pour_chaque_scenario(|graine, r, m, t| {
        let avant = calcul(&m, &t);
        let mut d = date(r);
        if let Some(dep) = retenus(&m).iter().find(|x| x.sens == Sens::Depart) {
            if d < dep.date {
                d = dep.date;
            }
        }
        let mut plus = m.clone();
        plus.push(Mouvement::depot(d, montant(r)));
        let apres = calcul(&plus, &t);
        for a in &avant {
            let b = apres.iter().find(|b| b.annee == a.annee).unwrap();
            assert!(b.interets >= a.interets, "graine {graine}, {} : {} < {}", a.annee, b.interets, a.interets);
        }
    });
}

/// Des taux plus élevés à chaque quinzaine ne rapportent jamais moins.
#[test]
fn taux_plus_eleve_ne_rapporte_jamais_moins() {
    pour_chaque_scenario(|graine, r, m, t| {
        let bonus = r.entre(1, 20_000);
        let hausse = Series::new(t.points().iter().map(|(d, x)| (*d, Rate::from_micro(x.micro() + bonus))).collect());
        let (a, b) = (calcul(&m, &t), calcul(&m, &hausse));
        for (x, y) in a.iter().zip(&b) {
            assert!(y.interets >= x.interets, "graine {graine}, {}", x.annee);
        }
    });
}

/// Déplacer un versement ou un retrait à un autre jour de la même quinzaine ne change rien.
#[test]
fn meme_quinzaine_meme_resultat() {
    pour_chaque_scenario(|graine, r, m, t| {
        let reference = calcul(&m, &t);
        let deplace: Vec<Mouvement> = m
            .iter()
            .map(|x| {
                if x.sens == Sens::Depart {
                    return *x; // la date d'un solde de départ compte aussi pour la normalisation
                }
                let (a, mo) = (x.date.year(), x.date.month());
                let (debut, fin) = if x.date.day() <= 15 { (1, 15) } else { (16, fin_de_mois(a, mo)) };
                let mut y = *x;
                y.date = Date::new(a, mo, r.entre(debut, fin as i64) as u8).unwrap();
                y
            })
            .collect();
        // La normalisation compare les dates au solde de départ : on ne garde que les scénarios où le
        // déplacement ne fait pas passer un mouvement de l'autre côté de ce solde.
        let cote = |v: &[Mouvement]| -> Vec<bool> {
            let d = v.iter().filter(|x| x.sens == Sens::Depart).map(|x| (x.date, x.montant.cents())).min();
            v.iter().map(|x| d.is_some_and(|(dd, _)| x.date >= dd)).collect()
        };
        if cote(&m) == cote(&deplace) {
            assert_eq!(calcul(&deplace, &t), reference, "graine {graine}");
        }
    });
}

/// Découper un mouvement en deux mouvements du même jour ne change rien (l'arrondi est annuel).
#[test]
fn decoupage_sans_effet() {
    pour_chaque_scenario(|graine, r, m, t| {
        let reference = calcul(&m, &t);
        let i = r.entre(0, m.len() as i64 - 1) as usize;
        if m[i].sens == Sens::Depart || m[i].montant.cents() < 2 {
            return;
        }
        let part = r.entre(1, m[i].montant.cents() - 1);
        let mut v = m.clone();
        v[i].montant = Money::from_cents(part);
        let mut reste = m[i];
        reste.montant = Money::from_cents(m[i].montant.cents() - part);
        v.push(reste);
        assert_eq!(calcul(&v, &t), reference, "graine {graine}");
    });
}

/// Le contrôle du plafond ne panique jamais, renvoie des indices valides, et ne dépend pas de l'ordre de saisie
/// quand les dates sont toutes différentes.
#[test]
fn controle_stable() {
    pour_chaque_scenario(|graine, r, m, t| {
        let plafond = Money::from_cents(r.entre(100_000, 3_000_000));
        let a = controler(&m, &t, plafond).unwrap();
        assert!(a.iter().all(|(i, _)| *i < m.len()), "graine {graine}");
        let mut dates: Vec<Date> = m.iter().map(|x| x.date).collect();
        dates.sort();
        dates.dedup();
        if dates.len() == m.len() {
            let mut inverse = m.clone();
            inverse.reverse();
            let b = controler(&inverse, &t, plafond).unwrap();
            let cle = |v: &[Mouvement], a: &[(usize, escarcelle_core::livret::Anomalie)]| {
                let mut k: Vec<String> = a.iter().map(|(i, x)| format!("{:?} {:?}", v[*i], x)).collect();
                k.sort();
                k
            };
            assert_eq!(cle(&m, &a), cle(&inverse, &b), "graine {graine}");
        }
    });
}

// ---------- 3. Historique officiel des taux ----------

/// À chaque changement officiel de taux (Livret A, LDDS, LEP depuis 2020) : 10 000 € présents pendant la seule
/// quinzaine qui commence le jour du changement rapportent 10 000 × nouveau taux / 24 ; présents pendant la
/// quinzaine précédente, ils rapportent 10 000 × ancien taux / 24.
#[test]
fn chaque_changement_officiel_de_taux() {
    let capital = Money::from_euros(10_000);
    let une_quinzaine = |debut: Date, t: &Series<Rate>| -> Money {
        // Solde de départ au début de la quinzaine, retrait total au début de la suivante.
        let fin = if debut.day() == 1 {
            Date::new(debut.year(), debut.month(), 16).unwrap()
        } else if debut.month() == 12 {
            d(debut.year() + 1, 1, 1)
        } else {
            Date::new(debut.year(), debut.month() + 1, 1).unwrap()
        };
        let m = [Mouvement::depart(debut, capital), Mouvement::retrait(fin, capital)];
        simuler(&m, t, debut.year()).unwrap().iter().find(|a| a.annee == debut.year()).unwrap().interets
    };
    let attendu = |taux: Rate| Money::from_cents((capital.cents() * taux.micro() + 12_000_000) / 24_000_000);
    let mut changements = 0;
    for p in [embarque::livret_a(), embarque::ldds(), embarque::lep()] {
        let points = p.taux.points();
        for w in points.windows(2) {
            let ((_, ancien), (jour, nouveau)) = (w[0], w[1]);
            assert_eq!(jour.day(), 1, "les taux changent un 1er du mois");
            assert_eq!(une_quinzaine(jour, &p.taux), attendu(nouveau), "{jour} : nouveau taux");
            let veille = if jour.month() == 1 {
                d(jour.year() - 1, 12, 16)
            } else {
                Date::new(jour.year(), jour.month() - 1, 16).unwrap()
            };
            assert_eq!(une_quinzaine(veille, &p.taux), attendu(ancien), "{veille} : ancien taux");
            changements += 1;
        }
    }
    assert!(changements >= 20, "{changements} changements vérifiés");
}

/// Capitalisation au passage d'année : 10 000 € à taux constant de 2 % pendant 3 ans donnent
/// 200,00 €, puis 204,00 €, puis 208,08 € (les intérêts de l'année rapportent à leur tour l'année suivante).
#[test]
fn capitalisation_sur_trois_ans() {
    let t = Series::new(vec![(d(2019, 1, 1), Rate::from_micro(20_000))]);
    let a = simuler(&[Mouvement::depart(d(2024, 1, 1), Money::from_euros(10_000))], &t, 2026).unwrap();
    let interets: Vec<i64> = a.iter().map(|x| x.interets.cents()).collect();
    assert_eq!(interets, vec![20_000, 20_400, 20_808]);
    assert_eq!(a[2].solde_fin, Money::from_cents(1_061_208));
}
