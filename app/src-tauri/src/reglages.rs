//! Sauvegarde, restauration et effacement des données.

use std::path::PathBuf;

use tauri::State;

use crate::store::Store;
use crate::Etat;

type Resultat<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn emplacement_donnees(etat: State<'_, Etat>) -> String {
    etat.chemin_db.to_string_lossy().into_owned()
}

/// Copie toutes les données dans le fichier choisi par l'utilisateur.
#[tauri::command]
pub fn sauvegarder(etat: State<'_, Etat>, chemin: String) -> Resultat<()> {
    let chemin = PathBuf::from(chemin);
    if chemin == etat.chemin_db {
        return Err("Choisissez un autre fichier que la base d'Escarcelle elle-même.".into());
    }
    // On écrit d'abord dans un fichier temporaire, puis on le renomme : si la copie échoue,
    // une ancienne sauvegarde portant le même nom n'est pas perdue.
    let mut temporaire = chemin.clone().into_os_string();
    temporaire.push(".escarcelle-tmp");
    let temporaire = PathBuf::from(temporaire);
    if temporaire.exists() {
        std::fs::remove_file(&temporaire).map_err(err)?;
    }
    if let Err(e) = etat.db().sauvegarder(&temporaire) {
        let _ = std::fs::remove_file(&temporaire);
        return Err(e.to_string());
    }
    // La boîte de dialogue a déjà demandé confirmation pour remplacer un fichier existant.
    std::fs::rename(&temporaire, &chemin).map_err(|e| {
        let _ = std::fs::remove_file(&temporaire);
        e.to_string()
    })
}

/// Remplace toutes les données par celles d'une sauvegarde.
#[tauri::command]
pub fn restaurer(etat: State<'_, Etat>, chemin: String) -> Resultat<()> {
    let chemin = PathBuf::from(chemin);
    if !Store::est_sauvegarde(&chemin) {
        return Err("Ce fichier n'est pas une sauvegarde Escarcelle.".into());
    }
    etat.db().restaurer(&chemin).map_err(err)
}

/// Efface toutes les données ; l'utilisateur doit taper EFFACER.
#[tauri::command]
pub fn tout_effacer(etat: State<'_, Etat>, confirmation: String) -> Resultat<()> {
    if confirmation.trim() != "EFFACER" {
        return Err("Tapez EFFACER en majuscules pour confirmer.".into());
    }
    etat.db().effacer_tout().map_err(err)
}

/// Enregistre un export (CSV ou image PNG) à l'emplacement choisi par l'utilisateur.
/// Seules les extensions .csv et .png sont acceptées.
#[tauri::command]
pub fn enregistrer_export(chemin: String, octets: Vec<u8>) -> Resultat<()> {
    let chemin = PathBuf::from(chemin);
    let ext = chemin.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    if !matches!(ext.as_deref(), Some("csv") | Some("png")) {
        return Err("Un export doit être un fichier .csv ou .png.".into());
    }
    std::fs::write(&chemin, octets).map_err(err)
}
