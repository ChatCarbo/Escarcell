//! Récupération des cours. Première source : Yahoo Finance (réponses JSON publiques, sans clé).
//! Les cours restent sur la machine de l'utilisateur et ne sont jamais rediffusés.
//! Boursorama et Euronext viendront s'ajouter devant dans la chaîne de `escarcelle-quotes`.

use serde::Deserialize;

pub const USER_AGENT: &str = concat!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Escarcelle/", env!("CARGO_PKG_VERSION"));

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
    #[serde(default)]
    events: Option<Evenements>,
}

#[derive(Deserialize)]
struct Evenements {
    #[serde(default)]
    dividends: std::collections::HashMap<String, DividendeBrut>,
}

#[derive(Deserialize)]
struct DividendeBrut {
    amount: f64,
    date: i64,
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

/// Devise normalisée et facteur à appliquer aux prix : la Bourse de Londres cote en pence
/// (« GBp » ou « GBX ») ; on convertit en livres.
pub fn normaliser_devise(devise: Option<&str>) -> (String, f64) {
    match devise {
        Some("GBp") | Some("GBX") | Some("GBx") => ("GBP".into(), 0.01),
        Some("ZAc") | Some("ZAC") => ("ZAR".into(), 0.01),
        Some("ILA") => ("ILS".into(), 0.01),
        Some(d) => (d.to_uppercase(), 1.0),
        None => ("EUR".into(), 1.0),
    }
}

/// Symbole Yahoo du taux de change : nombre d'unités de `devise` pour 1 euro.
pub fn symbole_change(devise: &str) -> String {
    format!("EUR{devise}=X")
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
    let (devise, facteur) = normaliser_devise(meta.currency.as_deref());
    // Un taux de change (EURUSD=X) est déjà exprimé en unités de devise : pas de conversion.
    let facteur = if symbole.ends_with("=X") { 1.0 } else { facteur };
    Ok(Cotation {
        cours_micro: (prix * facteur * 1_000_000.0).round() as i64,
        devise,
        horodatage: meta.regular_market_time.unwrap_or(0),
    })
}

/// Places hors zone euro acceptées dans le CTO (suffixes Yahoo).
const PLACES_MONDE: [(&str, &str, &str); 9] = [
    (".SW", "Zurich", "CHF"),
    (".L", "Londres", "GBP"),
    (".ST", "Stockholm", "SEK"),
    (".CO", "Copenhague", "DKK"),
    (".OL", "Oslo", "NOK"),
    (".TO", "Toronto", "CAD"),
    (".T", "Tokyo", "JPY"),
    (".HK", "Hong Kong", "HKD"),
    (".AX", "Sydney", "AUD"),
];

/// Codes Yahoo des places américaines (les symboles américains n'ont pas de suffixe).
const PLACES_US: [&str; 9] = ["NMS", "NYQ", "NGM", "NCM", "ASE", "PCX", "BTS", "NAS", "NYS"];

/// Places de cotation en euros (suffixes Yahoo).
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
    /// Devise de cotation probable, d'après la place.
    pub devise: String,
}

/// Place et devise d'un résultat de recherche. `monde` : accepter aussi les places hors euro (CTO).
fn place_et_devise(symbole: &str, exchange: Option<&str>, monde: bool) -> Option<(&'static str, &'static str)> {
    if let Some(p) = place_euro(symbole) {
        return Some((p, "EUR"));
    }
    if !monde {
        return None;
    }
    if let Some((_, p, d)) = PLACES_MONDE.iter().find(|(suf, _, _)| symbole.ends_with(suf)) {
        return Some((p, d));
    }
    if !symbole.contains('.') && exchange.is_some_and(|e| PLACES_US.contains(&e)) {
        return Some(("États-Unis", "USD"));
    }
    None
}

/// Recherche d'actions et d'ETF. PEA : cotations en euros ; CTO (`monde`) : aussi les États-Unis,
/// la Suisse, Londres, les pays nordiques, Toronto, Tokyo, Hong Kong et Sydney.
pub async fn rechercher_titres(http: &reqwest::Client, requete: &str, monde: bool) -> Result<Vec<Titre>, String> {
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
            let (place, devise) = place_et_devise(&q.symbol, q.exchange.as_deref(), monde)?;
            Some(Titre {
                nom: q.longname.or(q.shortname).unwrap_or_else(|| q.symbol.clone()),
                place: q.place.unwrap_or_else(|| place.into()),
                type_: if q.quote_type.as_deref() == Some("ETF") { "ETF".into() } else { "Action".into() },
                symbole: q.symbol,
                devise: devise.into(),
            })
        })
        .collect();
    // Paris en premier, puis la cotation principale américaine, puis le reste.
    titres.sort_by_key(|t| if t.symbole.ends_with(".PA") { 0 } else if t.devise == "USD" { 1 } else { 2 });
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
    let (_, f) = normaliser_devise(r.meta.currency.as_deref());
    let q = r.indicators.and_then(|mut i| i.quote.pop()).ok_or_else(|| format!("{symbole} : pas d'historique"))?;
    let val = |v: &Vec<Option<f64>>, i: usize| v.get(i).copied().flatten().map(|x| x * f);
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
                volume: q.volume.get(i).copied().flatten().unwrap_or(0.0),
            })
        })
        .collect())
}

/// Dividendes et cours d'il y a un an.
pub struct InfosDividendes {
    /// (date de détachement ISO, montant par action en millionièmes de la devise de cotation)
    pub versements: Vec<(String, i64)>,
    pub cours_un_an_micro: Option<i64>,
}

fn date_iso(t: i64) -> String {
    escarcelle_core::Date::depuis_jours_unix(t.div_euclid(86_400)).to_string()
}

/// Historique des dividendes sur 10 ans et cours mensuels (une seule requête, légère).
pub async fn dividendes(http: &reqwest::Client, symbole: &str) -> Result<InfosDividendes, String> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbole}");
    let c: Chart = http
        .get(url)
        .query(&[("interval", "1mo"), ("range", "10y"), ("events", "div")])
        .send()
        .await
        .map_err(|e| format!("{symbole} : dividendes injoignables ({e})"))?
        .error_for_status()
        .map_err(|e| format!("{symbole} : dividendes refusés ({e})"))?
        .json()
        .await
        .map_err(|e| format!("{symbole} : dividendes illisibles ({e})"))?;
    let r = c.chart.result.and_then(|mut r| r.pop()).ok_or_else(|| format!("{symbole} : titre introuvable"))?;
    let (_, f) = normaliser_devise(r.meta.currency.as_deref());
    let mut versements: Vec<(String, i64)> = r
        .events
        .map(|e| e.dividends.into_values().map(|d| (date_iso(d.date), (d.amount * f * 1_000_000.0).round() as i64)).collect())
        .unwrap_or_default();
    versements.sort();
    let maintenant = r.meta.regular_market_time.or_else(|| r.timestamp.last().copied()).unwrap_or(0);
    let cible = maintenant - 365 * 86_400;
    let cours_un_an_micro = r.indicators.and_then(|mut i| i.quote.pop()).and_then(|q| {
        let i = r.timestamp.iter().rposition(|&t| t <= cible)?;
        q.close.get(i).copied().flatten().map(|c| (c * f * 1_000_000.0).round() as i64)
    });
    Ok(InfosDividendes { versements, cours_un_an_micro })
}

/// Pays probable d'une société à partir de sa place de cotation, faute d'ISIN.
/// Approximation : une société néerlandaise peut être cotée à Paris (Stellantis, Airbus…),
/// une société américaine à Francfort (NVD.F).
pub fn pays_par_place(symbole: &str) -> Option<&'static str> {
    const PAYS: [(&str, &str); 21] = [
        (".PA", "FR"), (".AS", "NL"), (".BR", "BE"), (".LS", "PT"), (".MI", "IT"), (".MC", "ES"),
        (".DE", "DE"), (".F", "DE"), (".HE", "FI"), (".VI", "AT"), (".IR", "IE"), (".AT", "GR"),
        (".SW", "CH"), (".L", "GB"), (".ST", "SE"), (".CO", "DK"), (".OL", "NO"), (".TO", "CA"),
        (".T", "JP"), (".HK", "HK"), (".AX", "AU"),
    ];
    match PAYS.iter().find(|(suf, _)| symbole.ends_with(suf)) {
        Some((_, p)) => Some(p),
        None if !symbole.contains('.') => Some("US"),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devises_et_places() {
        assert_eq!(normaliser_devise(Some("GBp")), ("GBP".to_string(), 0.01));
        assert_eq!(normaliser_devise(Some("USD")), ("USD".to_string(), 1.0));
        assert_eq!(symbole_change("USD"), "EURUSD=X");
        assert_eq!(place_et_devise("NVDA", Some("NMS"), true), Some(("États-Unis", "USD")));
        assert_eq!(place_et_devise("NVDA", Some("NMS"), false), None);
        assert_eq!(place_et_devise("NESN.SW", None, true), Some(("Zurich", "CHF")));
        assert_eq!(place_et_devise("MC.PA", None, false), Some(("Paris", "EUR")));
        assert_eq!(pays_par_place("NVDA"), Some("US"));
        assert_eq!(pays_par_place("NESN.SW"), Some("CH"));
    }

    #[test]
    fn dates_iso() {
        assert_eq!(date_iso(0), "1970-01-01");
        assert_eq!(date_iso(1_767_571_200), "2026-01-05");
        assert_eq!(date_iso(1_709_164_800), "2024-02-29");
    }

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
