//! Récupération des cours, sans clé ni compte, auprès de Yahoo Finance (réponses JSON publiques).
//! Seul le symbole du titre est envoyé ; les cours restent sur la machine de l'utilisateur
//! et ne sont jamais rediffusés.

use serde::Deserialize;

pub const USER_AGENT: &str =
    concat!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Escarcelle/", env!("CARGO_PKG_VERSION"));

pub struct Cotation {
    /// Cours en millionièmes de la devise.
    pub cours_micro: i64,
    /// Devise de cotation : un PEA n'accepte que l'euro.
    pub devise: String,
    /// Heure du cours, secondes Unix.
    pub horodatage: i64,
    /// Source qui a fourni le cours.
    pub source: &'static str,
}

pub const YAHOO: &str = "Yahoo Finance";

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

/// Symbole de cotation Yahoo : lettres, chiffres, point, tiret, accent circonflexe ou égal (1 à 20 caractères).
/// Il est placé dans l'adresse de la requête : tout autre caractère est refusé.
pub fn symbole_valide(s: &str) -> bool {
    (1..=20).contains(&s.len())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '^' | '=' | '_'))
}

/// Un ISIN valide (clé de contrôle ISO 6166 comprise).
pub fn est_isin(s: &str) -> bool {
    escarcelle_core::identifiants::isin_valide(s)
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
    let choix = r
        .quotes
        .iter()
        .find(paris)
        .or_else(|| r.quotes.iter().find(|q| place_euro(&q.symbol).is_some()))
        .or_else(|| r.quotes.first());
    Ok(choix.map(|q| (q.symbol.clone(), q.longname.clone().or_else(|| q.shortname.clone()))))
}

/// Devise en majuscules (l'euro par défaut si Yahoo ne l'indique pas).
fn devise(d: Option<&str>) -> String {
    d.unwrap_or("EUR").to_uppercase()
}

fn maintenant() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Un cours doit être un nombre fini et strictement positif.
fn cours_valide(x: f64) -> bool {
    x.is_finite() && x > 0.0
}

// ---------- Yahoo Finance ----------

/// Dernier cours connu d'un symbole.
pub async fn coter(http: &reqwest::Client, symbole: &str) -> Result<Cotation, String> {
    if !symbole_valide(symbole) {
        return Err(format!("« {symbole} » n'est pas un symbole de cotation valide"));
    }
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
    let prix =
        meta.regular_market_price.filter(|p| cours_valide(*p)).ok_or_else(|| format!("{symbole} : pas de cours"))?;
    Ok(Cotation {
        cours_micro: (prix * 1_000_000.0).round() as i64,
        devise: devise(meta.currency.as_deref()),
        // Sans heure de cotation, on date le cours de maintenant : un horodatage nul le classerait en 1970,
        // derrière les cours plus anciens.
        horodatage: meta.regular_market_time.filter(|t| *t > 0).unwrap_or_else(maintenant),
        source: YAHOO,
    })
}

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
    "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "GR", "HR", "HU", "IE", "IS", "IT", "LI", "LT",
    "LU", "LV", "MT", "NL", "NO", "PL", "PT", "RO", "SE", "SI", "SK",
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
        Some(i) if i.get(..2).is_some_and(|p| EEE.contains(&p)) => "probable",
        Some(_) => "non (émetteur hors EEE)",
        None if [".PA", ".AS", ".BR", ".LS", ".MI", ".MC", ".HE", ".VI", ".IR", ".AT"]
            .iter()
            .any(|s| symbole.ends_with(s)) =>
        {
            "probable"
        }
        None => "à vérifier",
    }
}

pub struct Titre {
    pub symbole: String,
    pub nom: String,
    pub place: String,
    pub type_: String,
}

/// Recherche d'actions et d'ETF cotés en euros, par nom, mnémonique ou ISIN.
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
                type_: if q.quote_type.as_deref() == Some("ETF") { "ETF" } else { "Action" }.into(),
                symbole: q.symbol,
            })
        })
        .collect();
    // Paris en premier (tri stable : l'ordre de pertinence de Yahoo est conservé pour le reste).
    titres.sort_by_key(|t| !t.symbole.ends_with(".PA"));
    Ok(titres)
}

/// Historique quotidien (`periode` : 1y, 2y, 5y…).
pub async fn historique(
    http: &reqwest::Client,
    symbole: &str,
    periode: &str,
) -> Result<Vec<escarcelle_core::analyse::Bougie>, String> {
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
    let val = |v: &Vec<Option<f64>>, i: usize| v.get(i).copied().flatten().filter(|x| cours_valide(*x));
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
                volume: q.volume.get(i).copied().flatten().filter(|v| v.is_finite() && *v >= 0.0).unwrap_or(0.0),
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
    if !symbole_valide(symbole) {
        return Err(format!("« {symbole} » n'est pas un symbole de cotation valide"));
    }
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
    let mut versements: Vec<(String, i64)> = r
        .events
        .map(|e| {
            e.dividends
                .into_values()
                .filter(|d| cours_valide(d.amount))
                .map(|d| (date_iso(d.date), (d.amount * 1_000_000.0).round() as i64))
                .collect()
        })
        .unwrap_or_default();
    versements.sort();
    let maintenant = r.meta.regular_market_time.filter(|t| *t > 0).unwrap_or_else(maintenant);
    let cible = maintenant - 365 * 86_400;
    // Cours d'il y a un an : clôture de la séance du même jour un an plus tôt (ou de la précédente),
    // tirée de l'historique journalier. Les barres mensuelles ne conviennent pas : la barre d'un mois porte
    // la clôture de la fin du mois, jusqu'à un mois d'écart avec la date voulue.
    let cours_un_an_micro = match cloture_journaliere(http, symbole, cible).await {
        Some(c) => Some(c),
        None => r.indicators.and_then(|mut i| i.quote.pop()).and_then(|q| cloture_avant(&r.timestamp, &q.close, cible)),
    };
    Ok(InfosDividendes { versements, cours_un_an_micro })
}

/// Dernière clôture valide dont l'horodatage est au plus `cible`, en millionièmes.
fn cloture_avant(horodatages: &[i64], clotures: &[Option<f64>], cible: i64) -> Option<i64> {
    horodatages
        .iter()
        .zip(clotures)
        .rev()
        .filter(|(t, _)| **t <= cible)
        .find_map(|(_, c)| c.filter(|c| cours_valide(*c)))
        .map(|c| (c * 1_000_000.0).round() as i64)
}

/// Clôture journalière à la date `cible` (ou à la séance précédente), sur deux ans d'historique.
async fn cloture_journaliere(http: &reqwest::Client, symbole: &str, cible: i64) -> Option<i64> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbole}");
    let c: Chart = http
        .get(url)
        .query(&[("interval", "1d"), ("range", "2y")])
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json()
        .await
        .ok()?;
    let r = c.chart.result.and_then(|mut r| r.pop())?;
    let q = r.indicators.and_then(|mut i| i.quote.pop())?;
    cloture_avant(&r.timestamp, &q.close, cible)
}

/// Pays probable d'une société à partir de sa place de cotation, faute d'ISIN.
/// Approximation : une société néerlandaise peut être cotée à Paris (Stellantis, Airbus…).
pub fn pays_par_place(symbole: &str) -> Option<&'static str> {
    const PAYS: [(&str, &str); 12] = [
        (".PA", "FR"),
        (".AS", "NL"),
        (".BR", "BE"),
        (".LS", "PT"),
        (".MI", "IT"),
        (".MC", "ES"),
        (".DE", "DE"),
        (".F", "DE"),
        (".HE", "FI"),
        (".VI", "AT"),
        (".IR", "IE"),
        (".AT", "GR"),
    ];
    PAYS.iter().find(|(suf, _)| symbole.ends_with(suf)).map(|(_, p)| *p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloture_d_il_y_a_un_an() {
        let jour = 86_400;
        let t = [10 * jour, 11 * jour, 12 * jour, 15 * jour];
        let c = [Some(100.0), Some(101.5), None, Some(103.0)];
        // Séance du jour voulu.
        assert_eq!(cloture_avant(&t, &c, 11 * jour + 3_600), Some(101_500_000));
        // Jour sans cotation valide (ou week-end) : séance précédente.
        assert_eq!(cloture_avant(&t, &c, 13 * jour), Some(101_500_000));
        assert_eq!(cloture_avant(&t, &c, 9 * jour), None);
        assert_eq!(cloture_avant(&[], &[], 9 * jour), None);
    }

    #[test]
    fn symboles() {
        for ok in ["MC.PA", "DCAM.PA", "^FCHI", "EURUSD=X", "BRK-B", "0P0000XYZ.F"] {
            assert!(symbole_valide(ok), "{ok}");
        }
        for ko in ["", "MC PA", "MC.PA/../x", "A?b=c", "#", "é.PA", "ABCDEFGHIJKLMNOPQRSTU"] {
            assert!(!symbole_valide(ko), "{ko}");
        }
    }

    #[test]
    fn places_et_pays() {
        assert_eq!(place_euro("NVDA"), None);
        assert_eq!(place_euro("NESN.SW"), None);
        assert_eq!(pays_par_place("MC.PA"), Some("FR"));
        assert_eq!(pays_par_place("NVDA"), None);
        assert_eq!(pays_par_place("NESN.SW"), None);
        assert_eq!(devise(Some("eur")), "EUR");
        assert_eq!(devise(None), "EUR");
        assert!(!cours_valide(f64::NAN) && !cours_valide(0.0) && !cours_valide(-1.0) && cours_valide(0.01));
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
        assert!(est_isin("FR0000121014"));
        assert!(!est_isin("FR0000121015")); // clé de contrôle fausse
        assert!(!est_isin("DCAM.PA"));
        assert!(!est_isin("fr001400u5q4"));
        // Saisie accentuée de 12 octets : ne doit pas planter.
        assert!(!est_isin("Aé000012101"));
        assert!(!est_isin("VALLOUREC É"));
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
