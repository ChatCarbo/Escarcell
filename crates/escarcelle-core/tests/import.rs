//! Import CSV et journal du PEA : centaines de fichiers et d'historiques générés automatiquement.
//!
//! - Aller-retour : des opérations tirées au hasard sont écrites dans toutes les variantes de format admises
//!   (séparateurs, séparateur décimal, dates, milliers, guillemets, fins de ligne, encodage, ordre et
//!   intitulés des colonnes), puis relues : on doit retrouver exactement les mêmes opérations.
//! - Journal : quantités et PRU comparés à un calcul indépendant en fractions exactes.

use escarcelle_core::import_csv::*;
use escarcelle_core::journal::{rejouer, ErreurJournal, MouvementTitre, SensTitre};
use escarcelle_core::{Date, Money, Price};

struct Alea(u64);

impl Alea {
    fn suivant(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn entre(&mut self, a: i64, b: i64) -> i64 {
        a + (self.suivant() % (b - a + 1) as u64) as i64
    }
    fn choix<'a, T>(&mut self, t: &'a [T]) -> &'a T {
        &t[(self.suivant() % t.len() as u64) as usize]
    }
}

fn d(s: &str) -> Date {
    s.parse().unwrap()
}

const ISINS: [&str; 4] = ["FR001400U5Q4", "FR0000120271", "IE00B4L5Y983", "FR0000131104"];

/// Opération tirée au hasard, valide.
fn operation(a: &mut Alea) -> LigneImport {
    let compte = *a.choix(&[CompteImport::Pea, CompteImport::LivretA, CompteImport::Ldds, CompteImport::Lep]);
    let type_ = if compte == CompteImport::Pea {
        *a.choix(&[
            TypeImport::Achat,
            TypeImport::Vente,
            TypeImport::Dividende,
            TypeImport::Versement,
            TypeImport::Retrait,
            TypeImport::Frais,
        ])
    } else {
        *a.choix(&[TypeImport::Versement, TypeImport::Retrait, TypeImport::Interets, TypeImport::Solde])
    };
    let date = Date::depuis_jours_unix(d("2020-02-01").jours_unix() + a.entre(0, 2_400));
    let titres = matches!(type_, TypeImport::Achat | TypeImport::Vente);
    let (quantite_micro, prix, montant, frais) = if titres {
        // Quantités entières ou fractionnaires, prix jusqu'à 6 décimales.
        let q = if a.entre(0, 3) == 0 { a.entre(1, 9_999_999) } else { a.entre(1, 5_000) * 1_000_000 };
        let p = Price::from_micro(a.entre(1, 2_000_000_000) * *a.choix(&[1, 10, 10_000]) / 10);
        let mut p = Price::from_micro(p.micro().clamp(1, PRIX_MAX_MICRO));
        // Montant d'une opération limité à 10 000 000 €.
        while p.valeur(q).cents() > 1_000_000_000 {
            p = Price::from_micro(p.micro() / 10 + 1);
        }
        let m = p.valeur(q);
        let f = if a.entre(0, 1) == 0 { 0 } else { a.entre(1, 2_500) };
        (Some(q), Some(p), m, Money::from_cents(f))
    } else {
        (None, None, Money::from_cents(a.entre(1, 2_000_000)), Money::ZERO)
    };
    let isin = (titres || type_ == TypeImport::Dividende).then(|| a.choix(&ISINS).to_string());
    let libelle = match a.entre(0, 3) {
        0 => None,
        1 => Some("Intérêts « été » ; café".to_string()),
        2 => Some("Ligne \"citée\", avec virgule".to_string()),
        _ => Some("Amundi PEA Monde".to_string()),
    };
    LigneImport { date, compte, type_, isin, quantite_micro, prix, montant, frais, libelle }
}

struct Format {
    sep: char,
    dec: Decimale,
    date_fr: bool,
    milliers: bool,
    crlf: bool,
    windows_1252: bool,
    /// Colonnes dans l'ordre du fichier, avec leur intitulé.
    colonnes: Vec<(Champ, &'static str)>,
    /// Écrire le montant des achats et ventes (frais compris ou non).
    montant_titres: u8,
}

fn format(a: &mut Alea) -> Format {
    let sep = *a.choix(&[';', ',', '\t']);
    let dec = if sep == ',' { Decimale::Point } else { *a.choix(&[Decimale::Virgule, Decimale::Point]) };
    let intitules: [(Champ, &[&'static str]); 11] = [
        (Champ::Date, &["date", "Date d'opération", "DATE"]),
        (Champ::Compte, &["compte", "Enveloppe"]),
        (Champ::Type, &["type", "Opération", "Nature"]),
        (Champ::Isin, &["isin", "Code ISIN"]),
        (Champ::Libelle, &["libelle", "Libellé", "Désignation"]),
        (Champ::Quantite, &["quantite", "Quantité", "Qté"]),
        (Champ::PrixUnitaire, &["prix_unitaire", "Cours", "Prix"]),
        (Champ::Montant, &["montant", "Montant (EUR)", "Montant"]),
        (Champ::Frais, &["frais", "Courtage"]),
        (Champ::Devise, &["devise", "Devise"]),
        (Champ::Note, &["note", "Commentaire"]),
    ];
    let mut colonnes: Vec<(Champ, &'static str)> =
        intitules.iter().map(|(c, noms)| (*c, *a.choix(noms))).filter(|(c, _)| *c != Champ::Note).collect();
    // Mélange de l'ordre des colonnes.
    for i in (1..colonnes.len()).rev() {
        let j = (a.suivant() % (i as u64 + 1)) as usize;
        colonnes.swap(i, j);
    }
    // « Montant (EUR) » n'est pas un synonyme connu : on ne garde que des intitulés reconnus.
    for c in colonnes.iter_mut() {
        if champ_d_entete(c.1) != Some(c.0) {
            c.1 = c.0.nom();
        }
    }
    Format {
        sep,
        dec,
        date_fr: a.entre(0, 1) == 0,
        milliers: a.entre(0, 1) == 0,
        crlf: a.entre(0, 1) == 0,
        windows_1252: a.entre(0, 1) == 0,
        colonnes,
        montant_titres: a.entre(0, 2) as u8,
    }
}

/// Écrit un nombre en unités de 10^-`decimales` dans le format choisi.
fn ecrire_nombre(v: i64, decimales: u32, f: &Format) -> String {
    if v < 0 {
        return format!("-{}", ecrire_nombre(-v, decimales, f));
    }
    let div = 10i64.pow(decimales);
    let (e, r) = (v / div, v % div);
    let mut entier = e.to_string();
    if f.milliers && entier.len() > 3 {
        let sep_milliers = match f.dec {
            Decimale::Virgule => *[" ", "\u{A0}", "\u{202F}", "."].get((v % 4) as usize).unwrap_or(&" "),
            Decimale::Point => *[" ", ","].get((v % 2) as usize).unwrap_or(&" "),
        };
        let chiffres: Vec<char> = entier.chars().collect();
        let mut s = String::new();
        for (i, c) in chiffres.iter().enumerate() {
            if i > 0 && (chiffres.len() - i) % 3 == 0 {
                s.push_str(sep_milliers);
            }
            s.push(*c);
        }
        entier = s;
    }
    let point = if f.dec == Decimale::Virgule { ',' } else { '.' };
    let fraction = format!("{:0width$}", r, width = decimales as usize);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        entier
    } else {
        format!("{entier}{point}{fraction}")
    }
}

fn protege(s: &str, sep: char) -> String {
    if s.contains(sep) || s.contains('"') || s.contains('\n') || s.contains(',') || s.contains('.') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn ecrire(ops: &[LigneImport], f: &Format) -> Vec<u8> {
    let fin = if f.crlf { "\r\n" } else { "\n" };
    let sep = f.sep.to_string();
    let mut t: String = f.colonnes.iter().map(|(_, n)| protege(n, f.sep)).collect::<Vec<_>>().join(&sep);
    t.push_str(fin);
    for o in ops {
        let ligne: Vec<String> = f
            .colonnes
            .iter()
            .map(|(c, _)| match c {
                Champ::Date => {
                    if f.date_fr {
                        format!("{:02}/{:02}/{}", o.date.day(), o.date.month(), o.date.year())
                    } else {
                        o.date.to_string()
                    }
                }
                Champ::Compte => o.compte.libelle().to_string(),
                Champ::Type => {
                    let n = o.type_.nom();
                    if o.date.day() % 2 == 0 {
                        n.to_uppercase()
                    } else {
                        n.to_string()
                    }
                }
                Champ::Isin => o.isin.clone().unwrap_or_default(),
                Champ::Libelle => protege(o.libelle.as_deref().unwrap_or(""), f.sep),
                Champ::Quantite => {
                    o.quantite_micro.map(|q| protege(&ecrire_nombre(q, 6, f), f.sep)).unwrap_or_default()
                }
                Champ::PrixUnitaire => {
                    o.prix.map(|p| protege(&ecrire_nombre(p.micro(), 6, f), f.sep)).unwrap_or_default()
                }
                Champ::Montant => {
                    let m = match (o.quantite_micro.is_some(), f.montant_titres) {
                        (true, 0) => return String::new(),
                        (true, 1) => o.montant.cents(),
                        (true, _) if o.type_ == TypeImport::Achat => o.montant.cents() + o.frais.cents(),
                        (true, _) => o.montant.cents() - o.frais.cents(),
                        // Relevé signé pour les sorties d'argent, une fois sur deux.
                        (false, _) if o.type_ == TypeImport::Retrait && o.date.day() % 2 == 0 => -o.montant.cents(),
                        _ => o.montant.cents(),
                    };
                    if m < 0 && (o.type_ == TypeImport::Vente) {
                        // Frais supérieurs au produit de la vente : montant non écrit.
                        return String::new();
                    }
                    protege(&ecrire_nombre(m, 2, f), f.sep)
                }
                Champ::Frais => {
                    if o.frais.cents() == 0 {
                        String::new()
                    } else {
                        protege(&ecrire_nombre(o.frais.cents(), 2, f), f.sep)
                    }
                }
                Champ::Devise => if o.date.day() % 3 == 0 { "EUR" } else { "" }.to_string(),
                Champ::Note => String::new(),
            })
            .collect();
        t.push_str(&ligne.join(&sep));
        t.push_str(fin);
    }
    if f.windows_1252 {
        t.chars()
            .map(|c| match c {
                '€' => 0x80,
                '\u{A0}' => 0xA0,
                // Pas d'espace fine insécable en Windows-1252 : espace insécable à la place.
                '\u{202F}' => 0xA0,
                c if (c as u32) < 256 => c as u32 as u8,
                _ => b'?',
            })
            .collect()
    } else {
        t.into_bytes()
    }
}

/// Lecture complète d'un fichier, comme le fait l'application.
fn relire(octets: &[u8]) -> (Vec<AnalyseLigne>, Encodage, char, Decimale) {
    let (texte, enc) = decoder(octets);
    let sep = detecter_separateur(&texte);
    let mut lignes = lire_csv(&texte, sep);
    let entete = lignes.remove(0);
    let corr = proposer_correspondance(&entete.champs);
    verifier_correspondance(&corr).unwrap();
    let dec = detecter_decimale(valeurs_numeriques(&lignes, &corr), sep);
    (lignes.iter().map(|l| analyser_ligne(&l.champs, &corr, dec)).collect(), enc, sep, dec)
}

#[test]
fn aller_retour_sur_500_fichiers() {
    let mut a = Alea(0x5EED_CAFE_F00D_0001);
    for essai in 0..500 {
        let f = format(&mut a);
        let n = a.entre(5, 40) as usize;
        let mut ops: Vec<LigneImport> = (0..n).map(|_| operation(&mut a)).collect();
        ops.push(LigneImport {
            date: d("2025-03-10"),
            compte: CompteImport::Ldds,
            type_: TypeImport::Versement,
            isin: None,
            quantite_micro: None,
            prix: None,
            montant: Money::from_cents(1_234),
            frais: Money::ZERO,
            libelle: None,
        });
        let octets = ecrire(&ops, &f);
        let (lues, enc, sep, dec) = relire(&octets);
        assert_eq!(sep, f.sep, "essai {essai} : séparateur");
        assert_eq!(lues.len(), ops.len(), "essai {essai} : nombre de lignes");
        // Chaque fichier contient au moins une valeur décimale non ambiguë (dernière opération) : le
        // séparateur décimal doit être retrouvé.
        assert_eq!(dec, f.dec, "essai {essai} : séparateur décimal");
        if f.windows_1252 && ops.iter().any(|o| o.libelle.as_deref().is_some_and(|l| l.contains('é'))) {
            assert_eq!(enc, Encodage::Windows1252);
        }
        for (i, (o, l)) in ops.iter().zip(&lues).enumerate() {
            assert!(l.erreurs.is_empty(), "essai {essai}, ligne {i} : {:?}\n{o:?}", l.erreurs);
            assert!(l.avertissements.is_empty(), "essai {essai}, ligne {i} : {:?}", l.avertissements);
            // Tous les caractères des libellés existent en Windows-1252 : rien ne doit être perdu.
            assert_eq!(l.ligne.as_ref().unwrap(), o, "essai {essai}, ligne {i}");
        }
    }
}

#[test]
fn modele_relu_sans_erreur() {
    let (lues, enc, sep, dec) = relire(modele().as_bytes());
    assert_eq!((enc, sep, dec), (Encodage::Utf8, ';', Decimale::Virgule));
    assert_eq!(lues.len(), 8);
    for l in &lues {
        assert!(l.erreurs.is_empty() && l.avertissements.is_empty(), "{l:?}");
    }
    let achat = lues[1].ligne.as_ref().unwrap();
    assert_eq!(achat.montant, Money::from_cents(451_234));
    assert_eq!(achat.frais, Money::from_cents(199));
}

// ---------- Journal ----------

/// Fraction exacte (numérateur, dénominateur > 0), réduite. `None` en cas de dépassement : le cas
/// n'est alors pas vérifiable par cet oracle et il est compté à part.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frac(i128, i128);

fn pgcd(a: i128, b: i128) -> i128 {
    if b == 0 {
        a.abs().max(1)
    } else {
        pgcd(b, a % b)
    }
}

impl Frac {
    fn new(n: i128, d: i128) -> Frac {
        let g = pgcd(n, d);
        Frac(n / g, d / g)
    }
    fn plus(self, o: Frac) -> Option<Frac> {
        Some(Frac::new(self.0.checked_mul(o.1)?.checked_add(o.0.checked_mul(self.1)?)?, self.1.checked_mul(o.1)?))
    }
    fn fois(self, o: Frac) -> Option<Frac> {
        // Réduction croisée avant le produit pour limiter la taille des nombres.
        let (g1, g2) = (pgcd(self.0, o.1), pgcd(o.0, self.1));
        Some(Frac::new((self.0 / g1).checked_mul(o.0 / g2)?, (self.1 / g2).checked_mul(o.1 / g1)?))
    }
    /// Arrondi au plus proche, la moitié vers le haut (valeurs positives).
    fn arrondi(self) -> Option<i64> {
        Some((2i128.checked_mul(self.0)?.checked_add(self.1)? / 2i128.checked_mul(self.1)?) as i64)
    }
}

type Lignes = std::collections::BTreeMap<String, (Frac, Frac)>;

/// Calcul indépendant : quantité en titres et coût en euros, en fractions exactes.
fn oracle(mvts: &[MouvementTitre]) -> Option<(Lignes, Vec<usize>)> {
    let mut ordre: Vec<usize> = (0..mvts.len()).collect();
    ordre.sort_by_key(|&k| (mvts[k].date, matches!(mvts[k].sens, SensTitre::Vente), k));
    let mut lignes = Lignes::new();
    let mut refus = Vec::new();
    for i in ordre {
        let m = &mvts[i];
        let q = Frac::new(m.quantite_micro as i128, 1_000_000);
        let l = lignes.entry(m.isin.clone()).or_insert((Frac(0, 1), Frac(0, 1)));
        match m.sens {
            SensTitre::Achat => {
                let cout = q
                    .fois(Frac::new(m.prix.micro() as i128, 1_000_000))?
                    .plus(Frac::new(m.frais.cents() as i128, 100))?;
                *l = (l.0.plus(q)?, l.1.plus(cout)?);
            }
            SensTitre::Vente => {
                if q.0.checked_mul(l.0 .1)? > l.0 .0.checked_mul(q.1)? {
                    refus.push(i);
                    continue;
                }
                let reste = l.0.plus(Frac(-q.0, q.1))?;
                // Le PRU ne change pas : coût × reste / détenu.
                let cout = if reste.0 == 0 { Frac(0, 1) } else { l.1.fois(reste)?.fois(Frac::new(l.0 .1, l.0 .0))? };
                *l = (reste, cout);
            }
        }
    }
    Some((lignes, refus))
}

#[test]
fn journal_conforme_au_calcul_exact_sur_400_historiques() {
    let mut a = Alea(0x0BAD_5EED_1234_5678);
    let mut verifies = 0;
    for essai in 0..400 {
        let n = a.entre(1, 30) as usize;
        let mvts: Vec<MouvementTitre> = (0..n)
            .map(|_| MouvementTitre {
                date: Date::depuis_jours_unix(d("2015-01-01").jours_unix() + a.entre(0, 60)),
                isin: a.choix(&ISINS[..2]).to_string(),
                sens: if a.entre(0, 2) == 0 { SensTitre::Vente } else { SensTitre::Achat },
                quantite_micro: if a.entre(0, 3) > 0 { a.entre(1, 20) * 1_000_000 } else { a.entre(1, 3_000_000) },
                prix: Price::from_micro(a.entre(1, 500_000_000)),
                frais: Money::from_cents(a.entre(0, 1_000)),
            })
            .collect();
        let r = rejouer(&mvts);
        let Some((attendu, mut refus)) = oracle(&mvts) else { continue };
        verifies += 1;
        let mut refusees: Vec<usize> = r.erreurs.iter().map(|(i, _)| *i).collect();
        refus.sort();
        refusees.sort();
        assert_eq!(refusees, refus, "essai {essai} : ventes refusées");
        assert!(r.erreurs.iter().all(|(_, e)| matches!(e, ErreurJournal::VenteSuperieure { .. })));
        for (isin, (q, cout)) in &attendu {
            let l = r.lignes[isin];
            assert_eq!(l.quantite_micro as i128 * q.1, q.0 * 1_000_000, "essai {essai} : quantité");
            let pru_attendu = if q.0 == 0 {
                0
            } else {
                match cout.fois(Frac::new(q.1, q.0)).and_then(|f| f.fois(Frac(1_000_000, 1))).and_then(Frac::arrondi) {
                    Some(p) => p,
                    None => continue,
                }
            };
            // Le coût restant après une vente est tronqué à 10⁻¹² € : écart d'au plus un millionième sur le PRU.
            assert!(
                (l.pru().micro() - pru_attendu).abs() <= 1,
                "essai {essai} : PRU {} au lieu de {pru_attendu}",
                l.pru().micro()
            );
            if let Some(c) = cout.fois(Frac(100, 1)).and_then(Frac::arrondi) {
                assert!((l.cout().cents() - c).abs() <= 1, "essai {essai} : coût");
            }
        }
    }
    assert!(verifies >= 300, "seulement {verifies} historiques vérifiables");
}

/// Le journal et la saisie manuelle donnent le même PRU : achats seuls, PRU frais compris.
#[test]
fn pru_identique_a_une_saisie_manuelle() {
    // 3 achats : 10 × 52,30 € (1,99 € de frais), 5 × 48,10 € (1,99 €), 2,5 × 50 € (0 €).
    let m = |q: i64, p: i64, f: i64| MouvementTitre {
        date: d("2025-01-02"),
        isin: ISINS[0].into(),
        sens: SensTitre::Achat,
        quantite_micro: q,
        prix: Price::from_micro(p),
        frais: Money::from_cents(f),
    };
    let r = rejouer(&[m(10_000_000, 52_300_000, 199), m(5_000_000, 48_100_000, 199), m(2_500_000, 50_000_000, 0)]);
    let l = r.lignes[ISINS[0]];
    // (523 + 1,99 + 240,50 + 1,99 + 125) / 17,5 = 892,48 / 17,5 = 50,998857…
    assert_eq!(l.quantite_micro, 17_500_000);
    assert_eq!(l.pru(), Price::from_micro(50_998_857));
    assert_eq!(l.cout(), Money::from_cents(89_248));
}
