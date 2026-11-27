//! Tests de robustesse : entrées vides, courtes, constantes, extrêmes ou mal formées.
//! Aucune fonction du cœur ne doit paniquer, et les séries calculées doivent garder la longueur de l'historique
//! (l'application lit `analyse.sma20[i]` pour chaque bougie affichée).

use escarcell_core::analyse::{analyser, Bougie};
use escarcell_core::chandeliers::{
    confirmation_multi_periodes, double_sommet_ou_fond, hebdomadaire, heikin_ashi, motifs, structure,
};
use escarcell_core::dividendes::{dividende_net, pays_isin, rendement, synthese, Versement};
use escarcell_core::identifiants::isin_valide;
use escarcell_core::{Date, Money, Price, Rate};

const JOUR: i64 = 86_400;
/// Lundi 5 janvier 2026, 0 h UTC.
const T0: i64 = 1_767_571_200;

fn serie(n: usize, prix: impl Fn(usize) -> f64) -> Vec<Bougie> {
    (0..n)
        .map(|i| {
            let p = prix(i);
            Bougie { t: T0 + i as i64 * JOUR, ouverture: p, haut: p, bas: p, cloture: p, volume: 0.0 }
        })
        .collect()
}

fn verifier_analyse(b: &[Bougie]) {
    let a = analyser(b);
    let n = b.len();
    assert_eq!(a.sma20.len(), n);
    assert_eq!(a.sma50.len(), n);
    assert_eq!(a.sma200.len(), n);
    assert_eq!(a.bollinger.len(), n);
    assert_eq!(a.rsi.len(), n);
    assert_eq!(a.macd.len(), n);
    for r in a.rsi.iter().flatten() {
        assert!(r.is_finite() && (0.0..=100.0).contains(r), "RSI hors bornes : {r}");
    }
    for n in &a.niveaux {
        assert!(n.prix.is_finite());
    }
}

#[test]
fn analyse_toutes_longueurs_et_formes() {
    for n in [0, 1, 2, 5, 19, 20, 21, 30, 49, 50, 199, 200, 201, 260] {
        verifier_analyse(&serie(n, |_| 10.0)); // cours constant : écart type nul
        verifier_analyse(&serie(n, |_| 0.0)); // cours nul
        verifier_analyse(&serie(n, |i| 10.0 + (i % 7) as f64)); // dents de scie
        verifier_analyse(&serie(n, |i| 100.0 * 0.99f64.powi(i as i32))); // baisse continue
        verifier_analyse(&serie(n, |i| 1.0 + i as f64)); // hausse continue
    }
}

#[test]
fn chandeliers_entrees_courtes() {
    for n in [0, 1, 2, 3, 4, 10, 40] {
        let b = serie(n, |i| 10.0 + (i % 3) as f64);
        assert_eq!(heikin_ashi(&b).len(), n);
        let _ = hebdomadaire(&b);
        let _ = motifs(&b);
        let _ = structure(&b, 5);
        let _ = structure(&b, 0);
        let _ = double_sommet_ou_fond(&b);
        let _ = confirmation_multi_periodes(&b);
    }
}

#[test]
fn semaines_regroupees_sans_perte() {
    // 15 jours ouvrés consécutifs à partir d'un lundi : 3 semaines civiles (en comptant les week-ends du calendrier).
    let b = serie(21, |i| 10.0 + i as f64);
    let s = hebdomadaire(&b);
    assert_eq!(s.len(), 3);
    assert_eq!(s[0].ouverture, 10.0);
    assert_eq!(s.last().unwrap().cloture, 30.0);
    assert!(s.iter().all(|w| w.haut >= w.bas));
}

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

#[test]
fn synthese_cas_limites() {
    let auj = d("2026-11-25");
    let vide = synthese(&[], auj);
    assert_eq!(vide.annuel_par_action, Price::from_micro(0));
    assert_eq!(vide.frequence, 0);
    assert_eq!(vide.prochain_estime, None);
    assert_eq!(vide.croissance_annuelle, None);

    // Versement annoncé dans le futur : pas compté dans l'année écoulée, pas pris comme dernier versement.
    let futur = [Versement { date: d("2026-12-15"), montant: Price::from_micro(1_000_000) }];
    let s = synthese(&futur, auj);
    assert_eq!(s.frequence, 0);
    assert_eq!(s.dernier, None);

    // 29 février : l'estimation de l'année suivante tombe le 28 février.
    let bissextile = [Versement { date: d("2028-02-29"), montant: Price::from_micro(500_000) }];
    let s = synthese(&bissextile, d("2028-06-01"));
    assert_eq!(s.prochain_estime, Some(d("2029-02-28")));

    // Montant nul : pas de division par zéro dans la croissance.
    let nuls = [
        Versement { date: d("2023-05-01"), montant: Price::from_micro(0) },
        Versement { date: d("2025-05-01"), montant: Price::from_micro(1_000_000) },
    ];
    let _ = synthese(&nuls, auj);
}

#[test]
fn rendement_cours_nul_ou_negatif() {
    assert_eq!(rendement(Price::from_micro(1_000_000), Price::from_micro(0)), None);
    let _ = rendement(Price::from_micro(1_000_000), Price::from_micro(-5));
}

#[test]
fn dividende_net_bornes() {
    for retenue in [0, 150_000, 350_000, 1_000_000] {
        for brut in [0, 1, 99, 10_000, 1_000_000_000] {
            let n = dividende_net(Money::from_cents(brut), Rate::from_micro(retenue));
            assert_eq!(n.net, n.brut - n.retenue_etrangere);
            assert!((0..=brut).contains(&n.net.cents()), "{retenue} {brut} → {n:?}");
        }
    }
}

#[test]
fn identifiants_mal_formes() {
    for s in ["", "FR", "fr0000121014", "FR000012101X", "FR00001210144", "ÉR000012101", "Aé0000121014", "FR000012101é"]
    {
        assert!(!isin_valide(s), "{s}");
        let _ = pays_isin(s);
    }
    assert_eq!(pays_isin("é"), None);
}

#[test]
fn dates_mal_formees() {
    for s in ["", "2026", "2026-13-01", "2026-02-30", "2025-02-29", "abcd-ef-gh", "2026-1-", "--"] {
        assert!(s.parse::<Date>().is_err(), "{s}");
    }
    assert_eq!(d(" 2024-02-29 ").to_string(), "2024-02-29");
}

#[test]
fn livrets_simulables_pour_toute_date_depuis_le_debut_des_taux() {
    use escarcell_core::livret::{simuler, Mouvement};
    use escarcell_core::params::embarque;
    let debut = embarque::debut();
    for p in [embarque::livret_a(), embarque::ldds(), embarque::lep()] {
        // Un versement le jour du début des taux, puis tous les 1er et 16 du mois jusqu'à fin 2026.
        let mut mvts = vec![Mouvement::depot(debut, Money::from_euros(100))];
        for annee in 2020..=2026 {
            for mois in 1..=12u8 {
                for jour in [1u8, 16] {
                    let date = Date::new(annee, mois, jour).unwrap();
                    if date >= debut {
                        mvts.push(Mouvement::depot(date, Money::from_cents(1)));
                    }
                }
            }
        }
        let annees = simuler(&mvts, &p.taux, 2026).expect("taux manquant");
        assert_eq!(annees.first().unwrap().annee, 2020);
        assert!(annees.iter().all(|a| a.interets.cents() >= 0));
    }
}
