//! Alertes (détachements de dividendes, assemblées générales) et vérification des nouvelles versions.

use escarcelle_core::Date;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::store::Evenement;
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Horizon des alertes, en jours.
const HORIZON: i64 = 45;

#[derive(Serialize)]
pub struct AlerteDto {
    pub date: String,
    /// "dividende" ou "ag" (ou "autre")
    #[serde(rename = "type")]
    pub type_: String,
    pub titre: String,
    pub enveloppe: String,
    pub message: String,
    /// Événement saisi par l'utilisateur (supprimable), sinon calculé.
    pub evenement_id: Option<i64>,
}

fn date_fr(d: Date) -> String {
    format!("{:02}/{:02}/{}", d.day(), d.month(), d.year())
}

/// Alertes des 45 prochains jours, triées par date.
#[tauri::command]
pub fn alertes(etat: State<'_, Etat>) -> Resultat<Vec<AlerteDto>> {
    calculer(&etat)
}

pub fn calculer(etat: &Etat) -> Resultat<Vec<AlerteDto>> {
    let jour = crate::commands::aujourd_hui();
    let limite = Date::depuis_jours_unix(jour.jours_unix() + HORIZON);
    let mut res = Vec::new();
    let mut noms = std::collections::HashMap::new();

    for env in ["PEA", "CTO"] {
        let p = crate::portefeuille::portefeuille(etat, env)?;
        for l in p.lignes {
            noms.insert(l.symbole.clone(), (l.nom.clone(), env));
            let Some(d) = l.div_prochain_estime.as_deref().and_then(|d| d.parse::<Date>().ok()) else { continue };
            if d < jour || d > limite {
                continue;
            }
            let montant = l.div_dernier_micro.unwrap_or(0) as f64 / 1_000_000.0;
            res.push(AlerteDto {
                date: d.to_string(),
                type_: "dividende".into(),
                titre: l.nom.clone(),
                enveloppe: env.into(),
                message: format!(
                    "Détachement du dividende estimé vers le {} (dernier versement : {} € par action). Date déduite de l'an dernier, à confirmer auprès de la société.",
                    date_fr(d),
                    format!("{montant:.2}").replace('.', ",")
                ),
                evenement_id: None,
            });
        }
    }

    for e in etat.db.lock().map_err(err)?.evenements().map_err(err)? {
        let Ok(d) = e.date.parse::<Date>() else { continue };
        if d < jour || d > limite {
            continue;
        }
        let (nom, env) = noms.get(&e.symbole).cloned().unwrap_or((e.symbole.clone(), ""));
        let quoi = if e.type_ == "AG" { "Assemblée générale" } else { "Événement" };
        res.push(AlerteDto {
            date: d.to_string(),
            type_: if e.type_ == "AG" { "ag" } else { "autre" }.into(),
            titre: nom,
            enveloppe: env.into(),
            message: match e.note.as_deref().filter(|n| !n.trim().is_empty()) {
                Some(n) => format!("{quoi} le {} : {n}", date_fr(d)),
                None => format!("{quoi} le {}.", date_fr(d)),
            },
            evenement_id: Some(e.id),
        });
    }
    res.sort_by(|a, b| a.date.cmp(&b.date));
    Ok(res)
}

#[tauri::command]
pub fn evenements(etat: State<'_, Etat>) -> Resultat<Vec<Evenement>> {
    etat.db.lock().map_err(err)?.evenements().map_err(err)
}

#[derive(Deserialize)]
pub struct SaisieEvenement {
    pub symbole: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub date: String,
    pub note: Option<String>,
}

#[tauri::command]
pub fn ajouter_evenement(etat: State<'_, Etat>, saisie: SaisieEvenement) -> Resultat<Vec<Evenement>> {
    saisie.date.parse::<Date>().map_err(err)?;
    if saisie.type_ != "AG" && saisie.type_ != "AUTRE" {
        return Err("Type d'événement inconnu.".into());
    }
    let db = etat.db.lock().map_err(err)?;
    db.ajouter_evenement(&Evenement { id: 0, symbole: saisie.symbole, type_: saisie.type_, date: saisie.date, note: saisie.note })
        .map_err(err)?;
    db.evenements().map_err(err)
}

#[tauri::command]
pub fn supprimer_evenement(etat: State<'_, Etat>, id: i64) -> Resultat<Vec<Evenement>> {
    let db = etat.db.lock().map_err(err)?;
    db.supprimer_evenement(id).map_err(err)?;
    db.evenements().map_err(err)
}

// ---------- Nouvelles versions ----------

#[derive(Deserialize)]
struct ReleaseGithub {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

#[derive(Serialize)]
pub struct MiseAJourDto {
    pub version_actuelle: String,
    pub derniere_version: Option<String>,
    pub disponible: bool,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub prerelease: bool,
}

/// "v0.2.1" ou "0.2.1" → (0, 2, 1).
fn version(s: &str) -> Option<(u64, u64, u64)> {
    let mut it = s.trim().trim_start_matches('v').split(['.', '-']);
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?, it.next().and_then(|x| x.parse().ok()).unwrap_or(0)))
}

/// Dépôt GitHub, déduit de `repository` dans Cargo.toml (https://github.com/proprietaire/depot).
fn depot() -> &'static str {
    env!("CARGO_PKG_REPOSITORY").trim_start_matches("https://github.com/").trim_end_matches('/')
}

/// Interroge GitHub pour savoir si une version plus récente est publiée.
/// Rien n'est téléchargé ni installé : l'utilisateur est seulement prévenu.
#[tauri::command]
pub async fn verifier_mise_a_jour(etat: State<'_, Etat>) -> Resultat<MiseAJourDto> {
    let actuelle = env!("CARGO_PKG_VERSION");
    let url = format!("https://api.github.com/repos/{}/releases?per_page=10", depot());
    let releases: Vec<ReleaseGithub> = etat
        .http
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("GitHub injoignable : {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub a refusé la demande : {e}"))?
        .json()
        .await
        .map_err(|e| format!("réponse GitHub illisible : {e}"))?;
    let derniere = releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| Some((version(&r.tag_name)?, r)))
        .max_by_key(|(v, _)| *v);
    let courante = version(actuelle);
    Ok(match derniere {
        Some((v, r)) => MiseAJourDto {
            version_actuelle: actuelle.into(),
            derniere_version: Some(r.tag_name.trim_start_matches('v').into()),
            disponible: courante.is_some_and(|c| v > c),
            url: Some(r.html_url),
            notes: r.body,
            prerelease: r.prerelease,
        },
        None => MiseAJourDto {
            version_actuelle: actuelle.into(),
            derniere_version: None,
            disponible: false,
            url: None,
            notes: None,
            prerelease: false,
        },
    })
}

/// Réglage « vérifier les nouvelles versions au démarrage » (activé par défaut).
#[tauri::command]
pub fn verification_auto(etat: State<'_, Etat>, activer: Option<bool>) -> Resultat<bool> {
    let db = etat.db.lock().map_err(err)?;
    if let Some(a) = activer {
        db.definir_parametre("maj.auto", if a { "1" } else { "0" }).map_err(err)?;
    }
    Ok(db.parametre("maj.auto").map_err(err)?.as_deref() != Some("0"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(version("v0.2.1"), Some((0, 2, 1)));
        assert_eq!(version("0.10"), Some((0, 10, 0)));
        assert!(version("v0.10.0") > version("v0.9.3"));
        assert_eq!(version("beta"), None);
    }
}
