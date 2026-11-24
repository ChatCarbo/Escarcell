//! Dividendes et fiscalité des enveloppes PEA et CTO.
//!
//! - Retenue à la source prélevée par le pays de la société (déduit des deux premières lettres de l'ISIN).
//! - **PEA** : pas d'impôt français tant que l'argent reste dans le plan ; la retenue étrangère n'est
//!   imputable sur rien, elle est donc perdue.
//! - **CTO** : prélèvement forfaitaire unique (12,8 % d'impôt + 18,6 % de prélèvements sociaux).
//!   La retenue étrangère ouvre droit à un crédit d'impôt, limité à la part d'impôt français (12,8 %)
//!   et au taux prévu par la convention fiscale (15 % en général) : en pratique `min(retenue, 12,8 %)`.
//!   L'excédent de retenue n'est récupérable qu'en le réclamant au fisc étranger ; il est compté comme perdu.
//!
//! Montants en centimes, taux en millionièmes, dividende par action en millionièmes de la devise de cotation.

use crate::{Date, Money, Price, Rate};

/// Enveloppe fiscale d'une ligne d'actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Enveloppe {
    Pea,
    Cto,
}

impl Enveloppe {
    pub fn code(self) -> &'static str {
        match self {
            Enveloppe::Pea => "PEA",
            Enveloppe::Cto => "CTO",
        }
    }
    pub fn depuis_code(s: &str) -> Option<Self> {
        match s {
            "PEA" => Some(Enveloppe::Pea),
            "CTO" => Some(Enveloppe::Cto),
            _ => None,
        }
    }
}

/// Paramètres fiscaux des revenus mobiliers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fiscalite {
    /// Part impôt sur le revenu du prélèvement forfaitaire unique.
    pub pfu_ir: Rate,
    pub prelevements_sociaux: Rate,
}

impl Fiscalite {
    /// Taux 2026 : 12,8 % + 18,6 % = 31,4 %.
    pub fn v2026() -> Self {
        Self { pfu_ir: Rate::from_micro(128_000), prelevements_sociaux: Rate::from_micro(186_000) }
    }
    pub fn pfu(&self) -> Rate {
        Rate::from_micro(self.pfu_ir.micro() + self.prelevements_sociaux.micro())
    }
}

/// Retenue à la source par défaut d'un pays (code ISO à 2 lettres), sur les dividendes versés à un
/// résident français par un établissement qui n'applique pas le taux réduit de la convention.
/// Valeurs indicatives (1er janvier 2025), modifiables par l'utilisateur.
pub fn retenue_par_defaut(pays: &str) -> Option<Rate> {
    let micro = match pays {
        "FR" => 0,
        "DE" => 263_750,
        "AT" => 275_000,
        "BE" => 300_000,
        "DK" => 270_000,
        "ES" => 190_000,
        "FI" => 200_000,
        "IE" => 250_000,
        "IT" => 260_000,
        "LU" => 150_000,
        "NL" => 150_000,
        "NO" => 250_000,
        "PL" => 190_000,
        "PT" => 250_000,
        "CZ" => 150_000,
        "SE" => 300_000,
        // Hors PEA, utiles pour le CTO.
        "US" => 150_000, // avec formulaire W-8BEN
        "CH" => 350_000,
        "GB" => 0,
        "CA" => 250_000,
        "JP" => 153_150,
        "HK" => 0,
        "AU" => 300_000, // dividendes non « franked »
        _ => return None,
    };
    Some(Rate::from_micro(micro))
}

/// Liste des pays connus, pour l'écran de réglage.
pub const PAYS_CONNUS: [(&str, &str); 23] = [
    ("FR", "France"),
    ("DE", "Allemagne"),
    ("AT", "Autriche"),
    ("BE", "Belgique"),
    ("DK", "Danemark"),
    ("ES", "Espagne"),
    ("FI", "Finlande"),
    ("IE", "Irlande"),
    ("IT", "Italie"),
    ("LU", "Luxembourg"),
    ("NL", "Pays-Bas"),
    ("NO", "Norvège"),
    ("PL", "Pologne"),
    ("PT", "Portugal"),
    ("CZ", "Tchéquie"),
    ("SE", "Suède"),
    ("US", "États-Unis"),
    ("CH", "Suisse"),
    ("GB", "Royaume-Uni"),
    ("CA", "Canada"),
    ("JP", "Japon"),
    ("HK", "Hong Kong"),
    ("AU", "Australie"),
];

/// Pays d'une société d'après son ISIN.
pub fn pays_isin(isin: &str) -> Option<&str> {
    let p = isin.get(..2)?;
    p.chars().all(|c| c.is_ascii_uppercase()).then_some(p)
}

/// Décomposition d'un dividende brut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DividendeNet {
    pub brut: Money,
    pub retenue_etrangere: Money,
    /// Crédit d'impôt imputé sur l'impôt français (CTO seulement).
    pub credit_impot: Money,
    pub impot_francais: Money,
    pub prelevements_sociaux: Money,
    pub net: Money,
}

/// Dividende net encaissé pour un montant brut, selon l'enveloppe et la retenue du pays.
pub fn dividende_net(brut: Money, retenue: Rate, env: Enveloppe, f: &Fiscalite) -> DividendeNet {
    let retenue_etrangere = retenue.apply(brut);
    match env {
        Enveloppe::Pea => DividendeNet {
            brut,
            retenue_etrangere,
            credit_impot: Money::ZERO,
            impot_francais: Money::ZERO,
            prelevements_sociaux: Money::ZERO,
            net: brut - retenue_etrangere,
        },
        Enveloppe::Cto => {
            let taux_credit = Rate::from_micro(retenue.micro().min(f.pfu_ir.micro()));
            let credit_impot = taux_credit.apply(brut);
            let impot_francais = f.pfu_ir.apply(brut) - credit_impot;
            let prelevements_sociaux = f.prelevements_sociaux.apply(brut);
            DividendeNet {
                brut,
                retenue_etrangere,
                credit_impot,
                impot_francais,
                prelevements_sociaux,
                net: brut - retenue_etrangere - impot_francais - prelevements_sociaux,
            }
        }
    }
}

/// Impôt dû si la plus-value latente était réalisée aujourd'hui.
/// PEA : 0 (l'impôt n'intervient qu'au retrait, selon l'âge du plan). CTO : PFU sur la plus-value positive.
pub fn impot_plus_value(plus_value: Money, env: Enveloppe, f: &Fiscalite) -> Money {
    match env {
        Enveloppe::Pea => Money::ZERO,
        Enveloppe::Cto if plus_value.cents() > 0 => f.pfu().apply(plus_value),
        Enveloppe::Cto => Money::ZERO,
    }
}

/// Un versement de dividende par action (date de détachement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Versement {
    pub date: Date,
    pub montant: Price,
}

/// Synthèse des dividendes d'un titre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntheseDividendes {
    /// Somme des dividendes par action détachés sur les 12 derniers mois.
    pub annuel_par_action: Price,
    /// Nombre de versements sur les 12 derniers mois (1 = annuel, 2 = semestriel, 4 = trimestriel).
    pub frequence: usize,
    /// Date de détachement suivante, estimée en décalant d'un an chaque détachement des 12 derniers mois.
    pub prochain_estime: Option<Date>,
    pub dernier: Option<Versement>,
    /// Croissance annuelle moyenne du dividende sur les années complètes disponibles (au plus 5).
    pub croissance_annuelle: Option<Rate>,
}

/// Résume l'historique des dividendes (dates de détachement) à la date `aujourd_hui`.
pub fn synthese(historique: &[Versement], aujourd_hui: Date) -> SyntheseDividendes {
    let il_y_a_un_an = aujourd_hui.plus_annees(-1);
    let recents: Vec<&Versement> = historique.iter().filter(|v| v.date > il_y_a_un_an && v.date <= aujourd_hui).collect();
    let annuel = recents.iter().map(|v| v.montant.micro()).sum::<i64>();
    let prochain_estime = recents.iter().map(|v| v.date.plus_annees(1)).filter(|d| *d > aujourd_hui).min();
    let dernier = historique.iter().filter(|v| v.date <= aujourd_hui).max_by_key(|v| v.date).copied();

    // Croissance : totaux par année civile complète, de la plus ancienne à la plus récente.
    let annee_courante = aujourd_hui.year();
    let mut totaux: Vec<(i32, i64)> = Vec::new();
    for a in (annee_courante - 6)..annee_courante {
        let t: i64 = historique.iter().filter(|v| v.date.year() == a).map(|v| v.montant.micro()).sum();
        if t > 0 {
            totaux.push((a, t));
        }
    }
    let totaux = if totaux.len() > 6 { totaux[totaux.len() - 6..].to_vec() } else { totaux };
    let croissance_annuelle = match (totaux.first(), totaux.last()) {
        (Some(&(a0, t0)), Some(&(a1, t1))) if a1 > a0 => {
            let n = (a1 - a0) as f64;
            let g = (t1 as f64 / t0 as f64).powf(1.0 / n) - 1.0;
            Some(Rate::from_micro((g * 1_000_000.0).round() as i64))
        }
        _ => None,
    };

    SyntheseDividendes {
        annuel_par_action: Price::from_micro(annuel),
        frequence: recents.len(),
        prochain_estime,
        dernier,
        croissance_annuelle,
    }
}

/// Rendement = dividende annuel / cours.
pub fn rendement(annuel: Price, cours: Price) -> Option<Rate> {
    (cours.micro() > 0).then(|| Rate::from_micro(crate::div_round(annuel.micro() as i128 * 1_000_000, cours.micro() as i128)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eur(e: i64) -> Money {
        Money::from_euros(e)
    }

    #[test]
    fn pea_france_sans_retenue() {
        let d = dividende_net(eur(100), retenue_par_defaut("FR").unwrap(), Enveloppe::Pea, &Fiscalite::v2026());
        assert_eq!(d.net, eur(100));
    }

    #[test]
    fn pea_allemagne_retenue_perdue() {
        let d = dividende_net(eur(100), retenue_par_defaut("DE").unwrap(), Enveloppe::Pea, &Fiscalite::v2026());
        assert_eq!(d.retenue_etrangere, Money::from_cents(26_38));
        assert_eq!(d.net, Money::from_cents(73_62));
    }

    #[test]
    fn cto_france_flat_tax() {
        let d = dividende_net(eur(100), Rate::ZERO, Enveloppe::Cto, &Fiscalite::v2026());
        assert_eq!(d.impot_francais, Money::from_cents(12_80));
        assert_eq!(d.prelevements_sociaux, Money::from_cents(18_60));
        assert_eq!(d.net, Money::from_cents(68_60));
    }

    #[test]
    fn cto_allemagne_credit_limite_a_12_8() {
        // 100 € brut : 26,38 retenus, crédit 12,80 = impôt français, PS 18,60 → net 55,02.
        let d = dividende_net(eur(100), retenue_par_defaut("DE").unwrap(), Enveloppe::Cto, &Fiscalite::v2026());
        assert_eq!(d.credit_impot, Money::from_cents(12_80));
        assert_eq!(d.impot_francais, Money::ZERO);
        assert_eq!(d.net, Money::from_cents(55_02));
    }

    #[test]
    fn cto_pays_bas_credit_partiel() {
        // Retenue 15 % : crédit 12,80, impôt français 0, PS 18,60 → net 66,40.
        let d = dividende_net(eur(100), retenue_par_defaut("NL").unwrap(), Enveloppe::Cto, &Fiscalite::v2026());
        assert_eq!(d.net, Money::from_cents(66_40));
    }

    #[test]
    fn plus_value_cto() {
        let f = Fiscalite::v2026();
        assert_eq!(impot_plus_value(eur(1000), Enveloppe::Cto, &f), eur(314));
        assert_eq!(impot_plus_value(eur(-500), Enveloppe::Cto, &f), Money::ZERO);
        assert_eq!(impot_plus_value(eur(1000), Enveloppe::Pea, &f), Money::ZERO);
    }

    #[test]
    fn cto_etats_unis_w8ben() {
        // Retenue 15 % : crédit 12,80, PS 18,60 → net 66,40 €.
        let d = dividende_net(eur(100), retenue_par_defaut("US").unwrap(), Enveloppe::Cto, &Fiscalite::v2026());
        assert_eq!(d.net, Money::from_cents(66_40));
    }

    #[test]
    fn pays_depuis_isin() {
        assert_eq!(pays_isin("FR0000120271"), Some("FR"));
        assert_eq!(pays_isin("NL0000235190"), Some("NL"));
        assert_eq!(pays_isin("x"), None);
    }

    fn v(a: i32, m: u8, j: u8, euros_micro: i64) -> Versement {
        Versement { date: Date::ymd(a, m, j), montant: Price::from_micro(euros_micro) }
    }

    #[test]
    fn synthese_dividende_trimestriel() {
        let h = [
            v(2025, 1, 2, 790_000),
            v(2025, 3, 20, 790_000),
            v(2025, 6, 20, 850_000),
            v(2025, 9, 19, 850_000),
            v(2025, 12, 19, 850_000),
            v(2026, 3, 20, 850_000),
            v(2026, 6, 19, 850_000),
            v(2026, 9, 18, 850_000),
        ];
        let s = synthese(&h, Date::ymd(2026, 11, 24));
        assert_eq!(s.frequence, 4);
        assert_eq!(s.annuel_par_action, Price::from_micro(3_400_000));
        assert_eq!(s.prochain_estime, Some(Date::ymd(2026, 12, 19)));
        assert_eq!(s.dernier.unwrap().date, Date::ymd(2026, 9, 18));
        assert_eq!(rendement(s.annuel_par_action, Price::from_micro(77_490_000)), Some(Rate::from_micro(43_877)));
    }

    #[test]
    fn croissance_du_dividende() {
        let h = [v(2022, 5, 10, 2_000_000), v(2023, 5, 10, 2_100_000), v(2024, 5, 10, 2_205_000), v(2025, 5, 10, 2_315_250)];
        let s = synthese(&h, Date::ymd(2026, 3, 1));
        assert_eq!(s.croissance_annuelle, Some(Rate::from_micro(50_000)));
        assert_eq!(s.frequence, 1);
        assert_eq!(s.prochain_estime, Some(Date::ymd(2026, 5, 10)));
    }
}
