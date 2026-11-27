#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod alertes;
mod calendrier;
mod chiffrement;
mod commands;
mod cours;
mod import;
mod import_pea;
mod livrets;
mod portefeuille;
mod reglages;
mod store;

use std::sync::{Mutex, MutexGuard};

use tauri::Manager;

/// État partagé entre les commandes.
pub struct Etat {
    db: Mutex<store::Store>,
    pub http: reqwest::Client,
    pub chemin_db: std::path::PathBuf,
}

impl Etat {
    /// Accès à la base. Si une commande a paniqué en la tenant, le verrou est « empoisonné » :
    /// on le récupère quand même, car chaque écriture est une requête ou une transaction SQLite complète,
    /// et la base reste cohérente. Sans cela, toutes les commandes suivantes échoueraient.
    pub fn db(&self) -> MutexGuard<'_, store::Store> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dossier = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dossier)?;
            let chemin_db = dossier.join("escarcelle.db");
            let db = store::Store::ouvrir(&chemin_db)?;
            let http = reqwest::Client::builder()
                .user_agent(cours::USER_AGENT)
                .timeout(std::time::Duration::from_secs(10))
                .build()?;
            app.manage(Etat { db: Mutex::new(db), http, chemin_db });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ajouter_operation,
            commands::supprimer_operation,
            commands::tableau_de_bord,
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
            alertes::evenements,
            alertes::ajouter_evenement,
            alertes::supprimer_evenement,
            alertes::verifier_mise_a_jour,
            alertes::verification_auto,
            reglages::emplacement_donnees,
            reglages::sauvegarder,
            reglages::restaurer,
            reglages::sauvegarder_chiffre,
            reglages::lire_sauvegarde,
            reglages::tout_effacer,
            reglages::enregistrer_export,
            livrets::livrets,
            livrets::ouvrir_livret,
            livrets::conseil_livret,
            livrets::repartir_livrets,
            import::champs_import,
            import::analyser_import,
            import::importer,
            import::correspondances,
            import::enregistrer_correspondance,
            import::supprimer_correspondance,
            import::enregistrer_modele,
            import_pea::analyser_fichier,
            calendrier::calendrier_dividendes,
            calendrier::enregistrer_dividende,
            calendrier::ignorer_dividende,
            calendrier::supprimer_dividende,
            import_pea::importer_positions,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement d'Escarcelle");
}
