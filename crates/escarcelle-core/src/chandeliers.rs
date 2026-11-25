//! Lecture des chandeliers et de la structure du marché :
//! Heikin Ashi, passage en hebdomadaire, motifs de chandeliers (doji, marteau, étoile filante, avalement),
//! tendance par plus hauts / plus bas, double sommet / double fond, confirmation multi-périodes.
//!
//! Comme le reste de l'analyse technique, ces lectures décrivent le passé et produisent beaucoup de faux signaux.

use crate::analyse::{Bougie, Signal, Tendance};

/// Bougies Heikin Ashi (lissées) : clôture = moyenne OHLC, ouverture = milieu de la bougie HA précédente.
pub fn heikin_ashi(b: &[Bougie]) -> Vec<Bougie> {
    let mut res: Vec<Bougie> = Vec::with_capacity(b.len());
    for x in b {
        let cloture = (x.ouverture + x.haut + x.bas + x.cloture) / 4.0;
        let ouverture = match res.last() {
            Some(p) => (p.ouverture + p.cloture) / 2.0,
            None => (x.ouverture + x.cloture) / 2.0,
        };
        res.push(Bougie {
            t: x.t,
            ouverture,
            haut: x.haut.max(ouverture).max(cloture),
            bas: x.bas.min(ouverture).min(cloture),
            cloture,
            volume: x.volume,
        });
    }
    res
}

/// Numéro de semaine (lundi à dimanche) d'un horodatage Unix. Le 1er janvier 1970 était un jeudi.
fn semaine(t: i64) -> i64 {
    (t.div_euclid(86_400) + 3).div_euclid(7)
}

/// Regroupe des bougies journalières en bougies hebdomadaires.
pub fn hebdomadaire(b: &[Bougie]) -> Vec<Bougie> {
    let mut res: Vec<Bougie> = Vec::new();
    let mut courante: Option<i64> = None;
    for x in b {
        let s = semaine(x.t);
        match res.last_mut() {
            Some(h) if courante == Some(s) => {
                h.haut = h.haut.max(x.haut);
                h.bas = h.bas.min(x.bas);
                h.cloture = x.cloture;
                h.volume += x.volume;
            }
            _ => {
                res.push(*x);
                courante = Some(s);
            }
        }
    }
    res
}

/// Motif de chandelier reconnu sur la dernière séance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motif {
    Doji,
    Marteau,
    EtoileFilante,
    AvalementHaussier,
    AvalementBaissier,
}

/// Motifs présents sur la dernière bougie, en tenant compte du mouvement des 5 séances précédentes.
pub fn motifs(b: &[Bougie]) -> Vec<Motif> {
    let n = b.len();
    if n < 7 {
        return Vec::new();
    }
    let x = b[n - 1];
    let p = b[n - 2];
    let amplitude = x.haut - x.bas;
    if amplitude <= 0.0 {
        return Vec::new();
    }
    let corps = (x.cloture - x.ouverture).abs();
    let meche_haute = x.haut - x.cloture.max(x.ouverture);
    let meche_basse = x.cloture.min(x.ouverture) - x.bas;
    let baisse_avant = b[n - 2].cloture < b[n - 7].cloture;
    let hausse_avant = b[n - 2].cloture > b[n - 7].cloture;
    let mut m = Vec::new();

    if corps <= 0.1 * amplitude {
        m.push(Motif::Doji);
    } else if meche_basse >= 2.0 * corps && meche_haute <= corps && baisse_avant {
        m.push(Motif::Marteau);
    } else if meche_haute >= 2.0 * corps && meche_basse <= corps && hausse_avant {
        m.push(Motif::EtoileFilante);
    }
    let p_baissiere = p.cloture < p.ouverture;
    let p_haussiere = p.cloture > p.ouverture;
    if p_baissiere && x.cloture > x.ouverture && x.ouverture <= p.cloture && x.cloture >= p.ouverture && baisse_avant {
        m.push(Motif::AvalementHaussier);
    }
    if p_haussiere && x.cloture < x.ouverture && x.ouverture >= p.cloture && x.cloture <= p.ouverture && hausse_avant {
        m.push(Motif::AvalementBaissier);
    }
    m
}

/// Signal lisible associé à un motif.
pub fn signal_motif(m: Motif) -> Signal {
    match m {
        Motif::Doji => Signal {
            nom: "Doji",
            sens: Tendance::Neutre,
            explication: "Ouverture et clôture presque égales : acheteurs et vendeurs s'équilibrent, la tendance hésite.".into(),
        },
        Motif::Marteau => Signal {
            nom: "Marteau",
            sens: Tendance::Haussier,
            explication: "Après une baisse, le cours est descendu puis a été racheté pour finir près du plus haut : possible retournement à la hausse.".into(),
        },
        Motif::EtoileFilante => Signal {
            nom: "Étoile filante",
            sens: Tendance::Baissier,
            explication: "Après une hausse, le cours est monté puis a été vendu pour finir près du plus bas : possible retournement à la baisse.".into(),
        },
        Motif::AvalementHaussier => Signal {
            nom: "Avalement haussier",
            sens: Tendance::Haussier,
            explication: "La bougie verte du jour englobe entièrement la bougie rouge de la veille, après une baisse : les acheteurs reprennent la main.".into(),
        },
        Motif::AvalementBaissier => Signal {
            nom: "Avalement baissier",
            sens: Tendance::Baissier,
            explication: "La bougie rouge du jour englobe entièrement la bougie verte de la veille, après une hausse : les vendeurs reprennent la main.".into(),
        },
    }
}

/// Liste de pivots (indice, prix).
type Pivots = Vec<(usize, f64)>;

/// Sommets et creux locaux, dans l'ordre chronologique.
fn pivots(b: &[Bougie], fenetre: usize) -> (Pivots, Pivots) {
    let (mut hauts, mut bas) = (Vec::new(), Vec::new());
    if b.len() < 2 * fenetre + 1 {
        return (hauts, bas);
    }
    for i in fenetre..b.len() - fenetre {
        let zone = &b[i - fenetre..=i + fenetre];
        if zone.iter().all(|x| x.haut <= b[i].haut) {
            hauts.push((i, b[i].haut));
        }
        if zone.iter().all(|x| x.bas >= b[i].bas) {
            bas.push((i, b[i].bas));
        }
    }
    (hauts, bas)
}

/// Structure de tendance d'après les deux derniers sommets et les deux derniers creux :
/// plus hauts et plus bas croissants = haussière, décroissants = baissière, sinon latérale (range).
pub fn structure(b: &[Bougie], fenetre: usize) -> Tendance {
    let (h, l) = pivots(b, fenetre);
    if h.len() < 2 || l.len() < 2 {
        return Tendance::Neutre;
    }
    let (h1, h2) = (h[h.len() - 2].1, h[h.len() - 1].1);
    let (l1, l2) = (l[l.len() - 2].1, l[l.len() - 1].1);
    if h2 > h1 && l2 > l1 {
        Tendance::Haussier
    } else if h2 < h1 && l2 < l1 {
        Tendance::Baissier
    } else {
        Tendance::Neutre
    }
}

/// Signal décrivant la structure de tendance journalière.
pub fn signal_structure(t: Tendance) -> Signal {
    match t {
        Tendance::Haussier => Signal {
            nom: "Plus hauts et plus bas croissants",
            sens: Tendance::Haussier,
            explication: "Les deux derniers sommets et les deux derniers creux sont de plus en plus hauts : les acheteurs contrôlent le marché.".into(),
        },
        Tendance::Baissier => Signal {
            nom: "Plus hauts et plus bas décroissants",
            sens: Tendance::Baissier,
            explication: "Les deux derniers sommets et les deux derniers creux sont de plus en plus bas : les vendeurs ont l'avantage.".into(),
        },
        Tendance::Neutre => Signal {
            nom: "Marché sans direction (range)",
            sens: Tendance::Neutre,
            explication: "Sommets et creux ne progressent pas dans le même sens : le cours évolue entre un support et une résistance.".into(),
        },
    }
}

/// Double sommet ou double fond sur les 120 dernières séances : deux extrêmes proches (écart ≤ 3 %),
/// séparés d'au moins 10 séances, avec un creux (ou un sommet) intermédiaire marqué (≥ 5 %).
/// La figure est « confirmée » quand le dernier cours franchit ce niveau intermédiaire.
pub fn double_sommet_ou_fond(b: &[Bougie]) -> Option<Signal> {
    let debut = b.len().saturating_sub(120);
    let z = &b[debut..];
    let (h, l) = pivots(z, 5);
    let dernier = z.last()?.cloture;
    let fmt = |x: f64| format!("{x:.2}").replace('.', ",");

    if h.len() >= 2 {
        let ((i1, p1), (i2, p2)) = (h[h.len() - 2], h[h.len() - 1]);
        let creux = z[i1..=i2].iter().map(|x| x.bas).fold(f64::INFINITY, f64::min);
        if i2 - i1 >= 10 && (p1 - p2).abs() / p1.max(p2) <= 0.03 && creux <= p1.min(p2) * 0.95 && dernier < p2 {
            let confirme = dernier < creux;
            return Some(Signal {
                nom: "Double sommet",
                sens: Tendance::Baissier,
                explication: if confirme {
                    format!("Deux sommets vers {} et le cours vient de passer sous le creux intermédiaire ({}) : figure de retournement baissier confirmée.", fmt(p2), fmt(creux))
                } else {
                    format!("Deux sommets vers {} : figure de retournement baissier possible, confirmée seulement si le cours passe sous {}.", fmt(p2), fmt(creux))
                },
            });
        }
    }
    if l.len() >= 2 {
        let ((i1, p1), (i2, p2)) = (l[l.len() - 2], l[l.len() - 1]);
        let sommet = z[i1..=i2].iter().map(|x| x.haut).fold(f64::NEG_INFINITY, f64::max);
        if i2 - i1 >= 10 && (p1 - p2).abs() / p1.min(p2) <= 0.03 && sommet >= p1.max(p2) * 1.05 && dernier > p2 {
            let confirme = dernier > sommet;
            return Some(Signal {
                nom: "Double fond",
                sens: Tendance::Haussier,
                explication: if confirme {
                    format!("Deux creux vers {} et le cours vient de dépasser le sommet intermédiaire ({}) : figure de retournement haussier confirmée.", fmt(p2), fmt(sommet))
                } else {
                    format!("Deux creux vers {} : figure de retournement haussier possible, confirmée seulement si le cours dépasse {}.", fmt(p2), fmt(sommet))
                },
            });
        }
    }
    None
}

/// Compare la tendance journalière et la tendance hebdomadaire (structure sur bougies hebdomadaires,
/// et cours par rapport à la moyenne 40 semaines ≈ 200 jours).
pub fn confirmation_multi_periodes(quotidien: &[Bougie]) -> Option<Signal> {
    let hebdo = hebdomadaire(quotidien);
    if hebdo.len() < 45 {
        return None;
    }
    let jour = structure(quotidien, 5);
    let c: Vec<f64> = hebdo.iter().map(|x| x.cloture).collect();
    let m40 = crate::analyse::sma(&c, 40)[c.len() - 1]?;
    let derniere = c[c.len() - 1];
    let semaine = match structure(&hebdo, 3) {
        Tendance::Haussier if derniere > m40 => Tendance::Haussier,
        Tendance::Baissier if derniere < m40 => Tendance::Baissier,
        _ => Tendance::Neutre,
    };
    match (jour, semaine) {
        (Tendance::Haussier, Tendance::Haussier) => Some(Signal {
            nom: "Tendance confirmée en hebdomadaire",
            sens: Tendance::Haussier,
            explication: "La hausse se voit aussi sur le graphique hebdomadaire : le signal est plus solide qu'en journalier seul.".into(),
        }),
        (Tendance::Baissier, Tendance::Baissier) => Some(Signal {
            nom: "Tendance confirmée en hebdomadaire",
            sens: Tendance::Baissier,
            explication: "La baisse se voit aussi sur le graphique hebdomadaire : le signal est plus solide qu'en journalier seul.".into(),
        }),
        (Tendance::Haussier, Tendance::Baissier) | (Tendance::Baissier, Tendance::Haussier) => Some(Signal {
            nom: "Journalier et hebdomadaire en désaccord",
            sens: Tendance::Neutre,
            explication: "La tendance des derniers jours va à l'inverse de la tendance hebdomadaire : prudence, il peut s'agir d'un simple mouvement passager.".into(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(t: i64, o: f64, h: f64, l: f64, c: f64) -> Bougie {
        Bougie { t, ouverture: o, haut: h, bas: l, cloture: c, volume: 1000.0 }
    }

    fn baisse(n: usize) -> Vec<Bougie> {
        (0..n)
            .map(|i| {
                let c = 100.0 - i as f64;
                b(i as i64 * 86_400, c + 0.5, c + 1.0, c - 1.0, c)
            })
            .collect()
    }

    #[test]
    fn heikin_ashi_lisse() {
        let v = vec![b(0, 10.0, 12.0, 9.0, 11.0), b(86_400, 11.0, 13.0, 10.0, 12.0)];
        let ha = heikin_ashi(&v);
        assert_eq!(ha[0].cloture, 10.5);
        assert_eq!(ha[1].ouverture, (ha[0].ouverture + ha[0].cloture) / 2.0);
        assert_eq!(ha[1].cloture, 11.5);
    }

    #[test]
    fn passage_en_hebdomadaire() {
        // Lundi 5 janvier 2026 = 1 767 571 200 ; deux semaines de 5 séances.
        let lundi = 1_767_571_200;
        let mut v = Vec::new();
        for s in 0..2 {
            for j in 0..5 {
                let t = lundi + (s * 7 + j) * 86_400;
                let c = 100.0 + (s * 5 + j) as f64;
                v.push(b(t, c, c + 2.0, c - 2.0, c + 1.0));
            }
        }
        let h = hebdomadaire(&v);
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].ouverture, 100.0);
        assert_eq!(h[0].cloture, 105.0);
        assert_eq!(h[0].haut, 106.0);
        assert_eq!(h[0].bas, 98.0);
        assert_eq!(h[0].volume, 5000.0);
    }

    #[test]
    fn marteau_apres_une_baisse() {
        let mut v = baisse(10);
        let t = 10 * 86_400;
        v.push(b(t, 90.2, 91.0, 86.0, 90.8)); // longue mèche basse, petit corps en haut
        assert!(motifs(&v).contains(&Motif::Marteau));
    }

    #[test]
    fn doji() {
        let mut v = baisse(10);
        v.push(b(10 * 86_400, 90.0, 91.0, 89.0, 90.05));
        assert_eq!(motifs(&v)[0], Motif::Doji);
    }

    #[test]
    fn avalement_haussier() {
        let mut v = baisse(10);
        v.push(b(10 * 86_400, 90.5, 92.5, 89.5, 92.0)); // englobe la bougie rouge 91,5 → 91,0
        assert!(motifs(&v).contains(&Motif::AvalementHaussier));
    }

    fn sinus(n: usize, tendance: f64) -> Vec<Bougie> {
        (0..n)
            .map(|i| {
                let c = 100.0 + tendance * i as f64 + 5.0 * ((i as f64) * std::f64::consts::PI / 10.0).sin();
                b(i as i64 * 86_400, c, c + 0.5, c - 0.5, c)
            })
            .collect()
    }

    #[test]
    fn structure_des_tendances() {
        assert_eq!(structure(&sinus(80, 0.3), 5), Tendance::Haussier);
        assert_eq!(structure(&sinus(80, -0.3), 5), Tendance::Baissier);
        assert_eq!(structure(&sinus(80, 0.0), 5), Tendance::Neutre);
    }

    #[test]
    fn double_sommet_en_formation() {
        // Montée à 120, repli à 105, remontée à 119, rechute à 112.
        let mut c = Vec::new();
        for i in 0..20 {
            c.push(100.0 + i as f64);
        }
        for i in 0..15 {
            c.push(119.0 - i as f64);
        }
        for i in 0..15 {
            c.push(105.0 + i as f64 * 0.95);
        }
        for i in 0..10 {
            c.push(118.0 - i as f64 * 0.7);
        }
        let v: Vec<Bougie> = c.iter().enumerate().map(|(i, &x)| b(i as i64 * 86_400, x, x + 0.3, x - 0.3, x)).collect();
        let s = double_sommet_ou_fond(&v).expect("double sommet attendu");
        assert_eq!(s.nom, "Double sommet");
        assert!(s.explication.contains("possible"));
    }
}
