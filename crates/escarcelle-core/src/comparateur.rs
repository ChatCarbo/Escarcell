//! Comparateur de frais d'enveloppes assurantielles (PER en V1, assurance-vie en V2).
//!
//! Simulation mensuelle, entièrement entière :
//! - chaque versement (initial puis mensuel) est réparti entre fonds euros et UC selon la cible,
//!   après frais de versement et frais de transaction éventuels sur la part UC ;
//! - UC : rendement brut mensuel = rendement annuel / 12, puis frais de gestion du contrat
//!   et frais des supports (TER) prélevés chaque mois sur l'encours ;
//! - fonds euros : rendement **net de frais publié** par l'assureur (les frais du fonds y sont
//!   déjà inclus), intérêts calculés chaque mois et crédités au 31 décembre ;
//! - pas de rééquilibrage : la répartition cible ne s'applique qu'aux versements.
//!
//! Les bonus commerciaux ne sont jamais intégrés.

use crate::money::Money;
use crate::rate::{Rate, SCALE};

/// Conditions d'un contrat, lues dans le catalogue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contrat {
    pub nom: String,
    pub frais_versement: Rate,
    pub frais_gestion_uc: Rate,
    /// Frais par transaction sur les ETF (ex. 0,15 % chez Linxea Apicil PER).
    pub frais_transaction_uc: Rate,
    /// Rendement net de frais publié du fonds euros (dernière année connue).
    pub rendement_fonds_euros: Rate,
    /// Part maximale autorisée en fonds euros (ex. 50 % pour le PERin Matla en gestion libre).
    pub part_fonds_euros_max: Option<Rate>,
}

/// Hypothèses communes à tous les contrats comparés.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hypotheses {
    pub versement_initial: Money,
    pub versement_mensuel: Money,
    pub annees: u32,
    pub part_fonds_euros: Rate,
    pub rendement_brut_uc: Rate,
    /// Frais annuels moyens des supports en UC (TER des ETF).
    pub ter_uc: Rate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resultat {
    pub nom: String,
    /// Part en fonds euros réellement appliquée (plafonnée si le contrat l'impose).
    pub part_fonds_euros: Rate,
    pub plafonnee: bool,
    pub versements: Money,
    pub capital_final: Money,
    pub frais_contrat: Money,
    pub frais_supports: Money,
    pub frais_versement_transaction: Money,
    pub frais_totaux: Money,
    /// Écart de capital final avec le meilleur contrat (≤ 0).
    pub ecart_avec_meilleur: Money,
}

fn taux_mensuel(m: i128, t: Rate) -> i128 {
    // Montant (en centièmes de centime) × taux / 12.
    m * t.micro() as i128 / (12 * SCALE as i128)
}

/// Simule un contrat. Les calculs internes sont en centièmes de centime pour limiter les arrondis.
pub fn simuler(c: &Contrat, h: &Hypotheses) -> Resultat {
    const K: i128 = 100; // centièmes de centime
    let (part_fe, plafonnee) = match c.part_fonds_euros_max {
        Some(max) if h.part_fonds_euros > max => (max, true),
        _ => (h.part_fonds_euros, false),
    };
    let (mut fe, mut uc, mut interets_fe) = (0i128, 0i128, 0i128);
    let (mut f_contrat, mut f_supports, mut f_entree) = (0i128, 0i128, 0i128);
    let mut verse = 0i128;

    let mut investir = |montant: Money, fe: &mut i128, uc: &mut i128, f_entree: &mut i128| {
        let m = montant.cents() as i128 * K;
        let frais_v = m * c.frais_versement.micro() as i128 / SCALE as i128;
        let net = m - frais_v;
        let vers_fe = net * part_fe.micro() as i128 / SCALE as i128;
        let vers_uc = net - vers_fe;
        let frais_t = vers_uc * c.frais_transaction_uc.micro() as i128 / SCALE as i128;
        *fe += vers_fe;
        *uc += vers_uc - frais_t;
        *f_entree += frais_v + frais_t;
        verse += m;
    };

    investir(h.versement_initial, &mut fe, &mut uc, &mut f_entree);
    for mois in 1..=h.annees * 12 {
        uc += taux_mensuel(uc, h.rendement_brut_uc);
        let fc = taux_mensuel(uc, c.frais_gestion_uc);
        let fs = taux_mensuel(uc, h.ter_uc);
        uc -= fc + fs;
        f_contrat += fc;
        f_supports += fs;
        interets_fe += taux_mensuel(fe, c.rendement_fonds_euros);
        if mois % 12 == 0 {
            fe += interets_fe;
            interets_fe = 0;
        }
        if mois < h.annees * 12 {
            investir(h.versement_mensuel, &mut fe, &mut uc, &mut f_entree);
        }
    }

    let cents = |x: i128| Money::from_cents(crate::div_round(x, K));
    Resultat {
        nom: c.nom.clone(),
        part_fonds_euros: part_fe,
        plafonnee,
        versements: cents(verse),
        capital_final: cents(fe + uc),
        frais_contrat: cents(f_contrat),
        frais_supports: cents(f_supports),
        frais_versement_transaction: cents(f_entree),
        frais_totaux: cents(f_contrat + f_supports + f_entree),
        ecart_avec_meilleur: Money::ZERO,
    }
}

/// Compare plusieurs contrats, triés par capital final décroissant.
/// Un contrat dont la contrainte a modifié la répartition (`plafonnee`) ne simule plus la même
/// allocation : il est classé après les autres et ne sert pas de référence pour les écarts.
pub fn comparer(contrats: &[Contrat], h: &Hypotheses) -> Vec<Resultat> {
    let mut r: Vec<Resultat> = contrats.iter().map(|c| simuler(c, h)).collect();
    r.sort_by(|a, b| {
        a.plafonnee
            .cmp(&b.plafonnee)
            .then_with(|| b.capital_final.cmp(&a.capital_final))
            .then_with(|| a.nom.cmp(&b.nom))
    });
    if let Some(meilleur) = r.first().map(|x| x.capital_final) {
        for x in &mut r {
            x.ecart_avec_meilleur = x.capital_final - meilleur;
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pct(s: &str) -> Rate {
        Rate::parse_percent(s).unwrap()
    }
    fn eur(e: i64) -> Money {
        Money::from_euros(e)
    }
    fn contrat(nom: &str, uc: &str, fe: &str) -> Contrat {
        Contrat {
            nom: nom.into(),
            frais_versement: Rate::ZERO,
            frais_gestion_uc: pct(uc),
            frais_transaction_uc: Rate::ZERO,
            rendement_fonds_euros: pct(fe),
            part_fonds_euros_max: None,
        }
    }
    fn hyp(part_fe: &str) -> Hypotheses {
        Hypotheses {
            versement_initial: eur(10_000),
            versement_mensuel: Money::ZERO,
            annees: 1,
            part_fonds_euros: pct(part_fe),
            rendement_brut_uc: Rate::ZERO,
            ter_uc: Rate::ZERO,
        }
    }

    #[test]
    fn fonds_euros_seul_un_an() {
        // 10 000 € à 3 % net : 25 € d'intérêts par mois, crédités au 31/12.
        let r = simuler(&contrat("A", "0,5", "3"), &hyp("100"));
        assert_eq!(r.capital_final, eur(10_300));
        assert_eq!(r.frais_totaux, Money::ZERO);
    }

    #[test]
    fn uc_seules_frais_mensuels() {
        // Rendement nul, 0,60 %/an prélevés chaque mois sur l'encours : 10 000 × (1 − 0,0005)^12.
        let r = simuler(&contrat("A", "0,6", "0"), &hyp("0"));
        assert_eq!(r.capital_final, Money::from_cents(994_017));
        assert_eq!(r.frais_contrat, Money::from_cents(5_984)); // 59,835 € arrondis
    }

    #[test]
    fn plafond_fonds_euros_matla() {
        let mut c = contrat("PERin Matla", "0,5", "3,5");
        c.part_fonds_euros_max = Some(pct("50"));
        let r = simuler(&c, &hyp("80"));
        assert!(r.plafonnee);
        assert_eq!(r.part_fonds_euros, pct("50"));
    }

    #[test]
    fn frais_de_transaction_sur_etf() {
        let mut c = contrat("Linxea Apicil PER", "0,6", "1,45");
        c.frais_transaction_uc = pct("0,15");
        let r = simuler(&c, &hyp("0"));
        assert_eq!(r.frais_versement_transaction, eur(15));
    }

    #[test]
    fn classement_cinq_per_v1() {
        // Données du catalogue au 23/11/2026, gestion libre, rendements 2025.
        let mut matla = contrat("BoursoBank PERin Matla", "0,5", "3,5");
        matla.part_fonds_euros_max = Some(pct("50"));
        let mut apicil = contrat("Linxea Apicil PER", "0,6", "1,45");
        apicil.frais_transaction_uc = pct("0,15");
        let contrats = [
            contrat("Fortuneo PER", "0,5", "3,2"),
            matla,
            contrat("Linxea Spirit PER", "0,5", "3,08"),
            contrat("Linxea Suravenir PER", "0,6", "3"),
            apicil,
        ];
        let h = Hypotheses {
            versement_initial: eur(5_000),
            versement_mensuel: eur(200),
            annees: 20,
            part_fonds_euros: pct("30"),
            rendement_brut_uc: pct("6"),
            ter_uc: pct("0,2"),
        };
        let r = comparer(&contrats, &h);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0].ecart_avec_meilleur, Money::ZERO);
        assert!(r.windows(2).all(|w| w[0].capital_final >= w[1].capital_final));
        // À 70 % de fonds euros, Matla (plafonné à 50 %) est classé à part, en dernier.
        let r70 = comparer(&contrats, &Hypotheses { part_fonds_euros: pct("70"), ..h });
        assert_eq!(r70.last().unwrap().nom, "BoursoBank PERin Matla");
        assert!(r70.last().unwrap().plafonnee);
        assert_eq!(r70[0].ecart_avec_meilleur, Money::ZERO);
        assert_eq!(r.last().unwrap().nom, "Linxea Apicil PER");
        assert_eq!(r[0].versements, eur(5_000 + 200 * 239));
    }
}
