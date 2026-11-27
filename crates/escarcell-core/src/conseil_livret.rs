//! Conseils pour les livrets réglementés : quand verser, quand retirer, sur quel livret.
//!
//! Règles des quinzaines (1–15 et 16–fin de mois) :
//! - un versement rapporte à partir de la quinzaine **suivante** : verser n'importe quel jour
//!   d'une quinzaine revient au même ; il faut simplement le faire **avant la fin de la quinzaine**
//!   (le 15 ou le dernier jour du mois) pour ne pas en perdre une ;
//! - un retrait cesse de rapporter dès le **début de la quinzaine en cours** : retirer le 14 ou
//!   le 2 revient au même ; si l'on peut attendre, retirer **le 1er ou le 16** fait gagner la
//!   quinzaine qui se termine.
//!
//! Les dates sont des dates de valeur : un virement initié le 15 peut être daté du jour ouvré suivant.

use crate::date::{jours_dans_mois, Date};
use crate::livret::{debut_quinzaine, quinzaine};
use crate::money::Money;
use crate::rate::{Rate, SCALE};

/// Dernier jour de la quinzaine `q` (15 ou dernier jour du mois).
pub fn fin_quinzaine(annee: i32, q: u8) -> Date {
    let d = debut_quinzaine(annee, q);
    let jour = if q % 2 == 1 { 15 } else { jours_dans_mois(annee, d.month()) };
    Date::new(annee, d.month(), jour).expect("fin de quinzaine valide")
}

fn suivante(annee: i32, q: u8) -> (i32, u8) {
    if q == 24 {
        (annee + 1, 1)
    } else {
        (annee, q + 1)
    }
}

/// Intérêts d'une quinzaine pour un montant.
pub fn interets_quinzaine(montant: Money, taux: Rate) -> Money {
    Money::from_cents(crate::div_round(montant.cents() as i128 * taux.micro() as i128, 24 * SCALE as i128))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConseilVersement {
    /// Quinzaine de la date choisie.
    pub debut_quinzaine: Date,
    pub fin_quinzaine: Date,
    /// Premier jour rémunéré.
    pub remunere_a_partir_du: Date,
    /// Date limite pour verser sans perdre de quinzaine.
    pub verser_au_plus_tard_le: Date,
    /// Jours entre le versement et le premier jour rémunéré (argent « en attente »).
    pub jours_en_attente: i64,
}

pub fn conseil_versement(date: Date) -> ConseilVersement {
    let q = quinzaine(date);
    let fin = fin_quinzaine(date.year(), q);
    let (a, qs) = suivante(date.year(), q);
    let effet = debut_quinzaine(a, qs);
    ConseilVersement {
        debut_quinzaine: debut_quinzaine(date.year(), q),
        fin_quinzaine: fin,
        remunere_a_partir_du: effet,
        verser_au_plus_tard_le: fin,
        jours_en_attente: i64::from(fin.day()) - i64::from(date.day()) + 1,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConseilRetrait {
    pub debut_quinzaine: Date,
    pub fin_quinzaine: Date,
    /// Le montant retiré ne rapporte plus à partir de ce jour (début de la quinzaine en cours).
    pub non_remunere_depuis: Date,
    /// Jours déjà passés dans la quinzaine sans être rémunérés.
    pub jours_perdus: i64,
    /// Date conseillée si le retrait peut attendre : début de la quinzaine suivante.
    pub date_conseillee: Date,
    /// Intérêts gagnés en attendant la date conseillée (0 si la date choisie est déjà un 1er ou un 16).
    pub gain_en_attendant: Money,
}

pub fn conseil_retrait(date: Date, montant: Money, taux: Rate) -> ConseilRetrait {
    let q = quinzaine(date);
    let debut = debut_quinzaine(date.year(), q);
    let (a, qs) = suivante(date.year(), q);
    let au_debut = date == debut;
    ConseilRetrait {
        debut_quinzaine: debut,
        fin_quinzaine: fin_quinzaine(date.year(), q),
        non_remunere_depuis: debut,
        jours_perdus: i64::from(date.day()) - i64::from(debut.day()),
        date_conseillee: if au_debut { date } else { debut_quinzaine(a, qs) },
        gain_en_attendant: if au_debut { Money::ZERO } else { interets_quinzaine(montant, taux) },
    }
}

/// Un livret détenu, pour répartir un versement ou un retrait.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LivretDetenu {
    pub nom: String,
    pub solde: Money,
    pub plafond: Money,
    pub taux: Rate,
}

/// Répartit un versement en remplissant d'abord le livret le mieux rémunéré, dans la limite des plafonds.
/// À taux égal (Livret A et LDDS), le livret au plus grand plafond d'abord : c'est lui qu'il vaut mieux garder plein.
/// Renvoie (montant par livret, reste non plaçable).
pub fn repartir_versement(livrets: &[LivretDetenu], montant: Money) -> (Vec<Money>, Money) {
    let mut ordre: Vec<usize> = (0..livrets.len()).collect();
    ordre.sort_by(|&a, &b| livrets[b].taux.cmp(&livrets[a].taux).then(livrets[b].plafond.cmp(&livrets[a].plafond)));
    let mut parts = vec![Money::ZERO; livrets.len()];
    let mut reste = montant.max(Money::ZERO);
    for i in ordre {
        let place = (livrets[i].plafond - livrets[i].solde).max(Money::ZERO).min(reste);
        parts[i] = place;
        reste -= place;
    }
    (parts, reste)
}

/// Répartit un retrait en puisant d'abord dans le livret le moins rémunéré.
/// À taux égal (Livret A et LDDS), le livret au plus petit plafond d'abord : on garde l'argent dans celui qui
/// peut en recevoir le plus, et c'est le petit livret qu'on reconstituera le plus vite.
/// Renvoie (montant par livret, manque si les soldes ne suffisent pas).
pub fn repartir_retrait(livrets: &[LivretDetenu], montant: Money) -> (Vec<Money>, Money) {
    let mut ordre: Vec<usize> = (0..livrets.len()).collect();
    ordre.sort_by(|&a, &b| livrets[a].taux.cmp(&livrets[b].taux).then(livrets[a].plafond.cmp(&livrets[b].plafond)));
    let mut parts = vec![Money::ZERO; livrets.len()];
    let mut reste = montant.max(Money::ZERO);
    for i in ordre {
        let pris = livrets[i].solde.max(Money::ZERO).min(reste);
        parts[i] = pris;
        reste -= pris;
    }
    (parts, reste)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Date {
        s.parse().unwrap()
    }
    fn eur(e: i64) -> Money {
        Money::from_euros(e)
    }

    #[test]
    fn fins_de_quinzaine() {
        assert_eq!(fin_quinzaine(2026, 3), d("2026-02-15"));
        assert_eq!(fin_quinzaine(2026, 4), d("2026-02-28"));
        assert_eq!(fin_quinzaine(2028, 4), d("2028-02-29"));
        assert_eq!(fin_quinzaine(2026, 24), d("2026-12-31"));
    }

    #[test]
    fn versement_avant_la_fin_de_quinzaine() {
        let c = conseil_versement(d("2026-03-10"));
        assert_eq!(c.remunere_a_partir_du, d("2026-03-16"));
        assert_eq!(c.verser_au_plus_tard_le, d("2026-03-15"));
        assert_eq!(c.jours_en_attente, 6);
        let c = conseil_versement(d("2026-12-20"));
        assert_eq!(c.remunere_a_partir_du, d("2027-01-01"));
        assert_eq!(c.verser_au_plus_tard_le, d("2026-12-31"));
    }

    #[test]
    fn retrait_conseille_au_debut_de_quinzaine() {
        let taux = Rate::from_micro(17_000);
        let c = conseil_retrait(d("2026-03-14"), eur(2_400), taux);
        assert_eq!(c.non_remunere_depuis, d("2026-03-01"));
        assert_eq!(c.jours_perdus, 13);
        assert_eq!(c.date_conseillee, d("2026-03-16"));
        assert_eq!(c.gain_en_attendant, Money::from_cents(170)); // 2 400 × 1,7 % / 24
        let c = conseil_retrait(d("2026-03-16"), eur(2_400), taux);
        assert_eq!(c.gain_en_attendant, Money::ZERO);
        assert_eq!(c.date_conseillee, d("2026-03-16"));
        assert_eq!(conseil_retrait(d("2026-12-20"), eur(100), taux).date_conseillee, d("2027-01-01"));
    }

    fn livrets() -> Vec<LivretDetenu> {
        vec![
            LivretDetenu {
                nom: "Livret A".into(),
                solde: eur(20_000),
                plafond: eur(22_950),
                taux: Rate::from_micro(17_000),
            },
            LivretDetenu { nom: "LEP".into(), solde: eur(8_000), plafond: eur(10_000), taux: Rate::from_micro(25_000) },
        ]
    }

    #[test]
    fn versement_remplit_le_lep_d_abord() {
        let (parts, reste) = repartir_versement(&livrets(), eur(6_000));
        assert_eq!(parts, vec![eur(2_950), eur(2_000)]);
        assert_eq!(reste, eur(1_050));
    }

    #[test]
    fn retrait_puise_dans_le_moins_remunere() {
        let (parts, manque) = repartir_retrait(&livrets(), eur(21_000));
        assert_eq!(parts, vec![eur(20_000), eur(1_000)]);
        assert_eq!(manque, Money::ZERO);
    }

    /// Cas signalé : Livret A et LDDS au même taux (1,70 %). Un retrait se prend sur le LDDS (petit plafond),
    /// un versement va d'abord sur le Livret A (grand plafond), quel que soit l'ordre des livrets.
    #[test]
    fn taux_egaux_departages_par_le_plafond() {
        let a = LivretDetenu {
            nom: "Livret A".into(),
            solde: eur(22_950),
            plafond: eur(22_950),
            taux: Rate::from_micro(17_000),
        };
        let ldds = LivretDetenu {
            nom: "LDDS".into(),
            solde: eur(7_000),
            plafond: eur(12_000),
            taux: Rate::from_micro(17_000),
        };
        for l in [vec![a.clone(), ldds.clone()], vec![ldds.clone(), a.clone()]] {
            let (parts, _) = repartir_retrait(&l, eur(500));
            let i = l.iter().position(|x| x.nom == "LDDS").unwrap();
            assert_eq!(parts[i], eur(500), "retrait sur le LDDS");
        }
        let mut vide_a = a.clone();
        vide_a.solde = eur(20_000);
        for l in [vec![vide_a.clone(), ldds.clone()], vec![ldds.clone(), vide_a.clone()]] {
            let (parts, _) = repartir_versement(&l, eur(1_000));
            let i = l.iter().position(|x| x.nom == "Livret A").unwrap();
            assert_eq!(parts[i], eur(1_000), "versement sur le Livret A");
        }
    }
}
