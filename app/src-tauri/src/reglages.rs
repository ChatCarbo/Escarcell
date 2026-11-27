//! Sauvegarde, restauration et effacement des données.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::chiffrement::{self, Metadonnees};
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
        return Err("Choisissez un autre fichier que la base d'€scarcell elle-même.".into());
    }
    // On écrit d'abord dans un fichier temporaire, puis on le renomme : si la copie échoue,
    // une ancienne sauvegarde portant le même nom n'est pas perdue.
    let mut temporaire = chemin.clone().into_os_string();
    temporaire.push(".escarcell-tmp");
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

/// Date et heure UTC : (« AAAA-MM-JJ HH:MM », « AAAAMMJJ-HHMMSS »).
fn horodatage() -> (String, String) {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    let d = escarcell_core::Date::depuis_jours_unix(s.div_euclid(86_400));
    let r = s.rem_euclid(86_400);
    let (h, m, sec) = (r / 3600, r % 3600 / 60, r % 60);
    (format!("{d} {h:02}:{m:02}"), format!("{}{:02}{:02}-{h:02}{m:02}{sec:02}", d.year(), d.month(), d.day()))
}

/// Copie de sécurité de la base dans le dossier de données (`escarcell-<motif>-AAAAMMJJ-HHMMSS.db`), avant
/// une opération qui remplace ou ajoute beaucoup de données. Seules les dernières copies de ce motif sont gardées.
pub fn copie_de_securite(db: &Store, chemin_db: &Path, motif: &str) -> Resultat<PathBuf> {
    let dossier = chemin_db.parent().map(Path::to_path_buf).unwrap_or_default();
    let prefixe = format!("escarcell-{motif}-");
    let mut copie = dossier.join(format!("{prefixe}{}.db", horodatage().1));
    // Deux copies dans la même seconde : suffixe pour ne pas écraser la première.
    let mut n = 1;
    while copie.exists() {
        n += 1;
        copie = dossier.join(format!("{prefixe}{}-{n}.db", horodatage().1));
    }
    db.sauvegarder(&copie).map_err(err)?;
    if let Ok(entrees) = std::fs::read_dir(&dossier) {
        let mut copies: Vec<PathBuf> = entrees
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefixe) && n.ends_with(".db"))
            })
            .collect();
        copies.sort();
        let trop = copies.len().saturating_sub(COPIES_GARDEES);
        for p in &copies[..trop] {
            let _ = std::fs::remove_file(p);
        }
    }
    Ok(copie)
}

/// Base de l'application avant son changement de nom (Escarcelle → €scarcell) : `escarcelle.db`, dans le même
/// dossier ou dans l'ancien dossier de données (`fr.chatcarbo.escarcelle`, voisin du nouveau).
fn ancienne_base(chemin_db: &Path) -> Option<PathBuf> {
    let dossier = chemin_db.parent()?;
    let mut candidats = vec![dossier.join("escarcelle.db")];
    if let Some(parent) = dossier.parent() {
        candidats.push(parent.join("fr.chatcarbo.escarcelle").join("escarcelle.db"));
    }
    candidats.into_iter().find(|c| c.is_file())
}

/// Si la base n'existe pas encore, copie celle d'avant le changement de nom (avec son journal SQLite).
/// L'ancienne base n'est pas touchée. Rend le chemin repris, s'il y en a un. Un échec de copie n'empêche pas
/// le démarrage : l'application part alors d'une base vide et l'ancienne reste restaurable dans Paramètres.
pub fn reprendre_ancienne_base(chemin_db: &Path) -> Option<PathBuf> {
    if chemin_db.exists() {
        return None;
    }
    let ancienne = ancienne_base(chemin_db)?;
    let mut copiees = Vec::new();
    for suffixe in ["", "-wal", "-shm"] {
        let source = PathBuf::from(format!("{}{suffixe}", ancienne.display()));
        let cible = PathBuf::from(format!("{}{suffixe}", chemin_db.display()));
        if source.is_file() {
            if std::fs::copy(&source, &cible).is_err() {
                for c in &copiees {
                    let _ = std::fs::remove_file(c);
                }
                return None;
            }
            copiees.push(cible);
        }
    }
    Some(ancienne)
}

/// Fichier temporaire dans le dossier de données d'€scarcell.
fn temporaire(etat: &Etat, nom: &str) -> PathBuf {
    etat.chemin_db.with_file_name(format!("escarcell-{nom}-{}.tmp", std::process::id()))
}

/// Copie cohérente de la base, en octets.
fn copie_base(etat: &Etat) -> Resultat<Vec<u8>> {
    let tmp = temporaire(etat, "copie");
    let _ = std::fs::remove_file(&tmp);
    let resultat = etat.db().sauvegarder(&tmp).map_err(err).and_then(|_| std::fs::read(&tmp).map_err(err));
    let _ = std::fs::remove_file(&tmp);
    resultat
}

/// Écrit un fichier en passant par un fichier temporaire renommé : un échec ne détruit pas l'ancien fichier.
fn ecrire_sans_risque(chemin: &Path, octets: &[u8]) -> Resultat<()> {
    let mut tmp = chemin.as_os_str().to_owned();
    tmp.push(".escarcell-tmp");
    let tmp = PathBuf::from(tmp);
    if let Err(e) = std::fs::write(&tmp, octets) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    std::fs::rename(&tmp, chemin).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

fn extension(chemin: &Path) -> Option<String> {
    chemin.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase())
}

/// Sauvegarde chiffrée par mot de passe (fichier `.escarcell`).
#[tauri::command]
pub fn sauvegarder_chiffre(etat: State<'_, Etat>, chemin: String, mot_de_passe: String) -> Resultat<()> {
    let chemin = PathBuf::from(chemin);
    if extension(&chemin).as_deref() != Some("escarcell") {
        return Err("Une sauvegarde chiffrée doit porter l'extension .escarcell.".into());
    }
    chiffrement::verifier_mot_de_passe(&mot_de_passe)?;
    let base = copie_base(&etat)?;
    let meta = Metadonnees {
        cree_le: horodatage().0,
        version_escarcelle: env!("CARGO_PKG_VERSION").into(),
        version_schema: etat.db().version_schema().map_err(err)?,
        appareil: std::env::var("COMPUTERNAME").unwrap_or_default(),
    };
    let fichier = chiffrement::chiffrer(&base, &meta, &mot_de_passe)?;
    ecrire_sans_risque(&chemin, &fichier)
}

#[derive(Serialize)]
pub struct InfoSauvegarde {
    pub chiffree: bool,
    /// Vide pour une sauvegarde non chiffrée (pas de métadonnées).
    pub cree_le: String,
    pub version_escarcelle: String,
    pub appareil: String,
    pub version_schema: i64,
}

/// Fichier de sauvegarde lu et, s'il est chiffré, déchiffré : base SQLite écrite dans un fichier temporaire.
struct Lecture {
    info: InfoSauvegarde,
    base: PathBuf,
    a_effacer: bool,
}

fn lire(etat: &Etat, chemin: &Path, mot_de_passe: Option<&str>) -> Resultat<Lecture> {
    let mut entete = [0u8; 4];
    let chiffree = std::fs::File::open(chemin)
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut entete))
        .map(|_| chiffrement::est_chiffre(&entete))
        .unwrap_or(false);
    let (info, base, a_effacer) = if chiffree {
        let mdp = mot_de_passe.ok_or("Cette sauvegarde est chiffrée : indiquez son mot de passe.")?;
        let (meta, octets) = chiffrement::dechiffrer(&std::fs::read(chemin).map_err(err)?, mdp)?;
        let tmp = temporaire(etat, "restauration");
        std::fs::write(&tmp, &octets).map_err(err)?;
        let info = InfoSauvegarde {
            chiffree: true,
            cree_le: meta.cree_le,
            version_escarcelle: meta.version_escarcelle,
            appareil: meta.appareil,
            version_schema: meta.version_schema,
        };
        (info, tmp, true)
    } else {
        let info = InfoSauvegarde {
            chiffree: false,
            cree_le: String::new(),
            version_escarcelle: String::new(),
            appareil: String::new(),
            version_schema: 0,
        };
        (info, chemin.to_path_buf(), false)
    };
    let lecture = Lecture { info, base, a_effacer };
    let verifier = || -> Resultat<i64> {
        if !Store::est_sauvegarde(&lecture.base) {
            return Err("Ce fichier n'est pas une sauvegarde €scarcell.".into());
        }
        let v = Store::version_schema_fichier(&lecture.base).unwrap_or(0);
        if v > Store::version_schema_connue() {
            return Err("Cette sauvegarde vient d'une version plus récente d'€scarcell : mettez €scarcell à jour avant de la restaurer.".into());
        }
        Ok(v)
    };
    match verifier() {
        Ok(v) => {
            let mut lecture = lecture;
            lecture.info.version_schema = v;
            Ok(lecture)
        }
        Err(e) => {
            if lecture.a_effacer {
                let _ = std::fs::remove_file(&lecture.base);
            }
            Err(e)
        }
    }
}

/// Informations sur une sauvegarde, à afficher avant de confirmer la restauration.
#[tauri::command]
pub fn lire_sauvegarde(
    etat: State<'_, Etat>,
    chemin: String,
    mot_de_passe: Option<String>,
) -> Resultat<InfoSauvegarde> {
    let l = lire(&etat, Path::new(&chemin), mot_de_passe.as_deref())?;
    if l.a_effacer {
        let _ = std::fs::remove_file(&l.base);
    }
    Ok(l.info)
}

/// Nombre de copies de sécurité automatiques conservées avant restauration.
const COPIES_GARDEES: usize = 5;

/// Remplace toutes les données par celles d'une sauvegarde (chiffrée ou non).
/// Une copie de sécurité de la base actuelle est faite d'abord, dans le dossier de données.
#[tauri::command]
pub fn restaurer(etat: State<'_, Etat>, chemin: String, mot_de_passe: Option<String>) -> Resultat<()> {
    let l = lire(&etat, Path::new(&chemin), mot_de_passe.as_deref())?;
    let resultat = (|| {
        let mut db = etat.db();
        copie_de_securite(&db, &etat.chemin_db, "avant-restauration")?;
        db.restaurer(&l.base).map_err(err)
    })();
    if l.a_effacer {
        let _ = std::fs::remove_file(&l.base);
    }
    resultat
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
    if !matches!(extension(&chemin).as_deref(), Some("csv") | Some("png")) {
        return Err("Un export doit être un fichier .csv ou .png.".into());
    }
    std::fs::write(&chemin, octets).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reprise_de_la_base_d_avant_le_changement_de_nom() {
        let racine = std::env::temp_dir().join(format!("escarcell-test-reprise-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&racine);
        let ancien = racine.join("fr.chatcarbo.escarcelle");
        let nouveau = racine.join("fr.chatcarbo.escarcell");
        std::fs::create_dir_all(&ancien).unwrap();
        std::fs::create_dir_all(&nouveau).unwrap();
        let db = nouveau.join("escarcell.db");
        // Rien à reprendre.
        assert_eq!(reprendre_ancienne_base(&db), None);
        std::fs::write(ancien.join("escarcelle.db"), b"base").unwrap();
        std::fs::write(ancien.join("escarcelle.db-wal"), b"journal").unwrap();
        assert_eq!(reprendre_ancienne_base(&db), Some(ancien.join("escarcelle.db")));
        assert_eq!(std::fs::read(&db).unwrap(), b"base");
        assert_eq!(std::fs::read(nouveau.join("escarcell.db-wal")).unwrap(), b"journal");
        assert!(ancien.join("escarcelle.db").exists(), "l'ancienne base reste en place");
        // Base déjà présente : jamais écrasée.
        std::fs::write(&db, b"nouvelle").unwrap();
        assert_eq!(reprendre_ancienne_base(&db), None);
        assert_eq!(std::fs::read(&db).unwrap(), b"nouvelle");
        let _ = std::fs::remove_dir_all(&racine);
    }
}
