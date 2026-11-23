//! # escarcelle-quotes
//!
//! Cours des titres à partir de sources publiques gratuites, sans clé d'API,
//! pour un affichage personnel sur la machine de l'utilisateur (aucune rediffusion).
//!
//! Ordre par défaut des sources :
//! 1. **Boursorama** : source principale ;
//! 2. **Euronext** : source officielle du marché, secours pour tout titre coté sur Euronext ;
//! 3. **Yahoo Finance** : secours pour les titres cotés hors Euronext (ETF sur Xetra, Londres…).
//!
//! Ce crate contient la logique indépendante du réseau : chaîne de sources avec
//! bascule automatique, mise en pause d'une source défaillante, contrôle de
//! cohérence entre deux sources et indicateur de fraîcheur. Les requêtes HTTP et
//! la lecture des pages sont fournies par l'application via le trait [`QuoteProvider`].

use std::collections::HashMap;
use std::fmt;

use escarcelle_core::{Price, Rate};

/// Horodatage en secondes Unix (UTC).
pub type Timestamp = i64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Boursorama,
    Euronext,
    Yahoo,
    Manuel,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::Boursorama => "Boursorama",
            Source::Euronext => "Euronext",
            Source::Yahoo => "Yahoo Finance",
            Source::Manuel => "saisie manuelle",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quote {
    pub isin: String,
    pub cours: Price,
    /// Code ISO 4217, ex. `"EUR"`.
    pub devise: String,
    /// Heure du cours selon la source (pas l'heure de la requête).
    pub horodatage: Timestamp,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuoteError {
    /// Titre inconnu de cette source : passer à la suivante sans pénaliser la source.
    Introuvable,
    /// Réseau, refus du serveur, limite de requêtes.
    Indisponible(String),
    /// La page a changé de format.
    FormatInattendu(String),
}

impl fmt::Display for QuoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Introuvable => write!(f, "titre introuvable"),
            Self::Indisponible(e) => write!(f, "source indisponible : {e}"),
            Self::FormatInattendu(e) => write!(f, "format de page inattendu : {e}"),
        }
    }
}
impl std::error::Error for QuoteError {}

/// Une source de cours. L'implémentation fait la requête HTTP et lit la réponse.
pub trait QuoteProvider {
    fn source(&self) -> Source;
    fn fetch(&self, isin: &str) -> Result<Quote, QuoteError>;
}

/// Réglages de la chaîne de sources.
#[derive(Clone, Copy, Debug)]
pub struct Reglages {
    /// Échecs consécutifs avant mise en pause d'une source.
    pub echecs_avant_pause: u32,
    /// Durée de la pause, en secondes.
    pub duree_pause: i64,
    /// Écart au-delà duquel deux sources sont jugées incohérentes.
    pub ecart_max: Rate,
}

impl Default for Reglages {
    fn default() -> Self {
        Self { echecs_avant_pause: 3, duree_pause: 30 * 60, ecart_max: Rate::from_micro(20_000) }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Sante {
    echecs: u32,
    en_pause_jusqu_a: Option<Timestamp>,
}

/// Résultat d'une recherche de cours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resultat {
    pub quote: Quote,
    /// Sources essayées sans succès avant celle qui a répondu.
    pub echecs: Vec<(Source, QuoteError)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AucuneSource {
    pub echecs: Vec<(Source, QuoteError)>,
    pub en_pause: Vec<Source>,
}

impl fmt::Display for AucuneSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "aucune source n'a fourni de cours")
    }
}
impl std::error::Error for AucuneSource {}

/// Chaîne ordonnée de sources avec bascule automatique.
pub struct ChaineCours {
    sources: Vec<Box<dyn QuoteProvider>>,
    sante: HashMap<Source, Sante>,
    reglages: Reglages,
}

impl ChaineCours {
    pub fn new(sources: Vec<Box<dyn QuoteProvider>>, reglages: Reglages) -> Self {
        Self { sources, sante: HashMap::new(), reglages }
    }

    /// Sources actuellement en pause.
    pub fn en_pause(&self, maintenant: Timestamp) -> Vec<Source> {
        self.sources
            .iter()
            .map(|s| s.source())
            .filter(|s| self.est_en_pause(*s, maintenant))
            .collect()
    }

    fn est_en_pause(&self, s: Source, maintenant: Timestamp) -> bool {
        self.sante.get(&s).and_then(|h| h.en_pause_jusqu_a).is_some_and(|t| maintenant < t)
    }

    /// Essaie les sources dans l'ordre, en sautant celles en pause.
    pub fn cours(&mut self, isin: &str, maintenant: Timestamp) -> Result<Resultat, AucuneSource> {
        let mut echecs = Vec::new();
        let mut en_pause = Vec::new();
        for p in &self.sources {
            let s = p.source();
            if self.est_en_pause(s, maintenant) {
                en_pause.push(s);
                continue;
            }
            match p.fetch(isin) {
                Ok(quote) => {
                    self.sante.insert(s, Sante::default());
                    return Ok(Resultat { quote, echecs });
                }
                Err(QuoteError::Introuvable) => echecs.push((s, QuoteError::Introuvable)),
                Err(e) => {
                    let h = self.sante.entry(s).or_default();
                    h.echecs += 1;
                    if h.echecs >= self.reglages.echecs_avant_pause {
                        h.en_pause_jusqu_a = Some(maintenant + self.reglages.duree_pause);
                        h.echecs = 0;
                    }
                    echecs.push((s, e));
                }
            }
        }
        Err(AucuneSource { echecs, en_pause })
    }

    /// Interroge les deux premières sources disponibles et signale un écart anormal.
    /// À appeler rarement (ex. une fois par jour et par ligne) pour limiter les requêtes.
    pub fn verifier_coherence(&mut self, isin: &str, maintenant: Timestamp) -> Coherence {
        let mut cours = Vec::new();
        for p in &self.sources {
            if cours.len() == 2 {
                break;
            }
            if self.est_en_pause(p.source(), maintenant) {
                continue;
            }
            if let Ok(q) = p.fetch(isin) {
                cours.push(q);
            }
        }
        match cours.as_slice() {
            [a, b] => {
                let ecart = a.cours.ecart_relatif(b.cours);
                if a.devise != b.devise || ecart > self.reglages.ecart_max {
                    Coherence::Divergence { a: a.clone(), b: b.clone(), ecart }
                } else {
                    Coherence::Coherent { ecart }
                }
            }
            _ => Coherence::NonVerifiable,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Coherence {
    Coherent { ecart: Rate },
    /// Mauvais titre associé, cotation sur une autre place ou devise différente : à signaler.
    Divergence { a: Quote, b: Quote, ecart: Rate },
    NonVerifiable,
}

/// Fraîcheur d'un cours, pour la pastille de couleur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fraicheur {
    /// Moins de 20 minutes (vert).
    Recent,
    /// Moins d'une heure (orange).
    Ancien,
    /// Plus d'une heure en séance (gris).
    Perime,
    /// Marché fermé : cours de clôture.
    Cloture,
}

pub fn fraicheur(quote: &Quote, maintenant: Timestamp, marche_ouvert: bool) -> Fraicheur {
    if !marche_ouvert {
        return Fraicheur::Cloture;
    }
    match maintenant - quote.horodatage {
        a if a < 20 * 60 => Fraicheur::Recent,
        a if a < 60 * 60 => Fraicheur::Ancien,
        _ => Fraicheur::Perime,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    struct Faux {
        source: Source,
        reponse: Result<&'static str, QuoteError>,
        appels: Rc<Cell<u32>>,
    }

    impl QuoteProvider for Faux {
        fn source(&self) -> Source {
            self.source
        }
        fn fetch(&self, isin: &str) -> Result<Quote, QuoteError> {
            self.appels.set(self.appels.get() + 1);
            self.reponse.clone().map(|c| Quote {
                isin: isin.into(),
                cours: Price::parse(c).unwrap(),
                devise: "EUR".into(),
                horodatage: 1_000,
                source: self.source,
            })
        }
    }

    fn faux(source: Source, reponse: Result<&'static str, QuoteError>) -> (Box<dyn QuoteProvider>, Rc<Cell<u32>>) {
        let appels = Rc::new(Cell::new(0));
        (Box::new(Faux { source, reponse, appels: appels.clone() }), appels)
    }

    const ISIN: &str = "FR0000000000";

    #[test]
    fn bascule_sur_euronext_si_boursorama_tombe() {
        let (b, _) = faux(Source::Boursorama, Err(QuoteError::FormatInattendu("sélecteur absent".into())));
        let (e, _) = faux(Source::Euronext, Ok("100.50"));
        let mut c = ChaineCours::new(vec![b, e], Reglages::default());
        let r = c.cours(ISIN, 0).unwrap();
        assert_eq!(r.quote.source, Source::Euronext);
        assert_eq!(r.echecs.len(), 1);
    }

    #[test]
    fn source_mise_en_pause_puis_reprise() {
        let (b, appels_b) = faux(Source::Boursorama, Err(QuoteError::Indisponible("503".into())));
        let (e, _) = faux(Source::Euronext, Ok("100"));
        let mut c = ChaineCours::new(vec![b, e], Reglages::default());
        for t in 0..3 {
            c.cours(ISIN, t).unwrap();
        }
        assert_eq!(c.en_pause(10), vec![Source::Boursorama]);
        c.cours(ISIN, 10).unwrap();
        assert_eq!(appels_b.get(), 3, "source en pause non interrogée");
        // Après 30 minutes, la source est réessayée.
        c.cours(ISIN, 3 + 30 * 60).unwrap();
        assert_eq!(appels_b.get(), 4);
    }

    #[test]
    fn titre_introuvable_ne_penalise_pas() {
        let (b, _) = faux(Source::Boursorama, Err(QuoteError::Introuvable));
        let (y, _) = faux(Source::Yahoo, Ok("42"));
        let mut c = ChaineCours::new(vec![b, y], Reglages::default());
        for t in 0..5 {
            assert_eq!(c.cours(ISIN, t).unwrap().quote.source, Source::Yahoo);
        }
        assert!(c.en_pause(5).is_empty());
    }

    #[test]
    fn toutes_les_sources_en_echec() {
        let (b, _) = faux(Source::Boursorama, Err(QuoteError::Indisponible("timeout".into())));
        let (e, _) = faux(Source::Euronext, Err(QuoteError::Introuvable));
        let mut c = ChaineCours::new(vec![b, e], Reglages::default());
        let err = c.cours(ISIN, 0).unwrap_err();
        assert_eq!(err.echecs.len(), 2);
    }

    #[test]
    fn coherence_entre_sources() {
        let (b, _) = faux(Source::Boursorama, Ok("100"));
        let (e, _) = faux(Source::Euronext, Ok("101"));
        let mut c = ChaineCours::new(vec![b, e], Reglages::default());
        assert!(matches!(c.verifier_coherence(ISIN, 0), Coherence::Coherent { .. }));

        let (b, _) = faux(Source::Boursorama, Ok("100"));
        let (e, _) = faux(Source::Euronext, Ok("110"));
        let mut c = ChaineCours::new(vec![b, e], Reglages::default());
        assert!(matches!(c.verifier_coherence(ISIN, 0), Coherence::Divergence { .. }));
    }

    #[test]
    fn pastille_de_fraicheur() {
        let q = Quote { isin: ISIN.into(), cours: Price::parse("1").unwrap(), devise: "EUR".into(), horodatage: 0, source: Source::Euronext };
        assert_eq!(fraicheur(&q, 10 * 60, true), Fraicheur::Recent);
        assert_eq!(fraicheur(&q, 30 * 60, true), Fraicheur::Ancien);
        assert_eq!(fraicheur(&q, 2 * 3600, true), Fraicheur::Perime);
        assert_eq!(fraicheur(&q, 2 * 3600, false), Fraicheur::Cloture);
    }
}
