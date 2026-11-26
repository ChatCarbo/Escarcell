//! Lecture des fichiers CSV d'opérations (format de référence Escarcelle ou colonnes associées par
//! l'utilisateur), sans entrée/sortie : décodage, séparateurs, champs, nombres, dates et contrôle de chaque
//! ligne prise isolément. Les contrôles qui dépendent des données déjà enregistrées (doublons, plafonds,
//! quantités détenues) sont faits par l'application avec [`crate::livret`] et [`crate::journal`].
//!
//! Format de référence (une ligne d'en-tête, puis une opération par ligne) :
//! `date;compte;type;isin;libelle;quantite;prix_unitaire;montant;frais;devise;note`

use crate::identifiants::isin_valide;
use crate::livret::MONTANT_MAX;
use crate::{Date, Money, Price};

/// Quantité maximale d'une opération : dix millions de titres (en millionièmes).
pub const QUANTITE_MAX_MICRO: i64 = 10_000_000 * 1_000_000;
/// Cours unitaire maximal : un million d'euros (en millionièmes).
pub const PRIX_MAX_MICRO: i64 = 1_000_000 * 1_000_000;

// ---------- Décodage ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encodage {
    Utf8,
    Utf16,
    Windows1252,
}

impl Encodage {
    pub fn nom(self) -> &'static str {
        match self {
            Encodage::Utf8 => "UTF-8",
            Encodage::Utf16 => "UTF-16",
            Encodage::Windows1252 => "Windows-1252",
        }
    }
}

/// Caractères 0x80 à 0x9F de Windows-1252 (les cinq positions non attribuées gardent leur code).
const CP1252: [u16; 32] = [
    0x20AC, 0x0081, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008D,
    0x017D, 0x008F, 0x0090, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A,
    0x0153, 0x009D, 0x017E, 0x0178,
];

/// Décode un fichier : UTF-16 si une marque d'ordre l'indique, UTF-8 s'il est valide, sinon Windows-1252
/// (encodage des exports Excel en français). La marque d'ordre éventuelle est retirée.
pub fn decoder(octets: &[u8]) -> (String, Encodage) {
    let utf16 = |be: bool| {
        let unites = octets[2..].chunks_exact(2).map(|c| {
            if be {
                u16::from_be_bytes([c[0], c[1]])
            } else {
                u16::from_le_bytes([c[0], c[1]])
            }
        });
        char::decode_utf16(unites).map(|r| r.unwrap_or('\u{FFFD}')).collect::<String>()
    };
    if octets.starts_with(&[0xFF, 0xFE]) {
        return (utf16(false), Encodage::Utf16);
    }
    if octets.starts_with(&[0xFE, 0xFF]) {
        return (utf16(true), Encodage::Utf16);
    }
    let sans_bom = octets.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(octets);
    if let Ok(s) = std::str::from_utf8(sans_bom) {
        return (s.to_string(), Encodage::Utf8);
    }
    let s = octets
        .iter()
        .map(|&b| match b {
            0x80..=0x9F => char::from_u32(CP1252[(b - 0x80) as usize] as u32).unwrap_or('\u{FFFD}'),
            _ => b as char,
        })
        .collect();
    (s, Encodage::Windows1252)
}

// ---------- Lecture des enregistrements ----------

/// Enregistrement CSV : numéro de la ligne du fichier où il commence (à partir de 1) et ses champs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Enregistrement {
    pub ligne: usize,
    pub champs: Vec<String>,
}

/// Découpe un texte CSV (guillemets doubles, guillemets doublés, retours à la ligne dans un champ entre
/// guillemets, fins de ligne CRLF, LF ou CR). Les lignes entièrement vides sont ignorées.
pub fn lire_csv(texte: &str, sep: char) -> Vec<Enregistrement> {
    let mut res = Vec::new();
    let mut champs: Vec<String> = Vec::new();
    let mut champ = String::new();
    let mut entre_guillemets = false;
    let mut ligne = 1usize;
    let mut debut = 1usize;
    let mut it = texte.chars().peekable();
    let mut fin_enregistrement = |champs: &mut Vec<String>, champ: &mut String, debut: usize| {
        champs.push(std::mem::take(champ));
        let c = std::mem::take(champs);
        if !(c.len() == 1 && c[0].trim().is_empty()) {
            res.push(Enregistrement { ligne: debut, champs: c });
        }
    };
    while let Some(c) = it.next() {
        if entre_guillemets {
            match c {
                '"' if it.peek() == Some(&'"') => {
                    it.next();
                    champ.push('"');
                }
                '"' => entre_guillemets = false,
                '\n' => {
                    ligne += 1;
                    champ.push('\n');
                }
                '\r' => {
                    if it.peek() == Some(&'\n') {
                        it.next();
                    }
                    ligne += 1;
                    champ.push('\n');
                }
                _ => champ.push(c),
            }
            continue;
        }
        match c {
            '"' if champ.trim().is_empty() => {
                champ.clear();
                entre_guillemets = true;
            }
            c if c == sep => champs.push(std::mem::take(&mut champ)),
            '\n' | '\r' => {
                if c == '\r' && it.peek() == Some(&'\n') {
                    it.next();
                }
                fin_enregistrement(&mut champs, &mut champ, debut);
                ligne += 1;
                debut = ligne;
            }
            _ => champ.push(c),
        }
    }
    if !champ.is_empty() || !champs.is_empty() {
        fin_enregistrement(&mut champs, &mut champ, debut);
    }
    res
}

/// Séparateur de champs le plus probable parmi `;`, tabulation et `,` : celui qui donne le plus de lignes
/// ayant le même nombre de champs que l'en-tête (au moins deux), sur les 50 premières lignes.
/// À égalité, l'ordre de préférence est `;`, tabulation, `,`.
pub fn detecter_separateur(texte: &str) -> char {
    let extrait: String = texte.lines().take(50).collect::<Vec<_>>().join("\n");
    let mut meilleur = (';', 0usize, 0usize);
    for sep in [';', '\t', ','] {
        let lignes = lire_csv(&extrait, sep);
        let Some(entete) = lignes.first() else { continue };
        let n = entete.champs.len();
        if n < 2 {
            continue;
        }
        let coherentes = lignes.iter().filter(|l| l.champs.len() == n).count();
        if (coherentes, n) > (meilleur.1, meilleur.2) {
            meilleur = (sep, coherentes, n);
        }
    }
    meilleur.0
}

// ---------- Nombres ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decimale {
    Virgule,
    Point,
}

impl Decimale {
    fn car(self) -> char {
        match self {
            Decimale::Virgule => ',',
            Decimale::Point => '.',
        }
    }
}

/// Retire les espaces (y compris insécables), le symbole euro et un signe plus.
fn nettoyer_nombre(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace() && !matches!(c, '\u{A0}' | '\u{202F}' | '€' | '\'')).collect()
}

/// `1,234` ou `12.345.678` : groupes de trois chiffres, ambigus entre décimales et milliers.
fn groupes_de_trois(s: &str, sep: char) -> bool {
    let mut parts = s.trim_start_matches('-').split(sep);
    let premier = parts.next().unwrap_or("");
    let reste: Vec<&str> = parts.collect();
    !reste.is_empty()
        && (1..=3).contains(&premier.len())
        && premier.chars().all(|c| c.is_ascii_digit())
        && reste.iter().all(|p| p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))
}

/// Séparateur décimal le plus probable d'après des valeurs numériques du fichier.
/// Avec le séparateur de champs `,`, c'est forcément le point. Sinon, vote des valeurs non ambiguës
/// (à égalité ou sans indice : la virgule, usage français).
pub fn detecter_decimale<'a>(valeurs: impl IntoIterator<Item = &'a str>, sep: char) -> Decimale {
    if sep == ',' {
        return Decimale::Point;
    }
    let (mut virgule, mut point) = (0usize, 0usize);
    for v in valeurs {
        let v = nettoyer_nombre(v);
        match (v.rfind(','), v.rfind('.')) {
            (Some(a), Some(b)) => {
                if a > b {
                    virgule += 1
                } else {
                    point += 1
                }
            }
            (Some(_), None) if !groupes_de_trois(&v, ',') => virgule += 1,
            (None, Some(_)) if !groupes_de_trois(&v, '.') => point += 1,
            _ => {}
        }
    }
    if point > virgule {
        Decimale::Point
    } else {
        Decimale::Virgule
    }
}

/// Lit un nombre décimal et le rend en unités de 10^-`decimales` (centimes pour 2, millionièmes pour 6).
/// Accepte les espaces et l'autre séparateur comme séparateurs de milliers (groupes de trois chiffres),
/// un signe, le symbole €. Refuse plus de `decimales` décimales sauf zéros finaux. `Ok(None)` si vide.
pub fn lire_nombre(s: &str, dec: Decimale, decimales: u32) -> Result<Option<i64>, String> {
    let brut = s.trim();
    let v = nettoyer_nombre(brut);
    if v.is_empty() {
        return Ok(None);
    }
    let invalide = || format!("« {brut} » n'est pas un nombre");
    let (negatif, v) = match v.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, v.strip_prefix('+').unwrap_or(v.as_str())),
    };
    let milliers = match dec {
        Decimale::Virgule => '.',
        Decimale::Point => ',',
    };
    let (entier, fraction) = match v.rsplit_once(dec.car()) {
        Some((e, f)) => (e, f),
        None => (v, ""),
    };
    if fraction.contains(milliers) || (entier.is_empty() && fraction.is_empty()) {
        return Err(invalide());
    }
    let entier: String = if entier.contains(milliers) {
        if !groupes_de_trois(entier, milliers) {
            return Err(invalide());
        }
        entier.chars().filter(|&c| c != milliers).collect()
    } else {
        entier.to_string()
    };
    if !entier.chars().all(|c| c.is_ascii_digit()) || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return Err(invalide());
    }
    let significatives = fraction.trim_end_matches('0');
    if significatives.len() > decimales as usize {
        return Err(format!("« {brut} » a trop de décimales ({decimales} au plus)"));
    }
    let mut n: i128 = 0;
    for c in entier.chars().chain(significatives.chars()) {
        n = n * 10 + c.to_digit(10).unwrap_or(0) as i128;
        if n > i64::MAX as i128 {
            return Err(format!("« {brut} » est trop grand"));
        }
    }
    n *= 10i128.pow(decimales - significatives.len() as u32);
    if n > i64::MAX as i128 {
        return Err(format!("« {brut} » est trop grand"));
    }
    Ok(Some(if negatif { -(n as i64) } else { n as i64 }))
}

// ---------- Dates ----------

/// Lit `AAAA-MM-JJ` (ou `AAAA/MM/JJ`) et `JJ/MM/AAAA` (ou `JJ-MM-AAAA`, `JJ.MM.AAAA`). L'année doit
/// avoir quatre chiffres ; une heure éventuelle après la date (« 2024-01-10 00:00:00 ») est ignorée.
pub fn lire_date(s: &str) -> Option<Date> {
    let s = s.trim();
    let s = s.split([' ', 'T']).next().unwrap_or("");
    let parts: Vec<&str> = s.split(['-', '/', '.']).collect();
    if parts.len() != 3 || parts.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    let (a, m, j) = if parts[0].len() == 4 && parts[1].len() <= 2 && parts[2].len() <= 2 {
        (parts[0], parts[1], parts[2])
    } else if parts[2].len() == 4 && parts[0].len() <= 2 && parts[1].len() <= 2 {
        (parts[2], parts[1], parts[0])
    } else {
        return None;
    };
    Date::new(a.parse().ok()?, m.parse().ok()?, j.parse().ok()?).ok()
}

// ---------- Champs et correspondance des colonnes ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Champ {
    Date,
    Compte,
    Type,
    Isin,
    Libelle,
    Quantite,
    PrixUnitaire,
    Montant,
    Frais,
    Devise,
    Note,
}

impl Champ {
    pub const TOUS: [Champ; 11] = [
        Champ::Date,
        Champ::Compte,
        Champ::Type,
        Champ::Isin,
        Champ::Libelle,
        Champ::Quantite,
        Champ::PrixUnitaire,
        Champ::Montant,
        Champ::Frais,
        Champ::Devise,
        Champ::Note,
    ];

    /// Nom de la colonne dans le format de référence.
    pub fn nom(self) -> &'static str {
        match self {
            Champ::Date => "date",
            Champ::Compte => "compte",
            Champ::Type => "type",
            Champ::Isin => "isin",
            Champ::Libelle => "libelle",
            Champ::Quantite => "quantite",
            Champ::PrixUnitaire => "prix_unitaire",
            Champ::Montant => "montant",
            Champ::Frais => "frais",
            Champ::Devise => "devise",
            Champ::Note => "note",
        }
    }

    pub fn depuis_nom(nom: &str) -> Option<Champ> {
        Champ::TOUS.into_iter().find(|c| c.nom() == nom)
    }

    pub fn obligatoire(self) -> bool {
        matches!(self, Champ::Date | Champ::Compte | Champ::Type)
    }

    fn numerique(self) -> bool {
        matches!(self, Champ::Quantite | Champ::PrixUnitaire | Champ::Montant | Champ::Frais)
    }
}

/// Minuscules sans accents ; espaces, tirets, points et apostrophes remplacés par `_` (sans doublon).
fn normaliser(s: &str) -> String {
    let mut res = String::new();
    for c in s.trim().chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' | 'í' => 'i',
            'ô' | 'ö' | 'ó' => 'o',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'ç' => 'c',
            'œ' => {
                res.push('o');
                'e'
            }
            c if c.is_alphanumeric() => c,
            _ => '_',
        };
        if !(c == '_' && (res.is_empty() || res.ends_with('_'))) {
            res.push(c);
        }
    }
    res.trim_end_matches('_').to_string()
}

/// Champ correspondant à un intitulé de colonne (format de référence ou synonymes courants).
pub fn champ_d_entete(entete: &str) -> Option<Champ> {
    let n = normaliser(entete);
    let c = match n.as_str() {
        "date" | "date_operation" | "date_d_operation" | "date_de_l_operation" | "date_execution" => Champ::Date,
        "compte" | "enveloppe" | "support" => Champ::Compte,
        "type" | "operation" | "type_operation" | "type_d_operation" | "nature" | "sens" => Champ::Type,
        "isin" | "code_isin" => Champ::Isin,
        "libelle" | "designation" | "nom" | "titre" | "valeur" => Champ::Libelle,
        "quantite" | "qte" | "qty" | "nombre" | "nb_titres" | "nombre_de_titres" => Champ::Quantite,
        "prix_unitaire" | "prix" | "cours" | "prix_unitaire_eur" | "cours_execution" | "cours_d_execution" => {
            Champ::PrixUnitaire
        }
        "montant" | "montant_eur" | "montant_net" | "montant_en_euros" => Champ::Montant,
        "frais" | "courtage" | "commission" | "frais_de_courtage" => Champ::Frais,
        "devise" | "monnaie" => Champ::Devise,
        "note" | "commentaire" | "remarque" | "notes" => Champ::Note,
        _ => return None,
    };
    Some(c)
}

/// Correspondance proposée : chaque colonne reconnue est associée à son champ (une seule fois par champ).
pub fn proposer_correspondance(entetes: &[String]) -> Vec<Option<Champ>> {
    let mut vus = Vec::new();
    entetes
        .iter()
        .map(|e| {
            let c = champ_d_entete(e).filter(|c| !vus.contains(c))?;
            vus.push(c);
            Some(c)
        })
        .collect()
}

/// Vérifie une correspondance : date, compte et type associés, chaque champ au plus une fois.
pub fn verifier_correspondance(corr: &[Option<Champ>]) -> Result<(), String> {
    for c in Champ::TOUS {
        let n = corr.iter().filter(|x| **x == Some(c)).count();
        if n > 1 {
            return Err(format!("La colonne « {} » est associée {n} fois.", c.nom()));
        }
        if n == 0 && c.obligatoire() {
            return Err(format!("Associez une colonne au champ obligatoire « {} ».", c.nom()));
        }
    }
    Ok(())
}

/// Valeurs des champs numériques des enregistrements, pour [`detecter_decimale`].
pub fn valeurs_numeriques<'a>(
    lignes: &'a [Enregistrement],
    corr: &'a [Option<Champ>],
) -> impl Iterator<Item = &'a str> + 'a {
    lignes.iter().flat_map(move |l| {
        l.champs.iter().zip(corr.iter()).filter(|(_, c)| c.is_some_and(Champ::numerique)).map(|(v, _)| v.as_str())
    })
}

// ---------- Comptes et types ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum CompteImport {
    Pea,
    LivretA,
    Ldds,
    Lep,
}

impl CompteImport {
    /// Type de compte dans la base.
    pub fn code(self) -> &'static str {
        match self {
            CompteImport::Pea => "PEA",
            CompteImport::LivretA => "LIVRET_A",
            CompteImport::Ldds => "LDDS",
            CompteImport::Lep => "LEP",
        }
    }

    pub fn libelle(self) -> &'static str {
        match self {
            CompteImport::Pea => "PEA",
            CompteImport::LivretA => "Livret A",
            CompteImport::Ldds => "LDDS",
            CompteImport::Lep => "LEP",
        }
    }

    pub fn est_livret(self) -> bool {
        self != CompteImport::Pea
    }

    fn lire(s: &str) -> Result<CompteImport, String> {
        let n: String = normaliser(s).chars().filter(|c| *c != '_').collect();
        Ok(match n.as_str() {
            "pea" => CompteImport::Pea,
            "livreta" | "la" => CompteImport::LivretA,
            "ldds" | "ldd" | "livretdeveloppementdurable" | "livretdeveloppementdurableetsolidaire" => {
                CompteImport::Ldds
            }
            "lep" | "livretdepargnepopulaire" => CompteImport::Lep,
            "peapme" => return Err("le PEA-PME n'est pas suivi par Escarcelle".into()),
            "" => return Err("compte manquant".into()),
            _ => return Err(format!("compte « {} » inconnu (PEA, Livret A, LDDS ou LEP)", s.trim())),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TypeImport {
    Achat,
    Vente,
    Dividende,
    Versement,
    Retrait,
    Frais,
    /// Intérêts d'un livret : recalculés par Escarcelle, jamais importés (comparés au calcul).
    Interets,
    /// Solde de départ d'un livret (relevé à une date).
    Solde,
}

impl TypeImport {
    /// Type d'opération dans la base (`None` pour les intérêts, qui ne sont pas enregistrés).
    pub fn code(self) -> Option<&'static str> {
        Some(match self {
            TypeImport::Achat => "ACHAT",
            TypeImport::Vente => "VENTE",
            TypeImport::Dividende => "DIVIDENDE",
            TypeImport::Versement => "VERSEMENT",
            TypeImport::Retrait => "RETRAIT",
            TypeImport::Frais => "FRAIS",
            TypeImport::Solde => "VALORISATION",
            TypeImport::Interets => return None,
        })
    }

    pub fn nom(self) -> &'static str {
        match self {
            TypeImport::Achat => "achat",
            TypeImport::Vente => "vente",
            TypeImport::Dividende => "dividende",
            TypeImport::Versement => "versement",
            TypeImport::Retrait => "retrait",
            TypeImport::Frais => "frais",
            TypeImport::Interets => "interets",
            TypeImport::Solde => "solde",
        }
    }

    fn lire(s: &str) -> Result<TypeImport, String> {
        let n: String = normaliser(s).chars().filter(|c| *c != '_').collect();
        Ok(match n.as_str() {
            "achat" | "achatcomptant" | "buy" => TypeImport::Achat,
            "vente" | "ventecomptant" | "sell" => TypeImport::Vente,
            "dividende" | "dividendes" | "coupon" => TypeImport::Dividende,
            "versement" | "depot" | "apport" => TypeImport::Versement,
            "retrait" => TypeImport::Retrait,
            "frais" => TypeImport::Frais,
            "interets" | "interet" => TypeImport::Interets,
            "solde" | "soldedepart" | "soldededepart" => TypeImport::Solde,
            "" => return Err("type manquant".into()),
            _ => return Err(format!("type « {} » inconnu", s.trim())),
        })
    }

    fn admis(self, compte: CompteImport) -> bool {
        use TypeImport::*;
        match compte {
            CompteImport::Pea => matches!(self, Achat | Vente | Dividende | Versement | Retrait | Frais),
            _ => matches!(self, Versement | Retrait | Interets | Solde),
        }
    }
}

// ---------- Ligne ----------

/// Opération lue sur une ligne du fichier, contrôlée isolément.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LigneImport {
    pub date: Date,
    pub compte: CompteImport,
    pub type_: TypeImport,
    pub isin: Option<String>,
    pub quantite_micro: Option<i64>,
    pub prix: Option<Price>,
    /// Montant en euros : pour un achat ou une vente, quantité × prix (hors frais), arrondi au centime.
    pub montant: Money,
    /// Courtage d'un achat ou d'une vente.
    pub frais: Money,
    /// Libellé et note réunis (200 caractères au plus).
    pub libelle: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnalyseLigne {
    pub ligne: Option<LigneImport>,
    pub erreurs: Vec<String>,
    pub avertissements: Vec<String>,
}

/// Date de création du PEA (loi du 16 juillet 1992) : aucune opération ne peut être antérieure.
fn creation_pea() -> Date {
    Date::ymd(1992, 7, 16)
}

/// Contrôle une ligne isolément. `champs` peut être plus court que la correspondance (champs vides).
pub fn analyser_ligne(champs: &[String], corr: &[Option<Champ>], dec: Decimale) -> AnalyseLigne {
    let mut a = AnalyseLigne::default();
    let val = |c: Champ| -> &str {
        corr.iter().position(|x| *x == Some(c)).and_then(|i| champs.get(i)).map(|s| s.trim()).unwrap_or("")
    };
    if champs.len() > corr.len() && champs[corr.len()..].iter().any(|c| !c.trim().is_empty()) {
        a.erreurs.push(format!("{} colonnes au lieu de {}", champs.len(), corr.len()));
    }

    let date = match val(Champ::Date) {
        "" => {
            a.erreurs.push("date manquante".into());
            None
        }
        s => {
            let d = lire_date(s);
            if d.is_none() {
                a.erreurs.push(format!("date « {s} » invalide (AAAA-MM-JJ ou JJ/MM/AAAA)"));
            }
            d
        }
    };
    let compte = CompteImport::lire(val(Champ::Compte)).map_err(|e| a.erreurs.push(e)).ok();
    let type_ = TypeImport::lire(val(Champ::Type)).map_err(|e| a.erreurs.push(e)).ok();
    if let (Some(c), Some(t)) = (compte, type_) {
        if !t.admis(c) {
            let admis = if c.est_livret() {
                "versement, retrait, interets ou solde"
            } else {
                "achat, vente, dividende, versement, retrait ou frais"
            };
            a.erreurs.push(format!("type « {} » impossible sur un {} ({admis})", t.nom(), c.libelle()));
        }
    }

    let mut nombre = |c: Champ, decimales: u32| match lire_nombre(val(c), dec, decimales) {
        Ok(v) => v,
        Err(e) => {
            a.erreurs.push(format!("{} : {e}", c.nom()));
            None
        }
    };
    let quantite = nombre(Champ::Quantite, 6);
    let prix = nombre(Champ::PrixUnitaire, 6);
    let montant = nombre(Champ::Montant, 2);
    let frais = nombre(Champ::Frais, 2);

    match val(Champ::Devise).to_ascii_uppercase().as_str() {
        "" | "EUR" | "€" | "EURO" | "EUROS" => {}
        d => a.erreurs.push(format!("devise « {d} » : seul l'euro est accepté")),
    }

    let isin = match val(Champ::Isin).to_ascii_uppercase().replace(' ', "") {
        s if s.is_empty() => None,
        s => {
            if !isin_valide(&s) {
                a.erreurs.push(format!("ISIN « {s} » invalide (clé de contrôle)"));
            }
            Some(s)
        }
    };

    let libelle = {
        let (l, n) = (val(Champ::Libelle), val(Champ::Note));
        let t = match (l.is_empty(), n.is_empty()) {
            (true, true) => String::new(),
            (false, true) => l.to_string(),
            (true, false) => n.to_string(),
            (false, false) => format!("{l} — {n}"),
        };
        let t: String = t.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(200).collect();
        (!t.is_empty()).then_some(t)
    };

    let (Some(date), Some(compte), Some(type_)) = (date, compte, type_) else { return a };
    if !a.erreurs.is_empty() {
        return a;
    }

    // Dates possibles.
    if compte.est_livret() {
        let debut = crate::params::embarque::debut();
        if date < debut {
            a.erreurs.push(format!(
                "Escarcelle connaît les taux des livrets depuis le {:02}/{:02}/{} : remplacez les opérations plus anciennes par une ligne « solde » à cette date",
                debut.day(),
                debut.month(),
                debut.year()
            ));
        }
    } else if date < creation_pea() {
        a.erreurs.push("date antérieure à la création du PEA (juillet 1992)".into());
    }

    use TypeImport::*;
    let titres = matches!(type_, Achat | Vente);
    // ISIN : obligatoire pour les titres et les dividendes, interdit ailleurs.
    match (matches!(type_, Achat | Vente | Dividende), &isin) {
        (true, None) => a.erreurs.push("ISIN manquant".into()),
        (false, Some(_)) => a.erreurs.push(format!("pas d'ISIN pour un « {} »", type_.nom())),
        _ => {}
    }
    let frais = frais.unwrap_or(0);
    if frais < 0 {
        a.erreurs.push("frais négatifs".into());
    }
    if frais != 0 && !titres {
        a.erreurs.push(format!("frais sur un « {} » : utilisez une ligne de type frais", type_.nom()));
    }

    let montant_final;
    let (mut q_ok, mut p_ok) = (None, None);
    if titres {
        match quantite {
            None => a.erreurs.push("quantité manquante".into()),
            Some(q) if q <= 0 => a.erreurs.push("la quantité doit être positive".into()),
            Some(q) if q > QUANTITE_MAX_MICRO => a.erreurs.push("quantité supérieure à dix millions de titres".into()),
            Some(q) => q_ok = Some(q),
        }
        match prix {
            None => a.erreurs.push("prix unitaire manquant".into()),
            Some(p) if p <= 0 => a.erreurs.push("le prix unitaire doit être positif".into()),
            Some(p) if p > PRIX_MAX_MICRO => a.erreurs.push("prix unitaire supérieur à 1 000 000 €".into()),
            Some(p) => p_ok = Some(p),
        }
        let (Some(q), Some(p)) = (q_ok, p_ok) else { return a };
        let brut = Price::from_micro(p).valeur(q);
        if brut.cents() > MONTANT_MAX.cents() {
            a.erreurs.push("montant supérieur à 10 000 000 €".into());
            return a;
        }
        // Le montant du fichier est facultatif ; s'il est donné, il doit correspondre au calcul (frais
        // compris ou non, au centime près).
        if let Some(m) = montant {
            let m = m.abs();
            let f = frais;
            let possibles = [brut.cents(), brut.cents() + f, brut.cents() - f];
            if !possibles.iter().any(|v| (v - m).abs() <= 1) {
                a.avertissements.push(format!(
                    "montant indiqué {} différent de quantité × prix ({brut}) : le calcul est retenu",
                    Money::from_cents(m)
                ));
            }
        }
        montant_final = brut;
    } else {
        if quantite.is_some() || prix.is_some() {
            a.erreurs.push(format!("quantité et prix réservés aux achats et ventes (pas pour un « {} »)", type_.nom()));
        }
        match montant {
            None => {
                a.erreurs.push("montant manquant".into());
                return a;
            }
            // Un montant négatif est admis pour une sortie d'argent (relevés bancaires signés).
            Some(m) if m < 0 && matches!(type_, Retrait | Frais) => montant_final = Money::from_cents(-m),
            Some(m) if m <= 0 => {
                a.erreurs.push("le montant doit être positif".into());
                return a;
            }
            Some(m) => montant_final = Money::from_cents(m),
        }
        if montant_final.cents() > MONTANT_MAX.cents() {
            a.erreurs.push("montant supérieur à 10 000 000 €".into());
        }
    }
    if !a.erreurs.is_empty() {
        return a;
    }
    a.ligne = Some(LigneImport {
        date,
        compte,
        type_,
        isin,
        quantite_micro: q_ok,
        prix: p_ok.map(Price::from_micro),
        montant: montant_final,
        frais: Money::from_cents(frais),
        libelle,
    });
    a
}

/// Modèle téléchargeable : en-tête de référence et exemples commentés par leur note.
pub fn modele() -> String {
    let lignes = [
        "date;compte;type;isin;libelle;quantite;prix_unitaire;montant;frais;devise;note",
        "2025-01-02;PEA;versement;;;;;5000,00;;EUR;Versement initial",
        "2025-01-06;PEA;achat;FR001400U5Q4;Amundi PEA Monde;100;45,1234;;1,99;EUR;Montant calculé : quantité × prix",
        "2025-06-12;PEA;dividende;FR0000120271;TotalEnergies;;;23,50;;EUR;",
        "2025-09-15;PEA;vente;FR001400U5Q4;Amundi PEA Monde;20;52,10;;1,99;EUR;",
        "2025-01-01;Livret A;solde;;;;;10000,00;;EUR;Solde relevé à cette date",
        "2025-03-10;Livret A;versement;;;;;1500,00;;EUR;",
        "2025-11-02;LDDS;versement;;;;;300,00;;EUR;",
        "2025-12-31;Livret A;interets;;;;;240,17;;EUR;Comparé au calcul d'Escarcelle, non importé",
    ];
    let mut s = lignes.join("\r\n");
    s.push_str("\r\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn champs(s: &str) -> Vec<String> {
        s.split(';').map(String::from).collect()
    }

    fn reference() -> Vec<Option<Champ>> {
        Champ::TOUS.into_iter().map(Some).collect()
    }

    #[test]
    fn decodage() {
        assert_eq!(decoder("é;€".as_bytes()), ("é;€".into(), Encodage::Utf8));
        assert_eq!(decoder(b"\xEF\xBB\xBFdate").0, "date");
        assert_eq!(decoder(b"Int\xe9r\xeats \x80"), ("Intérêts €".into(), Encodage::Windows1252));
        assert_eq!(decoder(b"\xFF\xFEa\x00\xe9\x00"), ("aé".into(), Encodage::Utf16));
    }

    #[test]
    fn lecture_csv_guillemets_et_fins_de_ligne() {
        let t = "a;b;c\r\n\"x;y\";\"il a dit \"\"oui\"\"\";\"deux\nlignes\"\r\n\r\n1;2;3";
        let r = lire_csv(t, ';');
        assert_eq!(r.len(), 3);
        assert_eq!(r[1].champs, vec!["x;y", "il a dit \"oui\"", "deux\nlignes"]);
        assert_eq!(r[1].ligne, 2);
        assert_eq!(r[2].ligne, 5);
        assert_eq!(lire_csv("a,b\rc,d", ',').len(), 2);
    }

    #[test]
    fn separateur() {
        assert_eq!(detecter_separateur("date;compte;montant\n2024-01-01;PEA;1,5"), ';');
        assert_eq!(detecter_separateur("date\tcompte\tmontant\n2024-01-01\tPEA\t1,5"), '\t');
        assert_eq!(detecter_separateur("date,compte,montant\n2024-01-01,PEA,1.5"), ',');
        // Virgule décimale non protégée : la virgule ne donne pas des lignes cohérentes.
        assert_eq!(detecter_separateur("date;montant\n2024-01-01;1,5\n2024-01-02;12"), ';');
        // Égalité parfaite : le point-virgule (usage français) l'emporte.
        assert_eq!(detecter_separateur("a;b,c\n1;2,3"), ';');
        assert_eq!(detecter_separateur("a\tb,c\n1\t2,3"), '\t');
    }

    #[test]
    fn nombres() {
        let v = Decimale::Virgule;
        let p = Decimale::Point;
        assert_eq!(lire_nombre("1 234,56", v, 2), Ok(Some(123_456)));
        assert_eq!(lire_nombre("1\u{202F}234,5 €", v, 2), Ok(Some(123_450)));
        assert_eq!(lire_nombre("1.234,56", v, 2), Ok(Some(123_456)));
        assert_eq!(lire_nombre("1,234.56", p, 2), Ok(Some(123_456)));
        assert_eq!(lire_nombre("-12", v, 2), Ok(Some(-1_200)));
        assert_eq!(lire_nombre("+0,5", v, 6), Ok(Some(500_000)));
        assert_eq!(lire_nombre(",5", v, 2), Ok(Some(50)));
        assert_eq!(lire_nombre("10,500", v, 2), Ok(Some(1_050)));
        assert_eq!(lire_nombre("", v, 2), Ok(None));
        assert!(lire_nombre("10,505", v, 2).is_err());
        assert!(lire_nombre("1.23,4", v, 2).is_err());
        assert!(lire_nombre("12a", v, 2).is_err());
        assert!(lire_nombre("1,2,3", v, 2).is_err());
        assert!(lire_nombre("99999999999999999999", v, 2).is_err());
        assert!(lire_nombre("-", v, 2).is_err());
    }

    #[test]
    fn decimale_detectee() {
        assert_eq!(detecter_decimale(["12,5", "3"], ';'), Decimale::Virgule);
        assert_eq!(detecter_decimale(["12.5", "1,234"], ';'), Decimale::Point);
        assert_eq!(detecter_decimale(["1,234"], ';'), Decimale::Virgule);
        assert_eq!(detecter_decimale(["1.234,5", "1,234.5", "2.5"], ';'), Decimale::Point);
        assert_eq!(detecter_decimale(["12,5"], ','), Decimale::Point);
    }

    #[test]
    fn dates() {
        let d = |s: &str| lire_date(s).map(|d| d.to_string());
        assert_eq!(d("2024-02-29").as_deref(), Some("2024-02-29"));
        assert_eq!(d("29/02/2024").as_deref(), Some("2024-02-29"));
        assert_eq!(d("1.3.2024").as_deref(), Some("2024-03-01"));
        assert_eq!(d("2024-01-10 00:00:00").as_deref(), Some("2024-01-10"));
        assert_eq!(d("2024-01-10T12:00").as_deref(), Some("2024-01-10"));
        assert_eq!(d("29/02/2023"), None);
        assert_eq!(d("10/01/24"), None);
        assert_eq!(d("2024-13-01"), None);
        assert_eq!(d(""), None);
    }

    #[test]
    fn correspondance() {
        let e: Vec<String> =
            ["Date d'opération", "Compte", "Opération", "Code ISIN", "Qté", "Cours", "Montant", "Autre", "date"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        let c = proposer_correspondance(&e);
        assert_eq!(
            c,
            vec![
                Some(Champ::Date),
                Some(Champ::Compte),
                Some(Champ::Type),
                Some(Champ::Isin),
                Some(Champ::Quantite),
                Some(Champ::PrixUnitaire),
                Some(Champ::Montant),
                None,
                None
            ]
        );
        assert!(verifier_correspondance(&c).is_ok());
        assert!(verifier_correspondance(&[Some(Champ::Date), Some(Champ::Compte)]).is_err());
        assert!(verifier_correspondance(&[
            Some(Champ::Date),
            Some(Champ::Compte),
            Some(Champ::Type),
            Some(Champ::Date)
        ])
        .is_err());
        let m = modele();
        let entete = lire_csv(&m, ';').remove(0).champs;
        assert_eq!(proposer_correspondance(&entete), reference());
    }

    #[test]
    fn achat_complet() {
        let a = analyser_ligne(
            &champs("06/01/2025;pea;Achat;fr001400u5q4;Amundi;100;45,1234;4514,33;1,99;EUR;test"),
            &reference(),
            Decimale::Virgule,
        );
        assert!(a.erreurs.is_empty() && a.avertissements.is_empty(), "{a:?}");
        let l = a.ligne.unwrap();
        assert_eq!(l.isin.as_deref(), Some("FR001400U5Q4"));
        assert_eq!(l.montant, Money::from_cents(451_234));
        assert_eq!(l.frais, Money::from_cents(199));
        assert_eq!(l.libelle.as_deref(), Some("Amundi — test"));
    }

    #[test]
    fn montant_incoherent_averti() {
        let a =
            analyser_ligne(&champs("2025-01-06;PEA;achat;FR001400U5Q4;;10;10;150;;;"), &reference(), Decimale::Virgule);
        assert_eq!(a.avertissements.len(), 1);
        assert_eq!(a.ligne.unwrap().montant, Money::from_cents(10_000));
    }

    #[test]
    fn erreurs_de_ligne() {
        let r = reference();
        let err = |s: &str| analyser_ligne(&champs(s), &r, Decimale::Virgule).erreurs;
        assert!(!err("2025-01-06;PEA;achat;FR001400U5Q5;;10;10;;;;").is_empty(), "clé ISIN");
        assert!(!err("2025-01-06;PEA;achat;;;10;10;;;;").is_empty(), "ISIN manquant");
        assert!(!err("2025-01-06;PEA;achat;FR001400U5Q4;;0;10;;;;").is_empty(), "quantité nulle");
        assert!(!err("2025-01-06;PEA;interets;;;;;10;;;").is_empty(), "intérêts sur le PEA");
        assert!(!err("2025-01-06;Livret A;achat;FR001400U5Q4;;1;1;;;;").is_empty(), "achat sur livret");
        assert!(!err("2025-01-06;Livret A;versement;;;;;10;;USD;").is_empty(), "devise");
        assert!(!err("2019-12-31;Livret A;versement;;;;;10;;;").is_empty(), "avant les taux connus");
        assert!(!err("1990-01-01;PEA;versement;;;;;10;;;").is_empty(), "avant le PEA");
        assert!(!err("2025-01-06;PEA-PME;versement;;;;;10;;;").is_empty(), "PEA-PME");
        assert!(!err("2025-01-06;PEA;versement;;;;;-10;;;").is_empty(), "versement négatif");
        assert!(!err("2025-01-06;PEA;versement;;;;;10;1;;").is_empty(), "frais sur un versement");
        assert!(!err("2025-01-06;PEA;versement;;;;;10000000,01;;;").is_empty(), "plus de 10 M€");
        assert!(!err("2025-01-06;PEA;achat;FR001400U5Q4;;10000001;1;;;;").is_empty(), "quantité");
        assert!(!err("2025-01-06;PEA;versement;;;;;10;;;;x").is_empty(), "colonne en trop");
        assert!(err("2025-01-06;PEA;retrait;;;;;-10;;;").is_empty(), "retrait signé");
        assert!(err("2025-01-06;PEA;versement;;;;;10;;;;").is_empty(), "colonne vide en trop");
    }

    #[test]
    fn ligne_courte_et_colonnes_absentes() {
        let corr = vec![Some(Champ::Date), Some(Champ::Compte), Some(Champ::Type), Some(Champ::Montant)];
        let a = analyser_ligne(&champs("2025-03-10;LDDS;versement"), &corr, Decimale::Virgule);
        assert_eq!(a.erreurs, vec!["montant manquant".to_string()]);
        let a = analyser_ligne(&champs("2025-03-10;LDDS;Versement;1 500"), &corr, Decimale::Virgule);
        assert_eq!(a.ligne.unwrap().montant, Money::from_euros(1_500));
    }

    /// Aucune entrée ne fait paniquer le lecteur (octets et lignes pseudo-aléatoires).
    #[test]
    fn robustesse_entrees_quelconques() {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut alea = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        let alphabet: Vec<char> = "0123456789;,.\t\"\r\n -+€éAZaz/ ".chars().collect();
        for _ in 0..2_000 {
            let n = (alea() % 80) as usize;
            let octets: Vec<u8> = (0..n).map(|_| alea() as u8).collect();
            let (t, _) = decoder(&octets);
            let _ = lire_csv(&t, detecter_separateur(&t));
            let s: String = (0..n).map(|_| alphabet[(alea() % alphabet.len() as u64) as usize]).collect();
            let sep = detecter_separateur(&s);
            let lignes = lire_csv(&s, sep);
            let corr = reference();
            let dec = detecter_decimale(valeurs_numeriques(&lignes, &corr), sep);
            for l in &lignes {
                let _ = analyser_ligne(&l.champs, &corr, dec);
                for c in &l.champs {
                    let _ = lire_nombre(c, dec, 2);
                    let _ = lire_date(c);
                }
            }
        }
    }
}
