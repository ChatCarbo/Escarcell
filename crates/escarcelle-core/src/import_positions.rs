//! Import des lignes du PEA depuis un export de portefeuille (Moning, BoursoBank, Fortuneo, tableur…) :
//! un fichier « photo » qui donne, pour chaque titre, la quantité détenue et le prix de revient unitaire.
//!
//! Tout est automatique : la ligne d'en-tête est cherchée dans les premières lignes (les exports de banque
//! commencent souvent par un titre ou une date), les colonnes sont reconnues par leur intitulé en français ou
//! en anglais, et le fichier est classé soit comme photo de portefeuille, soit comme historique
//! d'opérations (format de [`crate::import_csv`]).

use crate::identifiants::isin_valide;
use crate::import_csv::{
    champ_d_entete, lire_nombre, proposer_correspondance, verifier_correspondance, Decimale, Enregistrement,
    PRIX_MAX_MICRO, QUANTITE_MAX_MICRO,
};

// ---------- Tableaux HTML (exports « Excel » de certaines banques) ----------

/// Décode les entités HTML courantes.
fn entites(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut reste = s;
    while let Some(i) = reste.find('&') {
        res.push_str(&reste[..i]);
        let apres = &reste[i + 1..];
        let fin = apres.find(';').filter(|&f| f <= 10);
        let decode = fin.and_then(|f| {
            let nom = &apres[..f];
            let c = match nom {
                "nbsp" => '\u{A0}',
                "amp" => '&',
                "lt" => '<',
                "gt" => '>',
                "quot" => '"',
                "apos" => '\'',
                "euro" => '€',
                "eacute" => 'é',
                "egrave" => 'è',
                "ecirc" => 'ê',
                "agrave" => 'à',
                "acirc" => 'â',
                "ccedil" => 'ç',
                "ocirc" => 'ô',
                "ucirc" => 'û',
                "icirc" => 'î',
                "Eacute" => 'É',
                _ => {
                    let code = nom.strip_prefix('#')?;
                    let n = match code.strip_prefix(['x', 'X']) {
                        Some(h) => u32::from_str_radix(h, 16).ok()?,
                        None => code.parse().ok()?,
                    };
                    char::from_u32(n)?
                }
            };
            Some((c, f))
        });
        match decode {
            Some((c, f)) => {
                res.push(c);
                reste = &apres[f + 1..];
            }
            None => {
                res.push('&');
                reste = apres;
            }
        }
    }
    res.push_str(reste);
    res
}

/// Vrai si le texte ressemble à une page HTML contenant un tableau.
pub fn est_html(texte: &str) -> bool {
    let debut: String = texte.trim_start().chars().take(2_000).collect::<String>().to_ascii_lowercase();
    debut.starts_with('<') && (debut.contains("<table") || debut.contains("<tr") || debut.contains("<html"))
}

/// Lignes d'un tableau HTML (`<tr>` et cellules `<td>`/`<th>`), balises retirées, entités décodées.
pub fn lire_html(texte: &str) -> Vec<Enregistrement> {
    let bas = texte.to_ascii_lowercase();
    let mut res = Vec::new();
    let mut pos = 0;
    while let Some(i) = bas[pos..].find("<tr") {
        let debut = pos + i;
        let fin = bas[debut + 3..].find("</tr").map_or(bas.len(), |f| debut + 3 + f);
        let ligne_html = &texte[debut..fin];
        let ligne_bas = &bas[debut..fin];
        let mut champs = Vec::new();
        let mut p = 0;
        loop {
            let td = ligne_bas[p..].find("<td");
            let th = ligne_bas[p..].find("<th");
            let Some(c) = [td, th].into_iter().flatten().min() else { break };
            let ouverture = p + c;
            let Some(g) = ligne_bas[ouverture..].find('>') else { break };
            let contenu = ouverture + g + 1;
            let fermeture = [
                ligne_bas[contenu..].find("</td"),
                ligne_bas[contenu..].find("</th"),
                ligne_bas[contenu..].find("<td"),
                ligne_bas[contenu..].find("<th"),
            ]
            .into_iter()
            .flatten()
            .min()
            .map_or(ligne_bas.len(), |f| contenu + f);
            // Texte de la cellule, sans les balises internes (<b>, <span>…).
            let brut = &ligne_html[contenu..fermeture];
            let mut texte_cellule = String::new();
            let mut dans_balise = false;
            for ch in brut.chars() {
                match ch {
                    '<' => dans_balise = true,
                    '>' => dans_balise = false,
                    c if !dans_balise => texte_cellule.push(c),
                    _ => {}
                }
            }
            champs.push(entites(&texte_cellule).split_whitespace().collect::<Vec<_>>().join(" "));
            p = fermeture;
            if p >= ligne_bas.len() {
                break;
            }
        }
        if champs.iter().any(|c| !c.is_empty()) {
            let ligne = texte[..debut].matches('\n').count() + 1;
            res.push(Enregistrement { ligne, champs });
        }
        pos = fin.max(debut + 3);
    }
    res
}

// ---------- Champs d'une photo de portefeuille ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChampPosition {
    Nom,
    Isin,
    Symbole,
    Quantite,
    Pru,
}

/// Minuscules sans accents, mots séparés par `_` ; `buyingPrice` devient `buying_price`.
fn normaliser(s: &str) -> String {
    let mut res = String::new();
    let mut prec_minuscule = false;
    for c in s.trim().chars() {
        if c.is_uppercase() && prec_minuscule {
            res.push('_');
        }
        prec_minuscule = c.is_lowercase();
        for c in c.to_lowercase() {
            let c = match c {
                'à' | 'â' | 'ä' | 'á' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'î' | 'ï' | 'í' => 'i',
                'ô' | 'ö' | 'ó' => 'o',
                'ù' | 'û' | 'ü' | 'ú' => 'u',
                'ç' => 'c',
                c if c.is_alphanumeric() => c,
                _ => '_',
            };
            if !(c == '_' && (res.is_empty() || res.ends_with('_'))) {
                res.push(c);
            }
        }
    }
    res.trim_end_matches('_').to_string()
}

/// Champ d'une photo de portefeuille reconnu d'après l'intitulé d'une colonne.
pub fn champ_position(entete: &str) -> Option<ChampPosition> {
    let n = normaliser(entete);
    // Unités ou devise entre parenthèses ignorées : « PRU (€) », « Quantité (titres) ».
    let n = n.trim_end_matches("_eur").trim_end_matches("_euros").trim_end_matches('_');
    Some(match n {
        "name" | "nom" | "libelle" | "valeur" | "titre" | "designation" | "instrument" | "produit" | "nom_du_titre"
        | "libelle_valeur" | "security" | "actif" => ChampPosition::Nom,
        "isin" | "code_isin" | "isin_code" => ChampPosition::Isin,
        "ticker" | "symbole" | "symbol" | "mnemo" | "mnemonique" | "code_mnemonique" => ChampPosition::Symbole,
        "quantity" | "quantite" | "qte" | "qty" | "total_shares" | "shares" | "nombre" | "nombre_de_titres"
        | "nb_titres" | "quantite_detenue" | "nb_parts" | "nombre_de_parts" => ChampPosition::Quantite,
        "buying_price"
        | "price_average"
        | "average_price"
        | "pru"
        | "p_r_u"
        | "prix_de_revient"
        | "prix_revient"
        | "prix_de_revient_unitaire"
        | "prix_revient_unitaire"
        | "prix_moyen"
        | "prix_moyen_d_achat"
        | "prix_moyen_achat"
        | "cours_de_revient"
        | "cours_revient"
        | "pmp"
        | "cout_moyen"
        | "prix_d_achat_moyen"
        | "prix_achat_moyen"
        | "pam" => ChampPosition::Pru,
        _ => return None,
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Genre {
    /// Photo du portefeuille : champ de chaque colonne.
    Positions(Vec<Option<ChampPosition>>),
    /// Historique d'opérations (format de référence ou synonymes reconnus).
    Operations,
}

/// Correspondance des colonnes d'une photo de portefeuille, si l'en-tête en est une : quantité, PRU et au
/// moins un identifiant (ISIN, symbole ou nom). Chaque champ est pris à sa première colonne.
fn correspondance_positions(entetes: &[String]) -> Option<Vec<Option<ChampPosition>>> {
    let mut vus = Vec::new();
    let corr: Vec<Option<ChampPosition>> = entetes
        .iter()
        .map(|e| {
            let c = champ_position(e).filter(|c| !vus.contains(c))?;
            vus.push(c);
            Some(c)
        })
        .collect();
    let a = |c| vus.contains(&c);
    (a(ChampPosition::Quantite)
        && a(ChampPosition::Pru)
        && (a(ChampPosition::Isin) || a(ChampPosition::Symbole) || a(ChampPosition::Nom)))
    .then_some(corr)
}

/// Cherche la ligne d'en-tête dans les 30 premières lignes et classe le fichier.
/// Rend l'indice de l'en-tête dans `lignes` et le genre, ou `None` si rien n'est reconnu.
pub fn reconnaitre(lignes: &[Enregistrement]) -> Option<(usize, Genre)> {
    for (i, l) in lignes.iter().enumerate().take(30) {
        let entetes: Vec<String> = l.champs.iter().map(|c| c.trim().to_string()).collect();
        if verifier_correspondance(&proposer_correspondance(&entetes)).is_ok() {
            return Some((i, Genre::Operations));
        }
        if let Some(c) = correspondance_positions(&entetes) {
            return Some((i, Genre::Positions(c)));
        }
    }
    None
}

/// Nom du format reconnu, pour l'affichage.
pub fn nom_du_format(entetes: &[String]) -> &'static str {
    let n: Vec<String> = entetes.iter().map(|e| normaliser(e)).collect();
    let a = |x: &str| n.iter().any(|e| e == x);
    if a("total_shares") && a("price_average") {
        "Moning"
    } else if a("buying_price") && a("amount_variation") {
        "BoursoBank"
    } else if n.iter().all(|e| champ_d_entete(e).is_some() || e.is_empty()) {
        "Escarcelle"
    } else {
        "tableau"
    }
}

/// Ligne lue dans une photo de portefeuille.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionLue {
    pub nom: Option<String>,
    pub isin: Option<String>,
    pub symbole: Option<String>,
    /// Mnémonique Euronext écrit entre parenthèses à la fin du libellé (Fortuneo : « TOTALENERGIES (TTE) »).
    pub mnemonique: Option<String>,
    pub quantite_micro: i64,
    pub pru_micro: i64,
}

/// Sépare le mnémonique final d'un libellé : « DASSAULT AVIATION (AM) » → (« DASSAULT AVIATION », AM).
/// « (-) » (pas de mnémonique) est retiré du libellé. Une parenthèse qui n'est pas un mnémonique (« (MSCI World) »)
/// reste dans le libellé.
pub fn separer_mnemonique(libelle: &str) -> (String, Option<String>) {
    let l = libelle.trim();
    if let Some(debut) = l.strip_suffix(')').and_then(|x| x.rfind('(').map(|i| (i, x))) {
        let (i, x) = debut;
        let code = x[i + 1..].trim();
        let nom = x[..i].trim().to_string();
        if code == "-" && !nom.is_empty() {
            return (nom, None);
        }
        if (1..=6).contains(&code.len())
            && code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && code.chars().any(|c| c.is_ascii_uppercase())
            && !nom.is_empty()
        {
            return (nom, Some(code.to_string()));
        }
    }
    (l.to_string(), None)
}

/// Nom à chercher sur Internet : libellé sans les mentions propres au compte (« PF28 » des actions à prime de
/// fidélité, « ACC », parenthèses).
pub fn nom_de_recherche(nom: &str) -> String {
    let sans_parentheses: String = {
        let mut res = String::new();
        let mut profondeur = 0;
        for c in nom.chars() {
            match c {
                '(' => profondeur += 1,
                ')' if profondeur > 0 => profondeur -= 1,
                c if profondeur == 0 => res.push(c),
                _ => {}
            }
        }
        res
    };
    sans_parentheses
        .split_whitespace()
        .filter(|m| {
            let pf = m.strip_prefix("PF").is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
            !pf && *m != "-"
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Vrai si les lignes avant l'en-tête désignent un PEA-PME (titre de l'export), que l'on ne suit pas.
pub fn export_pea_pme(lignes: &[Enregistrement], entete: usize) -> bool {
    lignes[..entete.min(lignes.len())].iter().flat_map(|l| &l.champs).any(|c| {
        let n: String = normaliser(c).chars().filter(|c| *c != '_').collect();
        n.contains("peapme")
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LecturePosition {
    Position(PositionLue),
    /// Ligne sans quantité (total, liquidités, ligne soldée) : ignorée sans erreur.
    Ignoree(String),
    Erreur(Vec<String>),
}

/// Lit une ligne d'une photo de portefeuille.
pub fn lire_position(champs: &[String], corr: &[Option<ChampPosition>], dec: Decimale) -> LecturePosition {
    let val = |c: ChampPosition| -> &str {
        corr.iter().position(|x| *x == Some(c)).and_then(|i| champs.get(i)).map(|s| s.trim()).unwrap_or("")
    };
    let (nom, mnemonique) =
        separer_mnemonique(&val(ChampPosition::Nom).split_whitespace().collect::<Vec<_>>().join(" "));
    let nom = Some(nom).filter(|s| !s.is_empty()).map(|s| s.chars().take(120).collect::<String>());
    let isin = Some(val(ChampPosition::Isin).replace(' ', "").to_ascii_uppercase()).filter(|s| !s.is_empty());
    let symbole = Some(val(ChampPosition::Symbole).trim().to_ascii_uppercase()).filter(|s| !s.is_empty());
    let (q, p) = (val(ChampPosition::Quantite), val(ChampPosition::Pru));
    // Total, sous-total, liquidités : ni quantité, ni (quantité et ISIN). Ignorés sans erreur.
    if (q.is_empty() && p.is_empty()) || (q.is_empty() && isin.is_none()) {
        let quoi = nom.clone().or(isin.clone()).unwrap_or_else(|| "ligne".into());
        return LecturePosition::Ignoree(format!("{quoi} : sans quantité (total ou liquidités)"));
    }
    let mut erreurs = Vec::new();
    let quantite = match lire_nombre(q, dec, 6) {
        Ok(Some(0)) => {
            return LecturePosition::Ignoree(format!("{} : quantité nulle (ligne soldée)", nom.unwrap_or_default()))
        }
        Ok(Some(v)) if v < 0 => {
            erreurs.push("quantité négative".to_string());
            0
        }
        Ok(Some(v)) if v > QUANTITE_MAX_MICRO => {
            erreurs.push("quantité supérieure à dix millions de titres".into());
            0
        }
        Ok(Some(v)) => v,
        Ok(None) => {
            erreurs.push("quantité manquante".into());
            0
        }
        Err(e) => {
            erreurs.push(format!("quantité : {e}"));
            0
        }
    };
    let pru = match lire_nombre(p, dec, 6) {
        Ok(Some(v)) if v < 0 => {
            erreurs.push("prix de revient négatif".into());
            0
        }
        Ok(Some(v)) if v > PRIX_MAX_MICRO => {
            erreurs.push("prix de revient supérieur à 1 000 000 €".into());
            0
        }
        Ok(Some(v)) => v,
        Ok(None) => {
            erreurs.push("prix de revient manquant".into());
            0
        }
        Err(e) => {
            erreurs.push(format!("prix de revient : {e}"));
            0
        }
    };
    if let Some(i) = &isin {
        if !isin_valide(i) {
            erreurs.push(format!("ISIN « {i} » invalide (clé de contrôle)"));
        }
    }
    if isin.is_none() && symbole.is_none() && nom.is_none() {
        erreurs.push("ni ISIN, ni symbole, ni nom".into());
    }
    if !erreurs.is_empty() {
        return LecturePosition::Erreur(erreurs);
    }
    LecturePosition::Position(PositionLue { nom, isin, symbole, mnemonique, quantite_micro: quantite, pru_micro: pru })
}

/// Regroupe deux lignes du même titre : quantités additionnées, PRU moyen pondéré par les quantités.
pub fn regrouper(a: &PositionLue, b: &PositionLue) -> PositionLue {
    let q = a.quantite_micro.saturating_add(b.quantite_micro);
    let cout = a.quantite_micro as i128 * a.pru_micro as i128 + b.quantite_micro as i128 * b.pru_micro as i128;
    let pru = if q > 0 { crate::div_round(cout, q as i128) } else { 0 };
    PositionLue {
        nom: a.nom.clone().or(b.nom.clone()),
        isin: a.isin.clone().or(b.isin.clone()),
        symbole: a.symbole.clone().or(b.symbole.clone()),
        mnemonique: a.mnemonique.clone().or(b.mnemonique.clone()),
        quantite_micro: q,
        pru_micro: pru,
    }
}

/// Date d'une cellule de tableur (numéro de série Excel, système 1900) au format AAAA-MM-JJ.
pub fn date_excel(serie: f64) -> Option<crate::Date> {
    if !(1.0..=2_958_465.0).contains(&serie) {
        return None;
    }
    // Le 30/12/1899 est le jour 0 (Excel compte à tort un 29/02/1900 : à partir du 01/03/1900, l'écart est bon).
    let jours = serie.floor() as i64 - 25_569;
    Some(crate::Date::depuis_jours_unix(jours))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import_csv::{decoder, detecter_decimale, detecter_separateur, lire_csv};

    fn lire(octets: &[u8]) -> (Vec<Enregistrement>, char) {
        let (t, _) = decoder(octets);
        let sep = detecter_separateur(&t);
        (lire_csv(&t, sep), sep)
    }

    fn positions(octets: &[u8]) -> (&'static str, Vec<LecturePosition>) {
        let (lignes, sep) = lire(octets);
        let (i, genre) = reconnaitre(&lignes).expect("format reconnu");
        let Genre::Positions(corr) = genre else { panic!("photo de portefeuille attendue") };
        let donnees = &lignes[i + 1..];
        let valeurs = donnees.iter().flat_map(|l| {
            l.champs
                .iter()
                .zip(&corr)
                .filter(|(_, c)| matches!(c, Some(ChampPosition::Quantite | ChampPosition::Pru)))
                .map(|(v, _)| v.as_str())
        });
        let dec = detecter_decimale(valeurs, sep);
        (nom_du_format(&lignes[i].champs), donnees.iter().map(|l| lire_position(&l.champs, &corr, dec)).collect())
    }

    fn pos(nom: &str, isin: &str, symbole: Option<&str>, q: i64, pru: i64) -> LecturePosition {
        LecturePosition::Position(PositionLue {
            nom: Some(nom.into()),
            isin: Some(isin.into()),
            symbole: symbole.map(String::from),
            mnemonique: None,
            quantite_micro: q,
            pru_micro: pru,
        })
    }

    /// Export réel de Moning (séparateur virgule, point décimal, symbole Yahoo fourni).
    #[test]
    fn export_moning() {
        let f = b"Ticker,Total Shares,Price average,Price average in stock currency,Name,Latest Price,Custom asset yield,Dividend Yield,Value Stock Currency,Value,Value invested,Return,Dividends,Price Variation One Year,ISIN,Dividend safety,Growth score\n\
AI.PA,11.00,140.99,true,Air Liquide SA,190.32,,1.79,2093.52,2093.52,1550.89,34.99,36.96,30,FR0000120073,,\n\
DCAM.PA,1618.00,5.32,true,Amundi PEA Monde (MSCI World) UCITS ETF,6.10,,0.0,9876.27,9876.27,8601.29,14.82,0.0,13,FR001400U5Q4,,\n\
AM.PA,10.00,156.99,true,Dassault Aviation SA,301.40,,1.6,3014.0,3014.0,1569.86,91.99,47.8,14,FR0014004L86,,\n\
TTE.PA,50.00,35.07,true,TotalEnergies SE,78.36,,4.62,3918.0,3918.0,1753.35,123.46,182.5,45,FR0000120271,,\n";
        let (format, l) = positions(f);
        assert_eq!(format, "Moning");
        assert_eq!(
            l,
            vec![
                pos("Air Liquide SA", "FR0000120073", Some("AI.PA"), 11_000_000, 140_990_000),
                pos(
                    "Amundi PEA Monde (MSCI World) UCITS ETF",
                    "FR001400U5Q4",
                    Some("DCAM.PA"),
                    1_618_000_000,
                    5_320_000
                ),
                pos("Dassault Aviation SA", "FR0014004L86", Some("AM.PA"), 10_000_000, 156_990_000),
                pos("TotalEnergies SE", "FR0000120271", Some("TTE.PA"), 50_000_000, 35_070_000),
            ]
        );
    }

    /// Export réel de BoursoBank (UTF-8 avec marque d'ordre, point-virgule, virgule décimale).
    #[test]
    fn export_boursobank() {
        let f = "\u{FEFF}name;isin;quantity;buyingPrice;lastPrice;intradayVariation;amount;amountVariation;variation\nNEURONES;FR0004050250;11,00;35,83;40,10;1,13;441,10;46,94;11,91\n";
        let (format, l) = positions(f.as_bytes());
        assert_eq!(format, "BoursoBank");
        assert_eq!(l, vec![pos("NEURONES", "FR0004050250", None, 11_000_000, 35_830_000)]);
    }

    /// Export de banque avec un titre avant l'en-tête, des montants en texte français et une ligne de total.
    #[test]
    fn en_tete_apres_un_titre_et_ligne_de_total() {
        let f = "Mon portefeuille PEA;;;\nArrêté au 26/11/2026;;;\n;;;\nLibellé;Code ISIN;Quantité;Prix de revient (€)\nAIR LIQUIDE;FR0000120073;11;140,99\nTOTAL;;;\nLiquidités;;;1 234,50\n";
        let (_, l) = positions(f.as_bytes());
        assert_eq!(l[0], pos("AIR LIQUIDE", "FR0000120073", None, 11_000_000, 140_990_000));
        assert!(matches!(l[1], LecturePosition::Ignoree(_)));
        assert!(matches!(l[2], LecturePosition::Ignoree(_)), "{:?}", l[2]);
    }

    /// Export réel de Fortuneo (classeur .xls, première feuille) : quatre lignes de titre avant l'en-tête,
    /// mnémonique entre parenthèses dans le libellé, ligne d'actions à prime de fidélité (PF28, ISIN provisoire).
    /// Cellules telles que les rend la lecture du classeur (nombres écrits avec une virgule).
    #[test]
    fn export_fortuneo() {
        let t = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let lignes: Vec<Enregistrement> = [
            t(&["Portefeuille xxxxxxxxxx", "", "", "", "", "", "", "", "", "", "", ""]),
            t(&["(PEA M xxxxxxxxxxx)"]),
            t(&["26/11/2026"]),
            t(&[
                "Libellé",
                "Cours",
                "",
                "Dev",
                "Var/Veille",
                "Qté",
                "PRU",
                "Valorisation",
                "+/- values",
                "+/- values (%)",
                "Poids",
                "ISIN",
            ]),
            t(&[
                "AIR LIQUIDE PF28   (-)",
                "190,32",
                "",
                "EUR",
                "0.99%",
                "11",
                "140,99",
                "2093,52",
                "542,63",
                "34.99%",
                "11.0758%",
                "FR0014010OO5",
            ]),
            t(&[
                "AMUNDI PEA MONDE (MSCI World) UCITS ETF - Acc EUR ACC (DCAM)",
                "6,104",
                "",
                "EUR",
                "2.85%",
                "1618",
                "5,3164",
                "9876,272",
                "1274,3368",
                "14.81%",
                "52.2505%",
                "FR001400U5Q4",
            ]),
            t(&[
                "DASSAULT AVIATION (AM)",
                "301,4",
                "",
                "EUR",
                "3.01%",
                "10",
                "156,986",
                "3014",
                "1444,14",
                "91.99%",
                "15.9456%",
                "FR0014004L86",
            ]),
            t(&[
                "TOTALENERGIES (TTE)",
                "78,36",
                "",
                "EUR",
                "1.4%",
                "50",
                "35,0666",
                "3918",
                "2164,67",
                "123.46%",
                "20.7282%",
                "FR0000120271",
            ]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, champs)| Enregistrement { ligne: i + 1, champs })
        .collect();
        let (i, g) = reconnaitre(&lignes).unwrap();
        assert_eq!(i, 3);
        assert!(!export_pea_pme(&lignes, i));
        let Genre::Positions(c) = g else { panic!() };
        let l: Vec<LecturePosition> =
            lignes[i + 1..].iter().map(|l| lire_position(&l.champs, &c, Decimale::Virgule)).collect();
        let p = |nom: &str, isin: &str, m: Option<&str>, q: i64, pru: i64| {
            LecturePosition::Position(PositionLue {
                nom: Some(nom.into()),
                isin: Some(isin.into()),
                symbole: None,
                mnemonique: m.map(String::from),
                quantite_micro: q,
                pru_micro: pru,
            })
        };
        assert_eq!(
            l,
            vec![
                p("AIR LIQUIDE PF28", "FR0014010OO5", None, 11_000_000, 140_990_000),
                p(
                    "AMUNDI PEA MONDE (MSCI World) UCITS ETF - Acc EUR ACC",
                    "FR001400U5Q4",
                    Some("DCAM"),
                    1_618_000_000,
                    5_316_400
                ),
                p("DASSAULT AVIATION", "FR0014004L86", Some("AM"), 10_000_000, 156_986_000),
                p("TOTALENERGIES", "FR0000120271", Some("TTE"), 50_000_000, 35_066_600),
            ]
        );
        assert_eq!(nom_de_recherche("AIR LIQUIDE PF28"), "AIR LIQUIDE");
        assert_eq!(nom_de_recherche("AMUNDI PEA MONDE (MSCI World) UCITS ETF"), "AMUNDI PEA MONDE UCITS ETF");
        let mut pme = lignes.clone();
        pme[1].champs[0] = "(PEA-PME 0123)".into();
        assert!(export_pea_pme(&pme, i));
    }

    #[test]
    fn mnemoniques() {
        assert_eq!(separer_mnemonique("TOTALENERGIES (TTE)"), ("TOTALENERGIES".into(), Some("TTE".into())));
        assert_eq!(separer_mnemonique("X (-)"), ("X".into(), None));
        assert_eq!(separer_mnemonique("ETF (MSCI World)"), ("ETF (MSCI World)".into(), None));
        assert_eq!(separer_mnemonique("(TTE)"), ("(TTE)".into(), None));
        assert_eq!(separer_mnemonique("ACME"), ("ACME".into(), None));
    }

    #[test]
    fn tableau_html() {
        let h = "<html><body><table><tr><th>Libell&eacute;</th><th>ISIN</th><th>Qt&eacute;</th><th>PRU</th></tr>\n\
                 <tr><td><b>TOTALENERGIES</b></td><td>FR0000120271</td><td>50</td><td>35,07&nbsp;&euro;</td></tr></table>";
        assert!(est_html(h));
        let lignes = lire_html(h);
        assert_eq!(lignes[0].champs, vec!["Libellé", "ISIN", "Qté", "PRU"]);
        assert_eq!(lignes[1].champs, vec!["TOTALENERGIES", "FR0000120271", "50", "35,07 €"]);
        let (i, g) = reconnaitre(&lignes).unwrap();
        assert_eq!(i, 0);
        let Genre::Positions(c) = g else { panic!() };
        assert_eq!(
            lire_position(&lignes[1].champs, &c, Decimale::Virgule),
            pos("TOTALENERGIES", "FR0000120271", None, 50_000_000, 35_070_000)
        );
        assert_eq!(entites("a &amp; b &#233;&#xE9; &inconnu; &"), "a & b éé &inconnu; &");
    }

    #[test]
    fn historique_d_operations_reconnu() {
        let (lignes, _) = lire(crate::import_csv::modele().as_bytes());
        assert_eq!(reconnaitre(&lignes), Some((0, Genre::Operations)));
        assert_eq!(nom_du_format(&lignes[0].champs), "Escarcelle");
        let (lignes, _) = lire(b"a;b\n1;2\n");
        assert_eq!(reconnaitre(&lignes), None);
    }

    #[test]
    fn erreurs_et_regroupement() {
        let c = vec![Some(ChampPosition::Isin), Some(ChampPosition::Quantite), Some(ChampPosition::Pru)];
        let l = |s: &str| lire_position(&s.split(';').map(String::from).collect::<Vec<_>>(), &c, Decimale::Virgule);
        assert!(matches!(l("FR0000120272;1;1"), LecturePosition::Erreur(_)));
        assert!(matches!(l("FR0000120271;-1;1"), LecturePosition::Erreur(_)));
        assert!(matches!(l("FR0000120271;1;abc"), LecturePosition::Erreur(_)));
        assert!(matches!(l("FR0000120271;0;10"), LecturePosition::Ignoree(_)));
        let (LecturePosition::Position(a), LecturePosition::Position(b)) =
            (l("FR0000120271;10;30"), l("FR0000120271;30;40"))
        else {
            panic!()
        };
        let r = regrouper(&a, &b);
        assert_eq!((r.quantite_micro, r.pru_micro), (40_000_000, 37_500_000));
    }

    #[test]
    fn dates_excel() {
        assert_eq!(date_excel(45_658.0).unwrap().to_string(), "2025-01-01");
        assert_eq!(date_excel(61.0).unwrap().to_string(), "1900-03-01");
        assert_eq!(date_excel(-1.0), None);
    }

    #[test]
    fn intitules() {
        assert_eq!(champ_position("buyingPrice"), Some(ChampPosition::Pru));
        assert_eq!(champ_position("Price average"), Some(ChampPosition::Pru));
        assert_eq!(champ_position("Price average in stock currency"), None);
        assert_eq!(champ_position("P.R.U."), Some(ChampPosition::Pru));
        assert_eq!(champ_position("Prix de revient unitaire (EUR)"), Some(ChampPosition::Pru));
        assert_eq!(champ_position("Total Shares"), Some(ChampPosition::Quantite));
        assert_eq!(champ_position("Qté"), Some(ChampPosition::Quantite));
        assert_eq!(champ_position("Ticker"), Some(ChampPosition::Symbole));
    }
}
