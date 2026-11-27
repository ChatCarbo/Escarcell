//! # escarcell-core
//!
//! Règles de calcul d'€scarcell, sans entrée/sortie ni dépendance externe :
//!
//! - [`money::Money`] : montants en centimes (`i64`), jamais de flottants ;
//! - [`price::Price`] : cours et PRU en millionièmes ;
//! - [`rate::Rate`] : taux en millionièmes, calculs entiers exacts ;
//! - [`livret`] : intérêts du Livret A, LDDS et LEP par quinzaines ;
//! - [`conseil_livret`] : meilleures dates de versement et de retrait, répartition entre livrets ;
//! - [`dividendes`] : dividendes et retenues à la source dans le PEA ;
//! - [`calendrier`] : calendrier des dividendes (à venir, à rapprocher, par mois) ;
//! - [`import_csv`] : lecture et contrôle des fichiers CSV d'opérations ;
//! - [`import_positions`] : lecture des exports de portefeuille (Moning, BoursoBank, tableurs) ;
//! - [`journal`] : quantités et PRU du PEA recalculés à partir des achats et ventes ;
//! - [`analyse`], [`chandeliers`] : indicateurs techniques, motifs et structure de tendance ;
//! - [`params`] : séries de paramètres réglementaires datés et jeu embarqué.
//!
//! Les simulations sont fournies à titre informatif et ne remplacent ni un
//! conseiller en investissements financiers ni l'administration fiscale.

pub mod analyse;
pub mod calendrier;
pub mod chandeliers;
pub mod conseil_livret;
pub mod date;
pub mod dividendes;
pub mod identifiants;
pub mod import_csv;
pub mod import_positions;
pub mod journal;
pub mod livret;
pub mod money;
pub mod params;
pub mod price;
pub mod rate;

pub use date::Date;
pub use money::Money;
pub use price::Price;
pub use rate::Rate;

/// Division entière arrondie au plus proche (demi vers l'extérieur).
/// `den` doit être strictement positif.
pub(crate) fn div_round(num: i128, den: i128) -> i64 {
    debug_assert!(den > 0);
    let q = if num >= 0 { num.saturating_add(den / 2) / den } else { num.saturating_sub(den / 2) / den };
    // Jamais de débordement silencieux : un résultat hors de i64 est borné (les saisies sont limitées bien avant).
    q.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

#[cfg(test)]
mod tests_arrondi {
    use super::div_round;

    /// Arrondi au plus proche, la moitié s'éloignant de zéro (0,5 centime → 1 centime).
    #[test]
    fn arrondi_au_plus_proche() {
        assert_eq!(div_round(4, 10), 0);
        assert_eq!(div_round(5, 10), 1);
        assert_eq!(div_round(15, 10), 2);
        assert_eq!(div_round(-4, 10), 0);
        assert_eq!(div_round(-5, 10), -1);
        assert_eq!(div_round(0, 7), 0);
    }

    #[test]
    fn arrondi_sans_debordement() {
        assert_eq!(div_round(i128::MAX, 1), i64::MAX);
        assert_eq!(div_round(i128::MIN, 1), i64::MIN);
    }
}
