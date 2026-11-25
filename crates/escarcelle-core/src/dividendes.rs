//! Dividendes des lignes du PEA.
//!
//! - Retenue à la source prélevée par le pays de la société (déduit des deux premières lettres de l'ISIN).
//! - **PEA** : pas d'impôt français tant que l'argent reste dans le plan ; la retenue étrangère n'est
//!   imputable sur rien, elle est donc perdue.
//!   Source : BOI-RPPM-RCM-40-50-30 (les crédits d'impôt conventionnels ne sont restitués que si les
//!   revenus sont eux-mêmes imposables, ce qui n'est pas le cas des revenus de titres inscrits sur un PEA).
//!
//! Montants en centimes, taux en millionièmes, dividende par action en millionièmes d'euro.

use crate::{Date, Money, Price, Rate};

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
        // Taux standard de 5 % depuis le 1er janvier 2020 (Clearstream, fiche marché Grèce).
        "GR" => 50_000,
        _ => return None,
    };
    Some(Rate::from_micro(micro))
}

/// Liste des pays connus, pour l'écran de réglage.
pub const PAYS_CONNUS: [(&str, &str); 17] = [
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
    ("GR", "Grèce"),
];

/// Pays d'une société d'après son ISIN.
pub fn pays_isin(isin: &str) -> Option<&str> {
    let p = isin.get(..2)?;
    p.chars().all(|c| c.is_ascii_uppercase()).then_some(p)
}

/// Décomposition d'un dividende brut encaissé dans le PEA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DividendeNet {
    pub brut: Money,
    /// Impôt prélevé par le pays de la société, perdu dans un PEA.
    pub retenue_etrangere: Money,
    /// Ce qui arrive sur le compte espèces du PEA : brut − retenue. Aucun impôt français.
    pub net: Money,
}

/// Dividende net encaissé dans le PEA pour un montant brut et la retenue prélevée par le pays.
pub fn dividende_net(brut: Money, retenue: Rate) -> DividendeNet {
    let retenue_etrangere = retenue.apply(brut);
    DividendeNet { brut, retenue_etrangere, net: brut - retenue_etrangere }
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
    let recents: Vec<&Versement> =
        historique.iter().filter(|v| v.date > il_y_a_un_an && v.date <= aujourd_hui).collect();
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
    (cours.micro() > 0)
        .then(|| Rate::from_micro(crate::div_round(annuel.micro() as i128 * 1_000_000, cours.micro() as i128)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(pays: &str) -> Money {
        dividende_net(Money::from_euros(100), retenue_par_defaut(pays).unwrap()).net
    }

    #[test]
    fn pea_retenue_perdue() {
        assert_eq!(net("FR"), Money::from_euros(100));
        assert_eq!(net("DE"), Money::from_cents(73_62));
        assert_eq!(net("NL"), Money::from_euros(85));
    }

    #[test]
    fn tous_les_pays_proposes_ont_une_retenue() {
        for (code, nom) in PAYS_CONNUS {
            assert!(retenue_par_defaut(code).is_some(), "retenue manquante pour {nom}");
        }
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
        let h = [
            v(2022, 5, 10, 2_000_000),
            v(2023, 5, 10, 2_100_000),
            v(2024, 5, 10, 2_205_000),
            v(2025, 5, 10, 2_315_250),
        ];
        let s = synthese(&h, Date::ymd(2026, 3, 1));
        assert_eq!(s.croissance_annuelle, Some(Rate::from_micro(50_000)));
        assert_eq!(s.frequence, 1);
        assert_eq!(s.prochain_estime, Some(Date::ymd(2026, 5, 10)));
    }
}
