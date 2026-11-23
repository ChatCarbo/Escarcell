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
    if chemin.exists() {
        // La boîte de dialogue a déjà demandé confirmation pour remplacer le fichier.
        std::fs::remove_file(&chemin).map_err(err)?;
    }
    etat.db.lock().map_err(err)?.sauvegarder(&chemin).map_err(err)
}

/// Remplace toutes les données par celles d'une sauvegarde.
#[tauri::command]
pub fn restaurer(etat: State<'_, Etat>, chemin: String) -> Resultat<()> {
    let chemin = PathBuf::from(chemin);
    if !Store::est_sauvegarde(&chemin) {
        return Err("Ce fichier n'est pas une sauvegarde Escarcelle.".into());
    }
    etat.db.lock().map_err(err)?.restaurer(&chemin).map_err(err)
}

/// Efface toutes les données ; l'utilisateur doit taper EFFACER.
#[tauri::command]
pub fn tout_effacer(etat: State<'_, Etat>, confirmation: String) -> Resultat<()> {
    if confirmation.trim() != "EFFACER" {
        return Err("Tapez EFFACER en majuscules pour confirmer.".into());
    }
    etat.db.lock().map_err(err)?.effacer_tout().map_err(err)
}
