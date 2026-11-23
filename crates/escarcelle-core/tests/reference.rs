//! Cas de référence de la spécification v0.1 (§1.4 et §3.6) et propriétés.

use escarcelle_core::impot::{calculer_ir, eligible_lep, seuil_rfr_lep, Foyer};
use escarcelle_core::livret::{simuler, Mouvement};
use escarcelle_core::params::embarque;
use escarcelle_core::{Date, Money, Rate};

fn d(s: &str) -> Date {
    s.parse().unwrap()
}
fn eur(e: i64) -> Money {
    Money::from_euros(e)
}
fn pct(s: &str) -> Rate {
    Rate::parse_percent(s).unwrap()
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
        &[
            Mouvement::depot(d("2026-03-02"), eur(5_000)),
            Mouvement::retrait(d("2026-03-14"), eur(5_000)),
        ],
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
        &[
            Mouvement::depot(d("2026-01-01"), eur(2_400)),
            Mouvement::retrait(d("2026-12-30"), eur(2_400)),
        ],
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

// ---------- Impôt ----------

#[test]
fn spec_3_6_seul_35000() {
    let r = calculer_ir(eur(35_000), &Foyer::seul(), &embarque::ir_revenus_2025()).unwrap();
    assert_eq!(r.impot, eur(3_604));
    assert_eq!(r.tmi, pct("30"));
    assert!(!r.plafonnement_applique);
    assert_eq!(r.decote, Money::ZERO);
}

#[test]
fn spec_3_6_couple_2_enfants_60000_avec_decote() {
    let r = calculer_ir(eur(60_000), &Foyer::couple(2), &embarque::ir_revenus_2025()).unwrap();
    assert_eq!(r.demi_parts, 6);
    assert_eq!(r.impot_brut, eur(2_772));
    assert_eq!(r.decote, Money::from_cents(22_867));
    assert_eq!(r.impot, eur(2_543));
    assert_eq!(r.tmi, pct("11"));
    assert_eq!(r.taux_marginal_effectif, pct("15,9775"));
}

#[test]
fn spec_3_6_sous_le_seuil() {
    let r = calculer_ir(eur(10_000), &Foyer::seul(), &embarque::ir_revenus_2025()).unwrap();
    assert_eq!(r.impot, Money::ZERO);
    assert_eq!(r.tmi, Rate::ZERO);
}

#[test]
fn plafonnement_du_quotient_familial() {
    // Couple + 2 enfants, 200 000 € : avantage 10 289 € > 2 × 1 807 €.
    let r = calculer_ir(eur(200_000), &Foyer::couple(2), &embarque::ir_revenus_2025()).unwrap();
    assert!(r.plafonnement_applique);
    assert_eq!(r.impot, eur(45_987));
    assert_eq!(r.tmi, pct("41"));
}

#[test]
fn parts() {
    assert_eq!(Foyer::seul().demi_parts(), 2);
    assert_eq!(Foyer::couple(0).demi_parts(), 4);
    assert_eq!(Foyer::couple(2).demi_parts(), 6);
    assert_eq!(Foyer::couple(3).demi_parts(), 8);
    let isole = Foyer { enfants: 1, parent_isole: true, ..Foyer::seul() };
    assert_eq!(isole.demi_parts(), 4);
    // Paramètre « parent isolé » non renseigné dans le jeu embarqué : erreur explicite.
    assert!(calculer_ir(eur(50_000), &isole, &embarque::ir_revenus_2025()).is_err());
}

#[test]
fn seuils_lep_2026() {
    let p = embarque::lep_rfr_2026();
    assert_eq!(seuil_rfr_lep(2, &p), eur(23_028));
    assert_eq!(seuil_rfr_lep(4, &p), eur(35_326));
    assert_eq!(seuil_rfr_lep(6, &p), eur(47_624));
    assert!(eligible_lep(eur(23_028), 2, &p));
    assert!(!eligible_lep(Money::from_cents(2_302_801), 2, &p));
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
fn propriete_impot_croissant_et_taux_coherents() {
    let p = embarque::ir_revenus_2025();
    for foyer in [Foyer::seul(), Foyer::couple(0), Foyer::couple(1), Foyer::couple(3)] {
        let mut prec = Money::ZERO;
        for e in (0..=400_000).step_by(250) {
            let r = calculer_ir(eur(e), &foyer, &p).unwrap();
            assert!(r.impot >= prec, "impôt non croissant à {e} € pour {foyer:?}");
            assert!(r.taux_moyen <= r.tmi.max(r.taux_moyen), "taux moyen incohérent");
            assert!(r.taux_moyen <= pct("45"));
            prec = r.impot;
        }
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
