//! Analyse technique d'un historique de cours : moyennes mobiles, bandes de Bollinger, RSI, MACD,
//! supports et résistances, et signaux de rebond ou de cassure.
//!
//! Ces calculs sont statistiques (pas comptables) : ils utilisent des `f64`.
//! L'analyse technique décrit le passé ; elle ne prédit pas l'avenir de façon fiable.
//! Les signaux sont des indications à confronter à d'autres informations, jamais un conseil d'achat ou de vente.

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bougie {
    /// Secondes Unix.
    pub t: i64,
    pub ouverture: f64,
    pub haut: f64,
    pub bas: f64,
    pub cloture: f64,
    pub volume: f64,
}

/// Moyenne mobile simple.
pub fn sma(v: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut res = vec![None; v.len()];
    if n == 0 || v.len() < n {
        return res;
    }
    let mut somme: f64 = v[..n].iter().sum();
    res[n - 1] = Some(somme / n as f64);
    for i in n..v.len() {
        somme += v[i] - v[i - n];
        res[i] = Some(somme / n as f64);
    }
    res
}

/// Moyenne mobile exponentielle, initialisée par la moyenne simple des `n` premières valeurs.
pub fn ema(v: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut res = vec![None; v.len()];
    if n == 0 || v.len() < n {
        return res;
    }
    let k = 2.0 / (n as f64 + 1.0);
    let mut e = v[..n].iter().sum::<f64>() / n as f64;
    res[n - 1] = Some(e);
    for i in n..v.len() {
        e = v[i] * k + e * (1.0 - k);
        res[i] = Some(e);
    }
    res
}

/// RSI de Wilder (0 à 100).
pub fn rsi(v: &[f64], n: usize) -> Vec<Option<f64>> {
    let mut res = vec![None; v.len()];
    if n == 0 || v.len() <= n {
        return res;
    }
    let (mut hausse, mut baisse) = (0.0, 0.0);
    for i in 1..=n {
        let d = v[i] - v[i - 1];
        if d > 0.0 { hausse += d } else { baisse -= d }
    }
    hausse /= n as f64;
    baisse /= n as f64;
    let calc = |h: f64, b: f64| if b == 0.0 { 100.0 } else { 100.0 - 100.0 / (1.0 + h / b) };
    res[n] = Some(calc(hausse, baisse));
    for i in n + 1..v.len() {
        let d = v[i] - v[i - 1];
        hausse = (hausse * (n as f64 - 1.0) + d.max(0.0)) / n as f64;
        baisse = (baisse * (n as f64 - 1.0) + (-d).max(0.0)) / n as f64;
        res[i] = Some(calc(hausse, baisse));
    }
    res
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bande {
    pub bas: f64,
    pub milieu: f64,
    pub haut: f64,
}

/// Bandes de Bollinger (moyenne `n`, ± `k` écarts-types).
pub fn bollinger(v: &[f64], n: usize, k: f64) -> Vec<Option<Bande>> {
    let m = sma(v, n);
    (0..v.len())
        .map(|i| {
            m[i].map(|milieu| {
                let fenetre = &v[i + 1 - n..=i];
                let var = fenetre.iter().map(|x| (x - milieu).powi(2)).sum::<f64>() / n as f64;
                let e = var.sqrt();
                Bande { bas: milieu - k * e, milieu, haut: milieu + k * e }
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Macd {
    pub macd: f64,
    pub signal: f64,
    pub histogramme: f64,
}

/// MACD (12, 26, 9 par défaut).
pub fn macd(v: &[f64], court: usize, long: usize, signal: usize) -> Vec<Option<Macd>> {
    let (ec, el) = (ema(v, court), ema(v, long));
    let ligne: Vec<Option<f64>> = ec.iter().zip(&el).map(|(a, b)| Some((*a)? - (*b)?)).collect();
    let debut = ligne.iter().position(Option::is_some);
    let mut res = vec![None; v.len()];
    if let Some(d) = debut {
        let valeurs: Vec<f64> = ligne[d..].iter().map(|x| x.unwrap_or(0.0)).collect();
        let s = ema(&valeurs, signal);
        for (j, sig) in s.into_iter().enumerate() {
            if let (Some(sig), Some(m)) = (sig, ligne[d + j]) {
                res[d + j] = Some(Macd { macd: m, signal: sig, histogramme: m - sig });
            }
        }
    }
    res
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeNiveau {
    Support,
    Resistance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Niveau {
    pub prix: f64,
    /// Nombre de fois où le cours a buté sur ce niveau.
    pub touches: usize,
    pub type_: TypeNiveau,
}

/// Supports et résistances : sommets et creux locaux (± `fenetre` séances), regroupés quand ils sont
/// à moins de `tolerance` (relative) les uns des autres ; seuls les niveaux touchés au moins deux fois
/// sont gardés. Renvoie au plus 3 supports (sous le dernier cours) et 3 résistances (au-dessus), les plus proches.
pub fn niveaux(b: &[Bougie], fenetre: usize, tolerance: f64) -> Vec<Niveau> {
    if b.len() < 2 * fenetre + 1 {
        return Vec::new();
    }
    let mut extremes = Vec::new();
    for i in fenetre..b.len() - fenetre {
        let zone = &b[i - fenetre..=i + fenetre];
        if zone.iter().all(|x| x.haut <= b[i].haut) {
            extremes.push(b[i].haut);
        }
        if zone.iter().all(|x| x.bas >= b[i].bas) {
            extremes.push(b[i].bas);
        }
    }
    extremes.sort_by(|a, c| a.total_cmp(c));
    let mut groupes: Vec<(f64, usize)> = Vec::new(); // (somme, nombre)
    for x in extremes {
        match groupes.last_mut() {
            Some((s, n)) if (x - *s / *n as f64).abs() / x <= tolerance => {
                *s += x;
                *n += 1;
            }
            _ => groupes.push((x, 1)),
        }
    }
    let dernier = b.last().map(|x| x.cloture).unwrap_or(0.0);
    let mut supports: Vec<Niveau> = Vec::new();
    let mut resistances: Vec<Niveau> = Vec::new();
    for (s, n) in groupes.into_iter().filter(|(_, n)| *n >= 2) {
        let prix = s / n as f64;
        if prix < dernier {
            supports.push(Niveau { prix, touches: n, type_: TypeNiveau::Support });
        } else {
            resistances.push(Niveau { prix, touches: n, type_: TypeNiveau::Resistance });
        }
    }
    supports.sort_by(|a, c| c.prix.total_cmp(&a.prix));
    resistances.sort_by(|a, c| a.prix.total_cmp(&c.prix));
    supports.truncate(3);
    resistances.truncate(3);
    supports.extend(resistances);
    supports
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tendance {
    Haussier,
    Baissier,
    Neutre,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Signal {
    pub nom: &'static str,
    pub sens: Tendance,
    pub explication: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Analyse {
    pub sma20: Vec<Option<f64>>,
    pub sma50: Vec<Option<f64>>,
    pub sma200: Vec<Option<f64>>,
    pub bollinger: Vec<Option<Bande>>,
    pub rsi: Vec<Option<f64>>,
    pub macd: Vec<Option<Macd>>,
    pub niveaux: Vec<Niveau>,
    pub signaux: Vec<Signal>,
    pub synthese: Tendance,
}

fn pct(x: f64) -> String {
    format!("{:.1} %", x * 100.0).replace('.', ",")
}

fn prix(x: f64) -> String {
    format!("{x:.2}").replace('.', ",")
}

/// Calcule tous les indicateurs et en déduit des signaux lisibles.
pub fn analyser(b: &[Bougie]) -> Analyse {
    let c: Vec<f64> = b.iter().map(|x| x.cloture).collect();
    let (sma20, sma50, sma200) = (sma(&c, 20), sma(&c, 50), sma(&c, 200));
    let bol = bollinger(&c, 20, 2.0);
    let r = rsi(&c, 14);
    let m = macd(&c, 12, 26, 9);
    let niv = niveaux(b, 5, 0.02);
    let mut s = Vec::new();
    let n = c.len();
    if n < 30 {
        return Analyse { sma20, sma50, sma200, bollinger: bol, rsi: r, macd: m, niveaux: niv, signaux: s, synthese: Tendance::Neutre };
    }
    let dernier = c[n - 1];

    // Tendance de fond.
    if let (Some(m50), Some(m200)) = (sma50[n - 1], sma200[n - 1]) {
        if dernier > m200 && m50 > m200 {
            s.push(Signal { nom: "Tendance de fond", sens: Tendance::Haussier, explication: format!("Le cours ({}) est au-dessus de sa moyenne 200 jours ({}) et la moyenne 50 jours est au-dessus de la 200 jours.", prix(dernier), prix(m200)) });
        } else if dernier < m200 && m50 < m200 {
            s.push(Signal { nom: "Tendance de fond", sens: Tendance::Baissier, explication: format!("Le cours ({}) est sous sa moyenne 200 jours ({}) et la moyenne 50 jours est sous la 200 jours.", prix(dernier), prix(m200)) });
        }
        // Croisement des moyennes 50 / 200 sur les 10 dernières séances.
        for i in n.saturating_sub(10).max(1)..n {
            if let (Some(a0), Some(b0), Some(a1), Some(b1)) = (sma50[i - 1], sma200[i - 1], sma50[i], sma200[i]) {
                if a0 <= b0 && a1 > b1 {
                    s.push(Signal { nom: "Croisement doré", sens: Tendance::Haussier, explication: "La moyenne 50 jours vient de passer au-dessus de la moyenne 200 jours, signal haussier classique.".into() });
                } else if a0 >= b0 && a1 < b1 {
                    s.push(Signal { nom: "Croisement de la mort", sens: Tendance::Baissier, explication: "La moyenne 50 jours vient de passer sous la moyenne 200 jours, signal baissier classique.".into() });
                }
            }
        }
    }

    // RSI.
    if let Some(v) = r[n - 1] {
        if v >= 70.0 {
            s.push(Signal { nom: "RSI en surachat", sens: Tendance::Baissier, explication: format!("RSI à {v:.0} : le titre a beaucoup monté récemment, un repli ou une pause est fréquent.") });
        } else if v <= 30.0 {
            s.push(Signal { nom: "RSI en survente", sens: Tendance::Haussier, explication: format!("RSI à {v:.0} : le titre a beaucoup baissé récemment, un rebond technique est fréquent.") });
        }
    }

    // Bollinger : excès et compression.
    if let Some(bd) = bol[n - 1] {
        if dernier < bd.bas {
            s.push(Signal { nom: "Sous la bande de Bollinger", sens: Tendance::Haussier, explication: "Le cours est passé sous la bande basse : excès de baisse, rebond possible vers la moyenne 20 jours.".into() });
        } else if dernier > bd.haut {
            s.push(Signal { nom: "Au-dessus de la bande de Bollinger", sens: Tendance::Baissier, explication: "Le cours est passé au-dessus de la bande haute : excès de hausse, retour possible vers la moyenne 20 jours.".into() });
        }
        let largeurs: Vec<f64> = bol[n.saturating_sub(120)..].iter().flatten().map(|x| (x.haut - x.bas) / x.milieu).collect();
        let actuelle = (bd.haut - bd.bas) / bd.milieu;
        if largeurs.len() >= 60 && largeurs.iter().all(|l| *l >= actuelle) {
            s.push(Signal { nom: "Compression des bandes", sens: Tendance::Neutre, explication: "Les bandes de Bollinger n'ont jamais été aussi resserrées depuis six mois : un mouvement marqué est probable, sans indication de sens.".into() });
        }
    }

    // MACD : croisement de la ligne de signal sur les 5 dernières séances.
    for i in n.saturating_sub(5).max(1)..n {
        if let (Some(a), Some(z)) = (m[i - 1], m[i]) {
            if a.histogramme <= 0.0 && z.histogramme > 0.0 {
                s.push(Signal { nom: "MACD haussier", sens: Tendance::Haussier, explication: "Le MACD vient de croiser sa ligne de signal à la hausse : l'élan redevient positif.".into() });
            } else if a.histogramme >= 0.0 && z.histogramme < 0.0 {
                s.push(Signal { nom: "MACD baissier", sens: Tendance::Baissier, explication: "Le MACD vient de croiser sa ligne de signal à la baisse : l'élan devient négatif.".into() });
            }
        }
    }

    // Cassures et rebonds sur les niveaux calculés avant les 5 dernières séances.
    let avant = &b[..n - 5];
    let niveaux_avant = niveaux(avant, 5, 0.02);
    let vol_moyen = b[n.saturating_sub(25)..n - 5].iter().map(|x| x.volume).sum::<f64>() / 20.0;
    let vol_recent = b[n - 5..].iter().map(|x| x.volume).sum::<f64>() / 5.0;
    let confirme = vol_moyen > 0.0 && vol_recent > 1.5 * vol_moyen;
    let precedent = c[n - 6];
    let volumes = |ok: &str, ko: &str| if confirme { ok.to_string() } else { ko.to_string() };
    // Une seule cassure par sens : le niveau franchi le plus proche du cours actuel.
    let resistance_franchie = niveaux_avant
        .iter()
        .filter(|nv| nv.type_ == TypeNiveau::Resistance && precedent < nv.prix && dernier > nv.prix * 1.01)
        .max_by(|a, b| a.prix.total_cmp(&b.prix));
    if let Some(nv) = resistance_franchie {
        s.push(Signal {
            nom: "Cassure de résistance",
            sens: Tendance::Haussier,
            explication: format!(
                "Le cours a franchi la résistance des {} (touchée {} fois){}. Cet ancien plafond peut désormais servir de support.",
                prix(nv.prix),
                nv.touches,
                volumes(" avec des volumes en hausse, ce qui renforce le signal", ", mais sans hausse des volumes : à confirmer")
            ),
        });
    }
    let support_casse = niveaux_avant
        .iter()
        .filter(|nv| nv.type_ == TypeNiveau::Support && precedent > nv.prix && dernier < nv.prix * 0.99)
        .min_by(|a, b| a.prix.total_cmp(&b.prix));
    if let Some(nv) = support_casse {
        s.push(Signal {
            nom: "Cassure de support",
            sens: Tendance::Baissier,
            explication: format!(
                "Le cours est passé sous le support des {} (touché {} fois){}. Cet ancien plancher peut désormais freiner une remontée.",
                prix(nv.prix),
                nv.touches,
                volumes(" avec des volumes en hausse, ce qui renforce le signal", ", sans hausse des volumes : à confirmer")
            ),
        });
    }
    if let Some(sup) = niv.iter().find(|x| x.type_ == TypeNiveau::Support) {
        let ecart = (dernier - sup.prix) / sup.prix;
        let rsi_bas = r[n - 1].is_some_and(|v| v < 45.0);
        if ecart <= 0.03 && rsi_bas && c[n - 1] > c[n - 2] {
            s.push(Signal {
                nom: "Rebond possible sur support",
                sens: Tendance::Haussier,
                explication: format!("Le cours est à {} au-dessus du support des {} (touché {} fois) et repart à la hausse après une baisse.", pct(ecart), prix(sup.prix), sup.touches),
            });
        }
    }

    let haussiers = s.iter().filter(|x| x.sens == Tendance::Haussier).count();
    let baissiers = s.iter().filter(|x| x.sens == Tendance::Baissier).count();
    let synthese = if haussiers >= baissiers + 2 {
        Tendance::Haussier
    } else if baissiers >= haussiers + 2 {
        Tendance::Baissier
    } else {
        Tendance::Neutre
    };
    Analyse { sma20, sma50, sma200, bollinger: bol, rsi: r, macd: m, niveaux: niv, signaux: s, synthese }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serie(c: &[f64]) -> Vec<Bougie> {
        c.iter()
            .enumerate()
            .map(|(i, &x)| Bougie { t: i as i64 * 86_400, ouverture: x, haut: x * 1.01, bas: x * 0.99, cloture: x, volume: 1_000.0 })
            .collect()
    }

    #[test]
    fn moyennes() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(sma(&v, 3), vec![None, None, Some(2.0), Some(3.0), Some(4.0)]);
        let e = ema(&v, 3);
        assert_eq!(e[2], Some(2.0));
        assert!((e[4].unwrap() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rsi_extremes() {
        let hausse: Vec<f64> = (1..=30).map(f64::from).collect();
        assert_eq!(rsi(&hausse, 14)[29], Some(100.0));
        let baisse: Vec<f64> = (1..=30).rev().map(f64::from).collect();
        assert!(rsi(&baisse, 14)[29].unwrap() < 1.0);
    }

    #[test]
    fn bollinger_serie_constante() {
        let v = vec![10.0; 25];
        let b = bollinger(&v, 20, 2.0)[24].unwrap();
        assert_eq!((b.bas, b.milieu, b.haut), (10.0, 10.0, 10.0));
    }

    #[test]
    fn macd_positif_en_tendance_haussiere() {
        let v: Vec<f64> = (0..60).map(|i| 100.0 + i as f64).collect();
        let m = macd(&v, 12, 26, 9);
        assert!(m[59].unwrap().macd > 0.0);
        assert!(m[32].is_none() || m[33].is_some());
    }

    #[test]
    fn support_et_resistance_d_un_range() {
        // Oscillation entre 90 et 110 : un support vers 90, une résistance vers 110.
        let c: Vec<f64> = (0..120).map(|i| 100.0 + 10.0 * ((i as f64) * std::f64::consts::PI / 10.0).sin()).collect();
        let nv = niveaux(&serie(&c), 5, 0.02);
        assert!(nv.iter().any(|n| n.type_ == TypeNiveau::Support && (n.prix - 89.1).abs() < 1.5));
        assert!(nv.iter().any(|n| n.type_ == TypeNiveau::Resistance && (n.prix - 111.1).abs() < 1.5));
    }

    #[test]
    fn cassure_de_resistance_detectee() {
        // Range 90–110 pendant 120 séances, puis sortie franche à 118 avec volumes doublés.
        let mut c: Vec<f64> = (0..120).map(|i| 100.0 + 10.0 * ((i as f64) * std::f64::consts::PI / 10.0).sin()).collect();
        let base = c.len();
        c.extend([104.0, 108.0, 112.0, 116.0, 118.0]);
        let mut b = serie(&c);
        for x in &mut b[base..] {
            x.volume = 3_000.0;
        }
        let a = analyser(&b);
        let cassure = a.signaux.iter().find(|s| s.nom == "Cassure de résistance").expect("cassure attendue");
        assert!(cassure.explication.contains("volumes en hausse"));
    }

    #[test]
    fn tendance_haussiere_de_fond() {
        let c: Vec<f64> = (0..260).map(|i| 50.0 + i as f64 * 0.2).collect();
        let a = analyser(&serie(&c));
        assert!(a.signaux.iter().any(|s| s.nom == "Tendance de fond" && s.sens == Tendance::Haussier));
    }
}
