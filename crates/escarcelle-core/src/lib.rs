//! # escarcelle-core
//!
//! Règles de calcul d'Escarcelle, sans entrée/sortie ni dépendance externe :
//!
//! - [`money::Money`] : montants en centimes (`i64`), jamais de flottants ;
//! - [`rate::Rate`] : taux en millionièmes, calculs entiers exacts ;
//! - [`livret`] : intérêts du Livret A, LDDS et LEP par quinzaines ;
//! - [`impot`] : impôt sur le revenu, plafonnement du quotient familial, décote, TMI ;
//! - [`params`] : séries de paramètres réglementaires datés et jeu embarqué.
//!
//! Les simulations sont fournies à titre informatif et ne remplacent ni un
//! conseiller en investissements financiers ni l'administration fiscale.

pub mod analyse;
pub mod comparateur;
pub mod conseil_livret;
pub mod courtage;
pub mod date;
pub mod dons;
pub mod impot;
pub mod livret;
pub mod money;
pub mod params;
pub mod pea;
pub mod per;
pub mod price;
pub mod rate;
pub mod strategie;

pub use date::Date;
pub use money::Money;
pub use price::Price;
pub use rate::Rate;

/// Division entière arrondie au plus proche (demi vers l'extérieur).
/// `den` doit être strictement positif.
pub(crate) fn div_round(num: i128, den: i128) -> i64 {
    debug_assert!(den > 0);
    let q = if num >= 0 {
        (num + den / 2) / den
    } else {
        (num - den / 2) / den
    };
    q as i64
}
