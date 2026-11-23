//! Récupération des cours. Première source : Yahoo Finance (réponses JSON publiques, sans clé).
//! Les cours restent sur la machine de l'utilisateur et ne sont jamais rediffusés.
//! Boursorama et Euronext viendront s'ajouter devant dans la chaîne de `escarcelle-quotes`.

use serde::Deserialize;

pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Escarcelle/0.1";

pub struct Cotation {
    /// Cours en millionièmes de la devise.
    pub cours_micro: i64,
    pub devise: String,
    /// Heure du cours, secondes Unix.
    pub horodatage: i64,
}

#[derive(Deserialize)]
struct Recherche {
    #[serde(default)]
    quotes: Vec<Resultat>,
}

#[derive(Deserialize)]
struct Resultat {
    symbol: String,
    #[serde(default, rename = "quoteType")]
    quote_type: Option<String>,
    #[serde(default, rename = "exchDisp")]
    place: Option<String>,
    #[serde(default)]
    shortname: Option<String>,
    #[serde(default)]
    longname: Option<String>,
    #[serde(default)]
    exchange: Option<String>,
}

#[derive(Deserialize)]
struct Chart {
    chart: ChartCorps,
}

#[derive(Deserialize)]
struct ChartCorps {
    result: Option<Vec<ChartResultat>>,
}

#[derive(Deserialize)]
struct ChartResultat {
    meta: Meta,
    #[serde(default)]
    timestamp: Vec<i64>,
    #[serde(default)]
    indicators: Option<Indicateurs>,
}

#[derive(Deserialize)]
struct Indicateurs {
    #[serde(default)]
    quote: Vec<Serie>,
}

#[derive(Deserialize)]
struct Serie {
    #[serde(default)]
    open: Vec<Option<f64>>,
    #[serde(default)]
    high: Vec<Option<f64>>,
    #[serde(default)]
    low: Vec<Option<f64>>,
    #[serde(default)]
    close: Vec<Option<f64>>,
    #[serde(default)]
    volume: Vec<Option<f64>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Meta {
    regular_market_price: Option<f64>,
    currency: Option<String>,
    regular_market_time: Option<i64>,
}

/// Un ISIN : 2 lettres, 9 caractères alphanumériques, 1 chiffre de contrôle.
pub fn est_isin(s: &str) -> bool {
    s.len() == 12
        && s[..2].chars().all(|c| c.is_ascii_uppercase())
        && s[2..].chars().all(|c| c.is_ascii_alphanumeric())
        && s[11..].chars().all(|c| c.is_ascii_digit())
}

/// Trouve le symbole de cotation d'un ISIN ou d'un nom, en privilégiant Euronext Paris.
pub async fn rechercher(http: &reqwest::Client, requete: &str) -> Result<Option<(String, Option<String>)>, String> {
    let r: Recherche = http
        .get("https://query2.finance.yahoo.com/v1/finance/search")
        .query(&[("q", requete), ("quotesCount", "10"), ("newsCount", "0")])
        .send()
        .await
        .map_err(|e| format!("recherche impossible : {e}"))?
        .error_for_status()
        .map_err(|e| format!("recherche refusée : {e}"))?
        .json()
        .await
        .map_err(|e| format!("réponse de recherche illisible : {e}"))?;
    let paris = |q: &&Resultat| q.symbol.ends_with(".PA") || q.exchange.as_deref() == Some("PAR");
    let choix = r.quotes.iter().find(paris).or_else(|| r.quotes.first());
    Ok(choix.map(|q| (q.symbol.clone(), q.longname.clone().or_else(|| q.shortname.clone()))))
}

/// Dernier cours connu d'un symbole.
pub async fn coter(http: &reqwest::Client, symbole: &str) -> Result<Cotation, String> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbole}");
    let c: Chart = http
        .get(url)
        .query(&[("interval", "1d"), ("range", "1d")])
        .send()
        .await
        .map_err(|e| format!("{symbole} : source injoignable ({e})"))?
        .error_for_status()
        .map_err(|e| format!("{symbole} : cours refusé ({e})"))?
        .json()
        .await
        .map_err(|e| format!("{symbole} : réponse illisible ({e})"))?;
    let meta = c
        .chart
        .result
        .and_then(|mut r| r.pop())
        .map(|r| r.meta)
        .ok_or_else(|| format!("{symbole} : titre introuvable"))?;
    let prix = meta.regular_market_price.ok_or_else(|| format!("{symbole} : pas de cours"))?;
    Ok(Cotation {
        cours_micro: (prix * 1_000_000.0).round() as i64,
        devise: meta.currency.unwrap_or_else(|| "EUR".into()),
        horodatage: meta.regular_market_time.unwrap_or(0),
    })
}

/// Places de cotation en euros (suffixes Yahoo) : seules ces cotations sont gérées en V1.
const PLACES_EURO: [(&str, &str); 12] = [
    (".PA", "Paris"),
    (".AS", "Amsterdam"),
    (".BR", "Bruxelles"),
    (".LS", "Lisbonne"),
    (".MI", "Milan"),
    (".MC", "Madrid"),
    (".DE", "Xetra"),
    (".F", "Francfort"),
    (".HE", "Helsinki"),
    (".VI", "Vienne"),
    (".IR", "Dublin"),
    (".AT", "Athènes"),
];

/// Pays de l'Espace économique européen (préfixe ISIN = pays de l'émetteur).
const EEE: [&str; 30] = [
    "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "GR", "HR", "HU", "IE",
    "IS", "IT", "LI", "LT", "LU", "LV", "MT", "NL", "NO", "PL", "PT", "RO", "SE", "SI", "SK",
];

pub fn place_euro(symbole: &str) -> Option<&'static str> {
    PLACES_EURO.iter().find(|(suf, _)| symbole.ends_with(suf)).map(|(_, nom)| *nom)
}

/// Éligibilité PEA, quand on peut l'estimer : un ISIN d'un pays de l'EEE pour une action.
/// Pour un ETF, l'éligibilité dépend de sa composition : elle reste à vérifier.
pub fn eligibilite_pea(isin: Option<&str>, symbole: &str, type_: &str) -> &'static str {
    if type_ == "ETF" {
        return "à vérifier (ETF)";
    }
    match isin {
        Some(i) if EEE.contains(&&i[..2]) => "probable",
        Some(_) => "non (émetteur hors EEE)",
        None if [".PA", ".AS", ".BR", ".LS", ".MI", ".MC", ".HE", ".VI", ".IR", ".AT"].iter().any(|s| symbole.ends_with(s)) => "probable",
        None => "à vérifier",
    }
}

pub struct Titre {
    pub symbole: String,
    pub nom: String,
    pub place: String,
    pub type_: String,
}

/// Recherche d'actions et d'ETF cotés en euros sur les places européennes.
pub async fn rechercher_titres(http: &reqwest::Client, requete: &str) -> Result<Vec<Titre>, String> {
    let r: Recherche = http
        .get("https://query2.finance.yahoo.com/v1/finance/search")
        .query(&[("q", requete), ("quotesCount", "20"), ("newsCount", "0"), ("lang", "fr-FR"), ("region", "FR")])
        .send()
        .await
        .map_err(|e| format!("recherche impossible : {e}"))?
        .error_for_status()
        .map_err(|e| format!("recherche refusée : {e}"))?
        .json()
        .await
        .map_err(|e| format!("réponse de recherche illisible : {e}"))?;
    let mut titres: Vec<Titre> = r
        .quotes
        .into_iter()
        .filter(|q| matches!(q.quote_type.as_deref(), Some("EQUITY") | Some("ETF")))
        .filter_map(|q| {
            let place = place_euro(&q.symbol)?;
            Some(Titre {
                nom: q.longname.or(q.shortname).unwrap_or_else(|| q.symbol.clone()),
                place: q.place.unwrap_or_else(|| place.into()),
                type_: if q.quote_type.as_deref() == Some("ETF") { "ETF".into() } else { "Action".into() },
                symbole: q.symbol,
            })
        })
        .collect();
    // Paris en premier.
    titres.sort_by_key(|t| !t.symbole.ends_with(".PA"));
    Ok(titres)
}

/// Historique quotidien (`periode` : 1y, 2y, 5y…).
pub async fn historique(http: &reqwest::Client, symbole: &str, periode: &str) -> Result<Vec<escarcelle_core::analyse::Bougie>, String> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbole}");
    let c: Chart = http
        .get(url)
        .query(&[("interval", "1d"), ("range", periode)])
        .send()
        .await
        .map_err(|e| format!("{symbole} : historique injoignable ({e})"))?
        .error_for_status()
        .map_err(|e| format!("{symbole} : historique refusé ({e})"))?
        .json()
        .await
        .map_err(|e| format!("{symbole} : historique illisible ({e})"))?;
    let r = c.chart.result.and_then(|mut r| r.pop()).ok_or_else(|| format!("{symbole} : pas d'historique"))?;
    let q = r.indicators.and_then(|mut i| i.quote.pop()).ok_or_else(|| format!("{symbole} : pas d'historique"))?;
    let val = |v: &Vec<Option<f64>>, i: usize| v.get(i).copied().flatten();
    Ok(r.timestamp
        .iter()
        .enumerate()
        .filter_map(|(i, &t)| {
            let cloture = val(&q.close, i)?;
            Some(escarcelle_core::analyse::Bougie {
                t,
                ouverture: val(&q.open, i).unwrap_or(cloture),
                haut: val(&q.high, i).unwrap_or(cloture),
                bas: val(&q.low, i).unwrap_or(cloture),
                cloture,
                volume: val(&q.volume, i).unwrap_or(0.0),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnaissance_isin() {
        assert!(est_isin("FR001400U5Q4"));
        assert!(est_isin("IE0002XZSHO1"));
        assert!(!est_isin("DCAM.PA"));
        assert!(!est_isin("fr001400u5q4"));
    }

    #[test]
    fn places_et_eligibilite() {
        assert_eq!(place_euro("MC.PA"), Some("Paris"));
        assert_eq!(place_euro("AAPL"), None);
        assert_eq!(eligibilite_pea(Some("FR0000121014"), "MC.PA", "Action"), "probable");
        assert_eq!(eligibilite_pea(Some("US0378331005"), "APC.DE", "Action"), "non (émetteur hors EEE)");
        assert_eq!(eligibilite_pea(None, "SAP.DE", "Action"), "à vérifier");
    }
}
