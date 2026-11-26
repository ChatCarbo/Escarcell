//! Journal des achats et ventes du PEA : quantité détenue et prix de revient unitaire (PRU) de chaque titre,
//! recalculés à partir de l'historique complet.
//!
//! Règles :
//! - PRU = coût total des titres détenus / quantité détenue, frais d'achat compris (moyenne pondérée) ;
//! - une vente ne change pas le PRU des titres restants : le coût diminue en proportion des titres vendus ;
//! - quand la quantité revient à zéro, le coût repart de zéro ;
//! - une vente supérieure à la quantité détenue à sa date est refusée (elle est ignorée et signalée) ;
//! - le même jour, les achats passent avant les ventes (un aller-retour dans la journée est possible) ;
//!   à date et sens égaux, l'ordre du journal est conservé.
//!
//! Le coût est suivi en 10⁻¹² € (quantité en millionièmes × cours en millionièmes) : aucun arrondi ne
//! s'accumule, seul le PRU affiché est arrondi au millionième d'euro.

use std::collections::BTreeMap;

use crate::{Date, Money, Price};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensTitre {
    Achat,
    Vente,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MouvementTitre {
    pub date: Date,
    pub isin: String,
    pub sens: SensTitre,
    /// Quantité en millionièmes de titre (strictement positive).
    pub quantite_micro: i64,
    pub prix: Price,
    /// Courtage : ajouté au coût à l'achat, sans effet sur le PRU à la vente.
    pub frais: Money,
}

/// Ligne détenue après rejeu du journal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LigneJournal {
    pub quantite_micro: i64,
    /// Coût des titres détenus, en 10⁻¹² €.
    cout: i128,
}

impl LigneJournal {
    /// Prix de revient unitaire, frais d'achat compris (0 si la ligne est soldée).
    pub fn pru(&self) -> Price {
        if self.quantite_micro <= 0 {
            return Price::from_micro(0);
        }
        Price::from_micro(crate::div_round(self.cout, self.quantite_micro as i128))
    }

    /// Coût total des titres détenus, arrondi au centime.
    pub fn cout(&self) -> Money {
        Money::from_cents(crate::div_round(self.cout, 10_000_000_000))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErreurJournal {
    /// Vente supérieure à la quantité détenue à sa date (quantité détenue, en millionièmes).
    VenteSuperieure { detenue_micro: i64 },
    /// Quantité nulle ou négative, cours ou frais négatifs, ou quantité cumulée hors limites.
    Invalide,
}

/// Résultat du rejeu : lignes par ISIN (y compris soldées, quantité 0) et mouvements refusés
/// (indice dans le journal fourni).
#[derive(Clone, Debug, Default)]
pub struct Rejeu {
    pub lignes: BTreeMap<String, LigneJournal>,
    pub erreurs: Vec<(usize, ErreurJournal)>,
}

/// Ordre de rejeu : date, achats avant ventes, puis ordre du journal.
pub fn ordre(mvts: &[MouvementTitre]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..mvts.len()).collect();
    idx.sort_by_key(|&i| (mvts[i].date, mvts[i].sens == SensTitre::Vente, i));
    idx
}

/// Rejoue le journal et calcule les lignes détenues. Les mouvements refusés n'ont aucun effet.
pub fn rejouer(mvts: &[MouvementTitre]) -> Rejeu {
    let mut r = Rejeu::default();
    for i in ordre(mvts) {
        let m = &mvts[i];
        if m.quantite_micro <= 0 || m.prix.micro() < 0 || m.frais.cents() < 0 {
            r.erreurs.push((i, ErreurJournal::Invalide));
            continue;
        }
        let l = r.lignes.entry(m.isin.clone()).or_default();
        match m.sens {
            SensTitre::Achat => {
                let Some(q) = l.quantite_micro.checked_add(m.quantite_micro) else {
                    r.erreurs.push((i, ErreurJournal::Invalide));
                    continue;
                };
                l.quantite_micro = q;
                l.cout = l.cout.saturating_add(
                    m.quantite_micro as i128 * m.prix.micro() as i128 + m.frais.cents() as i128 * 10_000_000_000,
                );
            }
            SensTitre::Vente => {
                if m.quantite_micro > l.quantite_micro {
                    r.erreurs.push((i, ErreurJournal::VenteSuperieure { detenue_micro: l.quantite_micro }));
                    continue;
                }
                let reste = l.quantite_micro - m.quantite_micro;
                // Coût restant = coût × reste / quantité : le PRU des titres gardés ne change pas.
                // Calcul en deux temps (quotient puis reste) : pas de dépassement même sur des montants extrêmes.
                let (q, r_) = (l.quantite_micro as i128, reste as i128);
                l.cout = if reste == 0 { 0 } else { l.cout / q * r_ + l.cout % q * r_ / q };
                l.quantite_micro = reste;
            }
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }

    fn mvt(date: &str, sens: SensTitre, q: i64, prix_micro: i64, frais_cents: i64) -> MouvementTitre {
        MouvementTitre {
            date: d(date),
            isin: "FR0000120271".into(),
            sens,
            quantite_micro: q * 1_000_000,
            prix: Price::from_micro(prix_micro),
            frais: Money::from_cents(frais_cents),
        }
    }

    #[test]
    fn pru_moyenne_ponderee_frais_compris() {
        // 10 × 50 € + 2 € de frais, puis 10 × 60 € + 2 € : coût 1 104 €, 20 titres, PRU 55,20 €.
        let r = rejouer(&[
            mvt("2024-01-10", SensTitre::Achat, 10, 50_000_000, 200),
            mvt("2024-06-10", SensTitre::Achat, 10, 60_000_000, 200),
        ]);
        let l = r.lignes["FR0000120271"];
        assert!(r.erreurs.is_empty());
        assert_eq!(l.quantite_micro, 20_000_000);
        assert_eq!(l.pru(), Price::from_micro(55_200_000));
        assert_eq!(l.cout(), Money::from_cents(110_400));
    }

    #[test]
    fn vente_sans_effet_sur_le_pru() {
        let r = rejouer(&[
            mvt("2024-01-10", SensTitre::Achat, 3, 10_000_000, 100),
            mvt("2024-02-10", SensTitre::Vente, 1, 99_000_000, 500),
        ]);
        let l = r.lignes["FR0000120271"];
        assert_eq!(l.quantite_micro, 2_000_000);
        // PRU 31 € / 3 = 10,333333… → 10,333333 €.
        assert_eq!(l.pru(), Price::from_micro(10_333_333));
    }

    #[test]
    fn vente_superieure_refusee() {
        let r = rejouer(&[
            mvt("2024-01-10", SensTitre::Achat, 5, 10_000_000, 0),
            mvt("2024-01-09", SensTitre::Vente, 1, 10_000_000, 0),
            mvt("2024-02-10", SensTitre::Vente, 6, 10_000_000, 0),
        ]);
        assert_eq!(
            r.erreurs,
            vec![
                (1, ErreurJournal::VenteSuperieure { detenue_micro: 0 }),
                (2, ErreurJournal::VenteSuperieure { detenue_micro: 5_000_000 })
            ]
        );
        assert_eq!(r.lignes["FR0000120271"].quantite_micro, 5_000_000);
    }

    #[test]
    fn achat_avant_vente_le_meme_jour() {
        let r = rejouer(&[
            mvt("2024-01-10", SensTitre::Vente, 2, 12_000_000, 0),
            mvt("2024-01-10", SensTitre::Achat, 2, 10_000_000, 0),
        ]);
        assert!(r.erreurs.is_empty());
        assert_eq!(r.lignes["FR0000120271"].quantite_micro, 0);
        assert_eq!(r.lignes["FR0000120271"].pru(), Price::from_micro(0));
    }

    #[test]
    fn ligne_soldee_repart_de_zero() {
        let r = rejouer(&[
            mvt("2024-01-10", SensTitre::Achat, 1, 100_000_000, 1_000),
            mvt("2024-02-10", SensTitre::Vente, 1, 120_000_000, 0),
            mvt("2024-03-10", SensTitre::Achat, 4, 20_000_000, 0),
        ]);
        assert_eq!(r.lignes["FR0000120271"].pru(), Price::from_micro(20_000_000));
    }

    #[test]
    fn quantites_fractionnaires() {
        // 0,5 titre à 101,123456 € : PRU exact au millionième.
        let r = rejouer(&[MouvementTitre {
            quantite_micro: 500_000,
            ..mvt("2024-01-10", SensTitre::Achat, 0, 101_123_456, 0)
        }]);
        assert_eq!(r.lignes["FR0000120271"].pru(), Price::from_micro(101_123_456));
    }

    #[test]
    fn mouvement_invalide() {
        let r = rejouer(&[mvt("2024-01-10", SensTitre::Achat, 0, 1, 0)]);
        assert_eq!(r.erreurs, vec![(0, ErreurJournal::Invalide)]);
    }
}
