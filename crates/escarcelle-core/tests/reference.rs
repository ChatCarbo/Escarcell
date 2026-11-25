//! Cas de référence des livrets (règles des quinzaines, changements de taux, plafonds) et propriétés.
//!
//! Source des règles de dates : service-public.gouv.fr, fiche F2365 « Livret A » (consultée le 24/11/2026),
//! https://www.service-public.gouv.fr/particuliers/vosdroits/F2365 :
//! « Les intérêts sont calculés le 1er et le 16 de chaque mois » ; dépôt jusqu'au 15 : pris en compte au 16 ;
//! dépôt à partir du 16 : au 1er du mois suivant ; retrait jusqu'au 15 : solde arrêté au dernier jour du mois
//! précédent ; retrait à partir du 16 : solde arrêté au 15 ; intérêts ajoutés au capital au 31 décembre.

use escarcelle_core::conseil_livret::{conseil_retrait, conseil_versement};
use escarcelle_core::livret::{simuler, Mouvement};
use escarcelle_core::params::embarque;
use escarcelle_core::{Date, Money};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}
fn eur(e: i64) -> Money {
    Money::from_euros(e)
}

// ---------- Exemples de dates de la fiche F2365 (service-public.gouv.fr) ----------

#[test]
fn f2365_depot_entre_le_1er_et_le_15_mai_pris_en_compte_le_16_mai() {
    for j in 1..=15 {
        let c = conseil_versement(Date::new(2026, 5, j).unwrap());
        assert_eq!(c.remunere_a_partir_du, d("2026-05-16"), "dépôt le {j} mai");
    }
}

#[test]
fn f2365_depot_entre_le_16_et_le_31_mai_pris_en_compte_le_1er_juin() {
    for j in 16..=31 {
        let c = conseil_versement(Date::new(2026, 5, j).unwrap());
        assert_eq!(c.remunere_a_partir_du, d("2026-06-01"), "dépôt le {j} mai");
    }
}

#[test]
fn f2365_retrait_entre_le_1er_et_le_15_mai_solde_arrete_au_30_avril() {
    for j in 1..=15 {
        let c = conseil_retrait(
            Date::new(2026, 5, j).unwrap(),
            eur(1_000),
            embarque::livret_a().taux.at(d("2026-05-01")).copied().unwrap(),
        );
        // Le montant retiré ne rapporte plus à partir du 1er mai : le solde rémunéré est celui du 30 avril.
        assert_eq!(c.non_remunere_depuis, d("2026-05-01"), "retrait le {j} mai");
    }
}

#[test]
fn f2365_retrait_entre_le_16_et_le_31_mai_solde_arrete_au_15_mai() {
    for j in 16..=31 {
        let c = conseil_retrait(
            Date::new(2026, 5, j).unwrap(),
            eur(1_000),
            embarque::livret_a().taux.at(d("2026-05-01")).copied().unwrap(),
        );
        assert_eq!(c.non_remunere_depuis, d("2026-05-16"), "retrait le {j} mai");
    }
}

#[test]
fn f2365_interets_ajoutes_au_capital_au_31_decembre() {
    // 1 000 € versés le 1er janvier 2027 (rémunérés à partir du 16 janvier) : les intérêts de 2027
    // ne sont ajoutés au solde qu'en fin d'année, puis rapportent à leur tour en 2028.
    let r = simuler(&[Mouvement::depot(d("2027-01-01"), eur(1_000))], &embarque::livret_a().taux, 2028).unwrap();
    assert_eq!(r[0].annee, 2027);
    assert!(r[0].interets > Money::ZERO);
    assert_eq!(r[0].solde_fin, eur(1_000) + r[0].interets);
    assert!(r[1].interets > r[0].interets, "les intérêts capitalisés rapportent l'année suivante");
}

// ---------- Livrets ----------

#[test]
fn spec_1_4_livret_a_changement_de_taux() {
    // 1 000 € le 10/03/2026 : effet 16/03 ; 9 quinzaines à 1,50 %, 10 à 1,70 %.
    let r = simuler(&[Mouvement::depot(d("2026-03-10"), eur(1_000))], &embarque::livret_a().taux, 2026).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].interets, Money::from_cents(1271));
    assert_eq!(r[0].solde_fin, Money::from_cents(101_271));
}

#[test]
fn depot_fin_decembre_compte_l_annee_suivante() {
    let r = simuler(&[Mouvement::depot(d("2025-12-20"), eur(1_000))], &embarque::livret_a().taux, 2026).unwrap();
    assert_eq!(r[0].annee, 2026); // aucune quinzaine rémunérée en 2025
                                  // 2026 : 2 quinzaines à 1,70 %, 12 à 1,50 %, 10 à 1,70 % → 1000 × 0,384 / 24 = 16,00
    assert_eq!(r[0].interets, Money::from_cents(1600));
}

#[test]
fn aller_retour_dans_la_quinzaine_ne_rapporte_rien() {
    let r = simuler(
        &[Mouvement::depot(d("2026-03-02"), eur(5_000)), Mouvement::retrait(d("2026-03-14"), eur(5_000))],
        &embarque::livret_a().taux,
        2026,
    )
    .unwrap();
    assert_eq!(r[0].interets, Money::ZERO);
}

#[test]
fn retrait_compte_des_le_debut_de_la_quinzaine() {
    // Dépôt le 01/01 (effet q2), retrait le 30/12 (effet q24) : 22 quinzaines rémunérées.
    let r = simuler(
        &[Mouvement::depot(d("2026-01-01"), eur(2_400)), Mouvement::retrait(d("2026-12-30"), eur(2_400))],
        &embarque::lep().taux,
        2026,
    )
    .unwrap();
    // q2 à 2,70 %, q3..q23 (21) à 2,50 % → 2400 × (0,027 + 0,525) / 24 = 55,20
    assert_eq!(r[0].interets, Money::from_cents(5520));
}

#[test]
fn capitalisation_et_interets_au_dela_du_plafond() {
    let lep = embarque::lep();
    let r = simuler(&[Mouvement::depot(d("2025-07-20"), eur(10_000))], &lep.taux, 2026).unwrap();
    // 2025 : 10 quinzaines à 2,70 %
    assert_eq!(r[0].interets, Money::from_cents(11_250));
    // 2026 : 10 112,50 € rémunérés, dont les intérêts au-delà du plafond
    assert_eq!(r[1].interets, Money::from_cents(25_450));
    assert!(r[1].solde_fin > lep.plafond);
}

#[test]
fn taux_manquant_signale() {
    let r = simuler(&[Mouvement::depot(d("2020-01-01"), eur(100))], &embarque::livret_a().taux, 2020);
    assert!(r.is_err());
}

// ---------- Propriétés (générateur déterministe, sans dépendance) ----------

struct Lcg(u64);
impl Lcg {
    fn next(&mut self, max: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % max
    }
}

#[test]
fn propriete_interets_positifs_et_bornes() {
    let taux = embarque::livret_a().taux;
    let mut g = Lcg(42);
    for _ in 0..500 {
        let mut mouvements = Vec::new();
        for _ in 0..g.next(12) + 1 {
            let date = Date::new(2026, g.next(12) as u8 + 1, g.next(28) as u8 + 1).unwrap();
            let montant = Money::from_cents(g.next(500_000) as i64 + 1);
            mouvements.push(if g.next(3) == 0 {
                Mouvement::retrait(date, montant)
            } else {
                Mouvement::depot(date, montant)
            });
        }
        let total_depots: i64 = mouvements
            .iter()
            .filter(|m| m.sens == escarcelle_core::livret::Sens::Depot)
            .map(|m| m.montant.cents())
            .sum();
        let r = simuler(&mouvements, &taux, 2026).unwrap();
        let interets: Money = r.iter().map(|a| a.interets).sum();
        assert!(interets >= Money::ZERO);
        // Majorant : tous les dépôts rémunérés toute l'année au taux maximal de 1,70 %.
        assert!(interets.cents() <= total_depots * 17 / 1000 + 1);
    }
}
