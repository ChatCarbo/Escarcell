//! Frais de courtage par ordre, à partir des grilles du catalogue.

use crate::money::Money;
use crate::rate::Rate;

/// Un palier : s'applique aux ordres dont le montant est ≤ `jusqu_a` (aucune borne si `None`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palier {
    pub jusqu_a: Option<Money>,
    pub fixe: Money,
    pub pourcentage: Rate,
    pub minimum: Money,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grille {
    /// Paliers triés par borne croissante, le dernier sans borne.
    pub paliers: Vec<Palier>,
    /// Premier ordre du mois gratuit s'il est ≤ ce montant (tarif Starter de Fortuneo).
    pub premier_ordre_mensuel_gratuit_jusqu_a: Option<Money>,
}

/// Contexte d'un ordre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Ordre {
    pub montant: Money,
    pub premier_du_mois: bool,
    /// Ordre gratuit au titre d'une offre (ex. ETF partenaire) : renseigné par l'appelant.
    pub offre_gratuite: bool,
}

impl Grille {
    /// Frais de l'ordre, plafonnés si `plafond_legal` est fourni (PEA : 0,5 % en ligne).
    pub fn frais(&self, o: Ordre, plafond_legal: Option<Rate>) -> Money {
        if o.offre_gratuite {
            return Money::ZERO;
        }
        if o.premier_du_mois && self.premier_ordre_mensuel_gratuit_jusqu_a.is_some_and(|s| o.montant <= s) {
            return Money::ZERO;
        }
        let Some(p) = self.paliers.iter().find(|p| p.jusqu_a.map_or(true, |b| o.montant <= b)) else {
            return Money::ZERO;
        };
        let brut = (p.fixe + p.pourcentage.apply(o.montant)).max(p.minimum);
        match plafond_legal {
            Some(t) => brut.min(t.apply(o.montant)),
            None => brut,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eur(c: i64) -> Money {
        Money::from_cents(c)
    }
    fn pct(s: &str) -> Rate {
        Rate::parse_percent(s).unwrap()
    }
    fn palier(jusqu_a: Option<i64>, fixe: i64, p: &str) -> Palier {
        Palier { jusqu_a: jusqu_a.map(Money::from_euros), fixe: eur(fixe), pourcentage: pct(p), minimum: Money::ZERO }
    }

    /// Fortuneo, tarifs au 6 août 2026.
    fn starter() -> Grille {
        Grille { paliers: vec![palier(None, 0, "0,35")], premier_ordre_mensuel_gratuit_jusqu_a: Some(Money::from_euros(500)) }
    }
    fn progress() -> Grille {
        Grille { paliers: vec![palier(Some(3_000), 490, "0"), palier(None, 0, "0,15")], premier_ordre_mensuel_gratuit_jusqu_a: None }
    }
    fn trader_pro() -> Grille {
        Grille { paliers: vec![palier(Some(10_000), 950, "0"), palier(None, 0, "0,10")], premier_ordre_mensuel_gratuit_jusqu_a: None }
    }
    fn ordre(e: i64, premier: bool) -> Ordre {
        Ordre { montant: Money::from_euros(e), premier_du_mois: premier, offre_gratuite: false }
    }

    #[test]
    fn exemples_de_la_brochure_fortuneo() {
        // Simulation MIF 2 de la brochure : 500 € en Starter → 0 € (1er ordre) ou 1,75 € ; 4 000 € en Progress → 6 €.
        assert_eq!(starter().frais(ordre(500, true), None), Money::ZERO);
        assert_eq!(starter().frais(ordre(500, false), None), eur(175));
        assert_eq!(progress().frais(ordre(4_000, false), None), eur(600));
    }

    #[test]
    fn paliers() {
        assert_eq!(starter().frais(ordre(501, true), None), eur(175)); // 1,7535 → 1,75
        assert_eq!(progress().frais(ordre(3_000, false), None), eur(490));
        assert_eq!(trader_pro().frais(ordre(10_000, false), None), eur(950));
        assert_eq!(trader_pro().frais(ordre(20_000, false), None), eur(2_000));
    }

    #[test]
    fn plafond_legal_pea() {
        // Progress, 500 € : 4,90 € plafonnés à 0,5 % = 2,50 €.
        assert_eq!(progress().frais(ordre(500, false), Some(pct("0,5"))), eur(250));
        assert_eq!(starter().frais(ordre(500, false), Some(pct("0,5"))), eur(175));
    }

    #[test]
    fn grilles_boursobank() {
        // Tarifs au 18/11/2026.
        let decouverte = Grille { paliers: vec![palier(Some(500), 199, "0"), palier(None, 0, "0,60")], premier_ordre_mensuel_gratuit_jusqu_a: None };
        let trader = Grille { paliers: vec![palier(Some(7_500), 1_665, "0"), palier(None, 0, "0,22")], premier_ordre_mensuel_gratuit_jusqu_a: None };
        assert_eq!(decouverte.frais(ordre(500, false), None), eur(199));
        assert_eq!(decouverte.frais(ordre(1_000, false), None), eur(600));
        assert_eq!(trader.frais(ordre(10_000, false), None), eur(2_200));
        // Trader, 1 000 € en PEA : 16,65 € plafonnés à 0,50 % = 5,00 €.
        assert_eq!(trader.frais(ordre(1_000, false), Some(pct("0,5"))), eur(500));
    }

    #[test]
    fn offre_gratuite() {
        let o = Ordre { offre_gratuite: true, ..ordre(2_000, false) };
        assert_eq!(trader_pro().frais(o, None), Money::ZERO);
    }
}
