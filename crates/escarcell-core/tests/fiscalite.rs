//! Fiscalité des dividendes dans le PEA, vérifiée contre les sources officielles.
//!
//! Chaque test cite la règle qu'il vérifie. Sources (consultées le 24/11/2026) :
//!
//! - [PEA] BOFiP BOI-RPPM-RCM-40-50-30 : « les crédits d'impôt ne sont restitués que dans l'hypothèse
//!   où les revenus sont eux-mêmes imposables. Tel n'est pas le cas des revenus de titres inscrits sur un PEA. »
//!   https://bofip.impots.gouv.fr/bofip/1560-PGP.html
//! - [PEA-REVENUS] BOFiP BOI-RPPM-RCM-40-50-30 : les produits des placements effectués dans le plan
//!   sont exonérés d'impôt sur le revenu tant qu'ils y sont réinvestis.
//!
//! Les retenues prélevées par chaque pays (26,375 % en Allemagne, 15 % aux Pays-Bas…) sont des valeurs par
//! défaut modifiables par l'utilisateur : elles ne sont pas vérifiées ici, seule la mécanique l'est.

use escarcell_core::dividendes::{dividende_net, pays_isin, retenue_par_defaut, PAYS_CONNUS};
use escarcell_core::{Money, Rate};

fn eur(e: i64) -> Money {
    Money::from_euros(e)
}
fn cents(c: i64) -> Money {
    Money::from_cents(c)
}
fn pct(s: &str) -> Rate {
    Rate::parse_percent(s).unwrap()
}

#[test]
fn pea_aucun_impot_francais_et_retenue_non_restituable() {
    // [PEA] [PEA-REVENUS] Pas d'impôt français dans le plan ; les crédits d'impôt conventionnels ne sont pas
    // restitués : le dividende net est le brut moins la retenue étrangère, rien d'autre.
    for (retenue, net) in [("0", 10_000), ("15", 8_500), ("26,375", 7_362), ("30", 7_000)] {
        let d = dividende_net(eur(100), pct(retenue));
        assert_eq!(d.brut, eur(100));
        assert_eq!(d.net, cents(net), "retenue {retenue} %");
        assert_eq!(d.brut - d.retenue_etrangere, d.net);
    }
}

#[test]
fn dividende_francais_entierement_encaisse() {
    // [PEA] Société française : pas de retenue étrangère, le PEA encaisse le brut.
    assert_eq!(retenue_par_defaut("FR"), Some(Rate::ZERO));
    let d = dividende_net(eur(1_234), retenue_par_defaut("FR").unwrap());
    assert_eq!(d.net, eur(1_234));
}

#[test]
fn pays_de_la_societe_d_apres_l_isin() {
    // Norme ISO 6166 : les deux premières lettres de l'ISIN désignent le pays de l'émetteur.
    assert_eq!(pays_isin("FR0000121014"), Some("FR"));
    assert_eq!(pays_isin("DE0007164600"), Some("DE"));
    assert_eq!(pays_isin("NL0000235190"), Some("NL"));
}

#[test]
fn chaque_pays_propose_a_une_retenue_entre_0_et_35_pour_cent() {
    for (code, nom) in PAYS_CONNUS {
        let r = retenue_par_defaut(code).unwrap_or_else(|| panic!("retenue manquante : {nom}"));
        assert!((0..=350_000).contains(&r.micro()), "{nom} : {}", r.micro());
    }
}
