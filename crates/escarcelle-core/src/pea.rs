//! PEA : compartiment espèces, lignes de titres au coût moyen pondéré (CUMP),
//! plus-values latentes et réalisées, fiscalité des retraits.
//!
//! Quantités en millionièmes de part (`QTE_SCALE`), montants en centimes,
//! cours en millionièmes d'euro : aucun flottant.

use std::collections::{BTreeMap, HashMap};
use std::fmt;

use crate::date::Date;
use crate::money::Money;
use crate::price::Price;
use crate::rate::{Rate, SCALE};

/// 1 part = 1 000 000.
pub const QTE_SCALE: i64 = 1_000_000;

/// Convertit un nombre entier de parts.
pub const fn parts(n: i64) -> i64 {
    n * QTE_SCALE
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeaParams {
    pub plafond_versements: Money,
    pub prelevements_sociaux: Rate,
    pub pfu_ir: Rate,
}

impl PeaParams {
    /// Règles en vigueur en 2026.
    pub fn v2026() -> Self {
        Self {
            plafond_versements: Money::from_euros(150_000),
            prelevements_sociaux: Rate::from_micro(186_000),
            pfu_ir: Rate::from_micro(128_000),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Versement(Money),
    /// `valeur_plan` : valeur totale du PEA juste avant le retrait (nécessaire au calcul de la quote-part de gain).
    Retrait { montant: Money, valeur_plan: Money },
    Achat { isin: String, quantite: i64, cours: Price, frais: Money },
    Vente { isin: String, quantite: i64, cours: Price, frais: Money },
    Dividende { isin: String, montant: Money },
    Frais(Money),
    /// Division ou regroupement : `ancien` parts deviennent `nouveau` parts.
    Division { isin: String, ancien: i64, nouveau: i64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mouvement {
    pub date: Date,
    pub op: Operation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PeaError {
    MontantInvalide,
    PlafondVersements { marge: Money },
    EspecesInsuffisantes { manque: Money },
    TitreInconnu(String),
    QuantiteInsuffisante { isin: String, detenue: i64 },
    CoursManquants(Vec<String>),
}

impl fmt::Display for PeaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MontantInvalide => write!(f, "montant ou quantité invalide"),
            Self::PlafondVersements { marge } => write!(f, "plafond de versements atteint, versement maximum : {marge}"),
            Self::EspecesInsuffisantes { manque } => write!(f, "espèces insuffisantes, il manque {manque}"),
            Self::TitreInconnu(i) => write!(f, "titre absent du PEA : {i}"),
            Self::QuantiteInsuffisante { isin, .. } => write!(f, "quantité insuffisante sur {isin}"),
            Self::CoursManquants(v) => write!(f, "cours manquant pour : {}", v.join(", ")),
        }
    }
}
impl std::error::Error for PeaError {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ligne {
    pub quantite: i64,
    /// Coût d'acquisition total, frais d'achat inclus.
    pub cout_total: Money,
}

impl Ligne {
    /// Prix de revient unitaire (CUMP, frais inclus).
    pub fn pru(&self) -> Option<Price> {
        (self.quantite > 0).then(|| {
            Price::from_micro(crate::div_round(
                self.cout_total.cents() as i128 * 10_000 * QTE_SCALE as i128,
                self.quantite as i128,
            ))
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pea {
    pub ouverture: Date,
    pub params: PeaParams,
    pub especes: Money,
    pub lignes: BTreeMap<String, Ligne>,
    /// Cumul brut des versements (base du plafond ; les retraits ne le reconstituent pas).
    pub versements_cumules: Money,
    /// Versements non encore remboursés par les retraits (base fiscale).
    pub versements_restants: Money,
    pub pv_realisees: Money,
    pub dividendes: Money,
    pub frais_payes: Money,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LigneValorisee {
    pub isin: String,
    pub quantite: i64,
    pub pru: Price,
    pub cours: Price,
    pub valeur: Money,
    pub pv_latente: Money,
    pub pv_latente_pct: Rate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Valorisation {
    pub lignes: Vec<LigneValorisee>,
    pub titres: Money,
    pub especes: Money,
    pub total: Money,
    /// Gain fiscal de l'enveloppe : valeur totale − versements restants.
    pub gain_enveloppe: Money,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiscaliteRetrait {
    pub plus_de_5_ans: bool,
    /// Avant 5 ans (hors exceptions légales), tout retrait clôture le plan.
    pub cloture: bool,
    pub montant_brut: Money,
    pub gain_imposable: Money,
    pub impot_revenu: Money,
    pub prelevements_sociaux: Money,
    pub net: Money,
}

fn proportion(m: Money, num: i64, den: i64) -> Money {
    Money::from_cents(crate::div_round(m.cents() as i128 * num as i128, den as i128))
}

impl Pea {
    pub fn new(ouverture: Date, params: PeaParams) -> Self {
        Self {
            ouverture,
            params,
            especes: Money::ZERO,
            lignes: BTreeMap::new(),
            versements_cumules: Money::ZERO,
            versements_restants: Money::ZERO,
            pv_realisees: Money::ZERO,
            dividendes: Money::ZERO,
            frais_payes: Money::ZERO,
        }
    }

    pub fn plus_de_5_ans(&self, d: Date) -> bool {
        d >= self.ouverture.plus_annees(5)
    }

    fn debiter(&mut self, m: Money) -> Result<(), PeaError> {
        if m > self.especes {
            return Err(PeaError::EspecesInsuffisantes { manque: m - self.especes });
        }
        self.especes -= m;
        Ok(())
    }

    /// Applique une opération ; en cas d'erreur, le PEA n'est pas modifié.
    pub fn appliquer(&mut self, m: &Mouvement) -> Result<(), PeaError> {
        let positif = |x: Money| if x > Money::ZERO { Ok(()) } else { Err(PeaError::MontantInvalide) };
        match &m.op {
            Operation::Versement(v) => {
                positif(*v)?;
                let marge = self.params.plafond_versements - self.versements_cumules;
                if *v > marge {
                    return Err(PeaError::PlafondVersements { marge: marge.max(Money::ZERO) });
                }
                self.especes += *v;
                self.versements_cumules += *v;
                self.versements_restants += *v;
            }
            Operation::Retrait { montant, valeur_plan } => {
                positif(*montant)?;
                positif(*valeur_plan)?;
                self.debiter(*montant)?;
                let rembourse = proportion(self.versements_restants, montant.cents(), valeur_plan.cents());
                self.versements_restants = (self.versements_restants - rembourse).max(Money::ZERO);
            }
            Operation::Achat { isin, quantite, cours, frais } => {
                if *quantite <= 0 || frais.is_negative() {
                    return Err(PeaError::MontantInvalide);
                }
                let total = cours.valeur(*quantite) + *frais;
                self.debiter(total)?;
                let l = self.lignes.entry(isin.clone()).or_default();
                l.quantite += quantite;
                l.cout_total += total;
                self.frais_payes += *frais;
            }
            Operation::Vente { isin, quantite, cours, frais } => {
                if *quantite <= 0 || frais.is_negative() {
                    return Err(PeaError::MontantInvalide);
                }
                let l = self.lignes.get(isin).ok_or_else(|| PeaError::TitreInconnu(isin.clone()))?;
                if *quantite > l.quantite {
                    return Err(PeaError::QuantiteInsuffisante { isin: isin.clone(), detenue: l.quantite });
                }
                let cout_sorti = if *quantite == l.quantite { l.cout_total } else { proportion(l.cout_total, *quantite, l.quantite) };
                let produit = cours.valeur(*quantite) - *frais;
                let l = self.lignes.get_mut(isin).expect("ligne vérifiée");
                l.quantite -= quantite;
                l.cout_total -= cout_sorti;
                if l.quantite == 0 {
                    self.lignes.remove(isin);
                }
                self.especes += produit;
                self.pv_realisees += produit - cout_sorti;
                self.frais_payes += *frais;
            }
            Operation::Dividende { montant, .. } => {
                positif(*montant)?;
                self.especes += *montant;
                self.dividendes += *montant;
            }
            Operation::Frais(f) => {
                positif(*f)?;
                self.debiter(*f)?;
                self.frais_payes += *f;
            }
            Operation::Division { isin, ancien, nouveau } => {
                if *ancien <= 0 || *nouveau <= 0 {
                    return Err(PeaError::MontantInvalide);
                }
                let l = self.lignes.get_mut(isin).ok_or_else(|| PeaError::TitreInconnu(isin.clone()))?;
                l.quantite = crate::div_round(l.quantite as i128 * *nouveau as i128, *ancien as i128);
            }
        }
        Ok(())
    }

    /// Valorise le PEA avec les cours fournis (un cours par ISIN détenu).
    pub fn valoriser(&self, cours: &HashMap<String, Price>) -> Result<Valorisation, PeaError> {
        let manquants: Vec<String> = self.lignes.keys().filter(|i| !cours.contains_key(*i)).cloned().collect();
        if !manquants.is_empty() {
            return Err(PeaError::CoursManquants(manquants));
        }
        let lignes: Vec<LigneValorisee> = self
            .lignes
            .iter()
            .map(|(isin, l)| {
                let c = cours[isin];
                let valeur = c.valeur(l.quantite);
                let pv = valeur - l.cout_total;
                let pct = if l.cout_total.cents() > 0 {
                    Rate::from_micro(crate::div_round(pv.cents() as i128 * SCALE as i128, l.cout_total.cents() as i128))
                } else {
                    Rate::ZERO
                };
                LigneValorisee {
                    isin: isin.clone(),
                    quantite: l.quantite,
                    pru: l.pru().unwrap_or_default(),
                    cours: c,
                    valeur,
                    pv_latente: pv,
                    pv_latente_pct: pct,
                }
            })
            .collect();
        let titres: Money = lignes.iter().map(|l| l.valeur).sum();
        let total = titres + self.especes;
        Ok(Valorisation { lignes, titres, especes: self.especes, total, gain_enveloppe: total - self.versements_restants })
    }

    /// Simule l'imposition d'un retrait de `montant` à la date `d`, le plan valant `valeur_plan`.
    pub fn fiscalite_retrait(&self, montant: Money, valeur_plan: Money, d: Date) -> FiscaliteRetrait {
        let plus_de_5_ans = self.plus_de_5_ans(d);
        let cloture = !plus_de_5_ans;
        let brut = if cloture { valeur_plan } else { montant.min(valeur_plan) };
        let gain_plan = valeur_plan - self.versements_restants;
        let gain = if gain_plan <= Money::ZERO || valeur_plan <= Money::ZERO {
            Money::ZERO
        } else if cloture {
            gain_plan
        } else {
            proportion(gain_plan, brut.cents(), valeur_plan.cents())
        };
        let ps = self.params.prelevements_sociaux.apply(gain);
        let ir = if cloture { self.params.pfu_ir.apply(gain) } else { Money::ZERO };
        FiscaliteRetrait {
            plus_de_5_ans,
            cloture,
            montant_brut: brut,
            gain_imposable: gain,
            impot_revenu: ir,
            prelevements_sociaux: ps,
            net: brut - ir - ps,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DCAM: &str = "FR001400U5Q4";

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }
    fn eur(c: i64) -> Money {
        Money::from_cents(c)
    }
    fn px(s: &str) -> Price {
        Price::parse(s).unwrap()
    }
    fn op(pea: &mut Pea, date: &str, o: Operation) {
        pea.appliquer(&Mouvement { date: d(date), op: o }).unwrap();
    }

    /// Scénario de référence : 10 000 € versés, deux achats, une vente partielle.
    fn scenario() -> Pea {
        let mut p = Pea::new(d("2020-01-15"), PeaParams::v2026());
        op(&mut p, "2020-01-15", Operation::Versement(Money::from_euros(10_000)));
        op(&mut p, "2020-01-16", Operation::Achat { isin: DCAM.into(), quantite: parts(100), cours: px("5"), frais: eur(199) });
        op(&mut p, "2021-01-16", Operation::Achat { isin: DCAM.into(), quantite: parts(100), cours: px("6"), frais: Money::ZERO });
        op(&mut p, "2022-01-16", Operation::Vente { isin: DCAM.into(), quantite: parts(50), cours: px("7"), frais: eur(100) });
        p
    }

    #[test]
    fn pru_cump_frais_inclus() {
        let mut p = Pea::new(d("2020-01-15"), PeaParams::v2026());
        op(&mut p, "2020-01-15", Operation::Versement(Money::from_euros(10_000)));
        op(&mut p, "2020-01-16", Operation::Achat { isin: DCAM.into(), quantite: parts(100), cours: px("5"), frais: eur(199) });
        assert_eq!(p.lignes[DCAM].pru(), Some(px("5.0199")));
        op(&mut p, "2021-01-16", Operation::Achat { isin: DCAM.into(), quantite: parts(100), cours: px("6"), frais: Money::ZERO });
        assert_eq!(p.lignes[DCAM].pru(), Some(px("5.50995")));
        assert_eq!(p.especes, eur(889_801));
    }

    #[test]
    fn vente_partielle_pv_realisee() {
        let p = scenario();
        // Produit 349,00 € ; coût sorti 1 101,99 × 50/200 = 275,50 € ; PV 73,50 €.
        assert_eq!(p.pv_realisees, eur(7_350));
        assert_eq!(p.lignes[DCAM].quantite, parts(150));
        assert_eq!(p.lignes[DCAM].cout_total, eur(82_649));
        assert_eq!(p.especes, eur(924_701));
        assert_eq!(p.frais_payes, eur(299));
    }

    #[test]
    fn valorisation_et_gain_enveloppe() {
        let p = scenario();
        let v = p.valoriser(&HashMap::from([(DCAM.to_string(), px("7"))])).unwrap();
        assert_eq!(v.titres, eur(105_000));
        assert_eq!(v.lignes[0].pv_latente, eur(22_351));
        assert_eq!(v.total, eur(1_029_701));
        assert_eq!(v.gain_enveloppe, eur(29_701));
        assert!(p.valoriser(&HashMap::new()).is_err());
    }

    #[test]
    fn retrait_apres_5_ans_prelevements_sociaux_seuls() {
        let p = scenario();
        let f = p.fiscalite_retrait(Money::from_euros(1_000), eur(1_029_701), d("2026-01-15"));
        assert!(f.plus_de_5_ans && !f.cloture);
        assert_eq!(f.gain_imposable, eur(2_884)); // 1 000 × 297,01 / 10 297,01
        assert_eq!(f.prelevements_sociaux, eur(536)); // 18,6 %
        assert_eq!(f.impot_revenu, Money::ZERO);
        assert_eq!(f.net, eur(99_464));
    }

    #[test]
    fn retrait_avant_5_ans_cloture_et_pfu() {
        let p = scenario();
        let f = p.fiscalite_retrait(Money::from_euros(1_000), eur(1_029_701), d("2024-06-01"));
        assert!(f.cloture);
        assert_eq!(f.montant_brut, eur(1_029_701));
        assert_eq!(f.gain_imposable, eur(29_701));
        assert_eq!(f.impot_revenu, eur(3_802)); // 12,8 %
        assert_eq!(f.prelevements_sociaux, eur(5_524)); // 18,6 %
        assert_eq!(f.net, eur(1_020_375));
    }

    #[test]
    fn retrait_reduit_les_versements_restants() {
        let mut p = scenario();
        op(&mut p, "2026-01-15", Operation::Retrait { montant: Money::from_euros(1_000), valeur_plan: eur(1_029_701) });
        // Remboursé : 1 000 × 10 000 / 10 297,01 = 971,16 €.
        assert_eq!(p.versements_restants, eur(902_884));
        assert_eq!(p.versements_cumules, Money::from_euros(10_000), "le plafond n'est pas reconstitué");
    }

    #[test]
    fn plafond_et_controles() {
        let mut p = Pea::new(d("2020-01-15"), PeaParams::v2026());
        op(&mut p, "2020-01-15", Operation::Versement(Money::from_euros(149_000)));
        let e = p.appliquer(&Mouvement { date: d("2020-02-01"), op: Operation::Versement(Money::from_euros(1_001)) });
        assert_eq!(e, Err(PeaError::PlafondVersements { marge: Money::from_euros(1_000) }));
        let e = p.appliquer(&Mouvement {
            date: d("2020-02-01"),
            op: Operation::Achat { isin: DCAM.into(), quantite: parts(1_000_000), cours: px("1"), frais: Money::ZERO },
        });
        assert!(matches!(e, Err(PeaError::EspecesInsuffisantes { .. })));
        let e = p.appliquer(&Mouvement { date: d("2020-02-01"), op: Operation::Vente { isin: DCAM.into(), quantite: parts(1), cours: px("1"), frais: Money::ZERO } });
        assert!(matches!(e, Err(PeaError::TitreInconnu(_))));
    }

    #[test]
    fn division_divise_le_pru() {
        let mut p = scenario();
        op(&mut p, "2023-01-01", Operation::Division { isin: DCAM.into(), ancien: 1, nouveau: 2 });
        assert_eq!(p.lignes[DCAM].quantite, parts(300));
        assert_eq!(p.lignes[DCAM].cout_total, eur(82_649));
    }

    #[test]
    fn vente_totale_supprime_la_ligne() {
        let mut p = scenario();
        op(&mut p, "2023-01-01", Operation::Vente { isin: DCAM.into(), quantite: parts(150), cours: px("8"), frais: Money::ZERO });
        assert!(p.lignes.is_empty());
        // PV cumulées : 73,50 + (1 200 − 826,49) = 447,01 €.
        assert_eq!(p.pv_realisees, eur(44_701));
    }
}
