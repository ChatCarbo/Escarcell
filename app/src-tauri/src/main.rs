#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod alertes;
mod catalog;
mod commands;
mod cours;
mod fiscal;
mod livrets;
mod portefeuille;
mod reglages;
mod store;

use std::sync::Mutex;

use tauri::Manager;

/// État partagé entre les commandes.
pub struct Etat {
    pub db: Mutex<store::Store>,
    pub catalogue: catalog::Catalogue,
    pub http: reqwest::Client,
    pub chemin_db: std::path::PathBuf,
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dossier = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dossier)?;
            let chemin_db = dossier.join("escarcelle.db");
            let db = store::Store::ouvrir(&chemin_db)?;
            let catalogue = catalog::Catalogue::embarque()?;
            let http = reqwest::Client::builder()
                .user_agent(cours::USER_AGENT)
                .timeout(std::time::Duration::from_secs(10))
                .build()?;
            app.manage(Etat { db: Mutex::new(db), catalogue, http, chemin_db });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::etablissements,
            commands::comptes,
            commands::ajouter_compte,
            commands::operations,
            commands::ajouter_operation,
            commands::supprimer_operation,
            commands::enregistrer_cours,
            commands::tableau_de_bord,
            commands::impot,
            commands::economie_per,
            commands::comparer_per,
            portefeuille::positions,
            portefeuille::ajouter_position,
            portefeuille::modifier_position,
            portefeuille::supprimer_position,
            portefeuille::actualiser_cours,
            portefeuille::saisir_cours,
            portefeuille::chercher_titres,
            portefeuille::analyse_titre,
            portefeuille::retenues,
            portefeuille::definir_retenue,
            alertes::alertes,
            alertes::evenements,
            alertes::ajouter_evenement,
            alertes::supprimer_evenement,
            alertes::verifier_mise_a_jour,
            alertes::verification_auto,
            reglages::emplacement_donnees,
            reglages::sauvegarder,
            reglages::restaurer,
            reglages::tout_effacer,
            reglages::enregistrer_export,
            livrets::livrets,
            livrets::ouvrir_livret,
            livrets::conseil_livret,
            livrets::repartir_livrets,
            fiscal::simulation_fiscale,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement d'Escarcelle");
}
