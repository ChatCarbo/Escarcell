//! Compare les 5 PER de la V1 : `cargo run -p escarcelle-core --example comparer_per`
use escarcelle_core::comparateur::{comparer, Contrat, Hypotheses};
use escarcelle_core::{Money, Rate};

fn pct(s: &str) -> Rate {
    Rate::parse_percent(s).unwrap()
}

fn contrat(nom: &str, uc: &str, fe: &str, transaction: &str, max_fe: Option<&str>) -> Contrat {
    Contrat {
        nom: nom.into(),
        frais_versement: Rate::ZERO,
        frais_gestion_uc: pct(uc),
        frais_transaction_uc: pct(transaction),
        rendement_fonds_euros: pct(fe),
        part_fonds_euros_max: max_fe.map(pct),
    }
}

fn main() {
    let contrats = [
        contrat("Fortuneo PER", "0,5", "3,2", "0", None),
        contrat("BoursoBank PERin Matla", "0,5", "3,5", "0", Some("50")),
        contrat("Linxea Spirit PER", "0,5", "3,08", "0", None),
        contrat("Linxea Suravenir PER", "0,6", "3", "0", None),
        contrat("Linxea Apicil PER", "0,6", "1,45", "0,15", None),
    ];
    for part_fe in ["0", "30", "70"] {
        let h = Hypotheses {
            versement_initial: Money::from_euros(5_000),
            versement_mensuel: Money::from_euros(200),
            annees: 20,
            part_fonds_euros: pct(part_fe),
            rendement_brut_uc: pct("6"),
            ter_uc: pct("0,2"),
        };
        println!("\n20 ans, 5 000 € + 200 €/mois, {part_fe} % fonds euros, UC à 6 % brut, TER 0,2 %");
        for r in comparer(&contrats, &h) {
            let note = if r.plafonnee { format!(" ← non comparable : fonds euros plafonnés à {}", r.part_fonds_euros) } else { String::new() };
            println!("  {:<24} {:>14}  frais {:>12}  écart {:>12}{note}", r.nom, r.capital_final.to_string(), r.frais_totaux.to_string(), r.ecart_avec_meilleur.to_string());
        }
    }
}
