//! Stockage local SQLite (un seul fichier `escarcelle.db`).
//! Montants en centimes, quantités et cours en millionièmes : aucun flottant.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("base de données : {0}")]
    Sql(#[from] rusqlite::Error),
}

const MIGRATIONS: &[&str] = &[r#"
CREATE TABLE comptes (
    id            INTEGER PRIMARY KEY,
    nom           TEXT NOT NULL,
    type          TEXT NOT NULL CHECK (type IN ('LIVRET_A','LDDS','LEP','PEA','PEA_PME','PER')),
    etablissement TEXT NOT NULL,
    produit       TEXT,
    ouverture     TEXT NOT NULL,
    cloture       TEXT
);

CREATE TABLE operations (
    id                INTEGER PRIMARY KEY,
    compte_id         INTEGER NOT NULL REFERENCES comptes(id) ON DELETE CASCADE,
    date              TEXT NOT NULL,
    type              TEXT NOT NULL CHECK (type IN
                        ('VERSEMENT','RETRAIT','ACHAT','VENTE','DIVIDENDE','FRAIS','VALORISATION')),
    montant_cents     INTEGER,
    isin              TEXT,
    quantite_micro    INTEGER,
    cours_micro       INTEGER,
    frais_cents       INTEGER NOT NULL DEFAULT 0,
    valeur_plan_cents INTEGER,
    libelle           TEXT,
    cree_le           TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX operations_par_compte ON operations(compte_id, date);

CREATE TABLE cours (
    isin        TEXT NOT NULL,
    date        TEXT NOT NULL,
    cours_micro INTEGER NOT NULL,
    source      TEXT NOT NULL,
    PRIMARY KEY (isin, date)
);

CREATE TABLE parametres (
    cle    TEXT PRIMARY KEY,
    valeur TEXT NOT NULL
);
"#, r#"
-- V1 du PEA : suivi de lignes (quantité, PRU), sans journal d'opérations.
CREATE TABLE positions (
    id             INTEGER PRIMARY KEY,
    enveloppe      TEXT NOT NULL DEFAULT 'PEA',
    isin           TEXT,
    symbole        TEXT NOT NULL,
    nom            TEXT NOT NULL,
    quantite_micro INTEGER NOT NULL CHECK (quantite_micro > 0),
    pru_micro      INTEGER NOT NULL CHECK (pru_micro >= 0),
    cree_le        TEXT NOT NULL DEFAULT (datetime('now'))
);
ALTER TABLE cours ADD COLUMN horodatage INTEGER;
"#];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Compte {
    pub id: i64,
    pub nom: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub etablissement: String,
    pub produit: Option<String>,
    pub ouverture: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    #[serde(default)]
    pub id: i64,
    pub compte_id: i64,
    pub date: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub montant_cents: Option<i64>,
    pub isin: Option<String>,
    pub quantite_micro: Option<i64>,
    pub cours_micro: Option<i64>,
    #[serde(default)]
    pub frais_cents: i64,
    pub valeur_plan_cents: Option<i64>,
    pub libelle: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub id: i64,
    pub enveloppe: String,
    pub isin: Option<String>,
    pub symbole: String,
    pub nom: String,
    pub quantite_micro: i64,
    pub pru_micro: i64,
}

#[derive(Clone, Debug)]
pub struct DernierCours {
    pub cours_micro: i64,
    /// Heure du cours (secondes Unix), 0 si inconnue.
    pub horodatage: i64,
    pub source: String,
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn ouvrir(chemin: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(chemin)?;
        Self::initialiser(conn)
    }

    #[cfg(test)]
    pub fn en_memoire() -> Result<Self, StoreError> {
        Self::initialiser(Connection::open_in_memory()?)
    }

    fn initialiser(conn: Connection) -> Result<Self, StoreError> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
        let mut s = Self { conn };
        s.migrer()?;
        Ok(s)
    }

    fn migrer(&mut self) -> Result<(), StoreError> {
        let version: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version.max(0) as usize) {
            let tx = self.conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    }

    /// Copie cohérente de toute la base dans `chemin` (le fichier ne doit pas exister).
    pub fn sauvegarder(&self, chemin: &Path) -> Result<(), StoreError> {
        self.conn.execute("VACUUM INTO ?1", [chemin.to_string_lossy().into_owned()])?;
        Ok(())
    }

    /// Vérifie qu'un fichier est bien une sauvegarde Escarcelle.
    pub fn est_sauvegarde(chemin: &Path) -> bool {
        let Ok(c) = Connection::open_with_flags(chemin, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else {
            return false;
        };
        c.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name IN ('comptes', 'operations', 'cours')",
            [],
            |r| r.get::<_, i64>(0),
        )
        .map(|n| n == 3)
        .unwrap_or(false)
    }

    /// Remplace toutes les données par celles de la sauvegarde, puis met le schéma à jour.
    pub fn restaurer(&mut self, chemin: &Path) -> Result<(), StoreError> {
        self.conn.restore(rusqlite::DatabaseName::Main, chemin, None::<fn(rusqlite::backup::Progress)>)?;
        self.migrer()
    }

    /// Efface toutes les données (le schéma est conservé).
    pub fn effacer_tout(&mut self) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute_batch(
            "DELETE FROM operations; DELETE FROM positions; DELETE FROM comptes; DELETE FROM cours; DELETE FROM parametres;",
        )?;
        tx.commit()?;
        self.conn.execute_batch("VACUUM;")?;
        Ok(())
    }

    pub fn comptes(&self) -> Result<Vec<Compte>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, nom, type, etablissement, produit, ouverture FROM comptes WHERE cloture IS NULL ORDER BY type, nom",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Compte {
                id: r.get(0)?,
                nom: r.get(1)?,
                type_: r.get(2)?,
                etablissement: r.get(3)?,
                produit: r.get(4)?,
                ouverture: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn ajouter_compte(&self, c: &Compte) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO comptes (nom, type, etablissement, produit, ouverture) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![c.nom, c.type_, c.etablissement, c.produit, c.ouverture],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn operations(&self, compte_id: i64) -> Result<Vec<Operation>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, compte_id, date, type, montant_cents, isin, quantite_micro, cours_micro, frais_cents, valeur_plan_cents, libelle
             FROM operations WHERE compte_id = ?1 ORDER BY date, id",
        )?;
        let rows = st.query_map([compte_id], |r| {
            Ok(Operation {
                id: r.get(0)?,
                compte_id: r.get(1)?,
                date: r.get(2)?,
                type_: r.get(3)?,
                montant_cents: r.get(4)?,
                isin: r.get(5)?,
                quantite_micro: r.get(6)?,
                cours_micro: r.get(7)?,
                frais_cents: r.get(8)?,
                valeur_plan_cents: r.get(9)?,
                libelle: r.get(10)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn ajouter_operation(&self, o: &Operation) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO operations (compte_id, date, type, montant_cents, isin, quantite_micro, cours_micro, frais_cents, valeur_plan_cents, libelle)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                o.compte_id,
                o.date,
                o.type_,
                o.montant_cents,
                o.isin,
                o.quantite_micro,
                o.cours_micro,
                o.frais_cents,
                o.valeur_plan_cents,
                o.libelle
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn supprimer_operation(&self, id: i64) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM operations WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn enregistrer_cours(&self, isin: &str, date: &str, cours_micro: i64, source: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO cours (isin, date, cours_micro, source) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(isin, date) DO UPDATE SET cours_micro = excluded.cours_micro, source = excluded.source",
            params![isin, date, cours_micro, source],
        )?;
        Ok(())
    }

    /// Dernier cours connu d'un titre (clé : symbole de cotation, ou ISIN pour les anciennes saisies).
    pub fn dernier_cours(&self, cle: &str) -> Result<Option<DernierCours>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT cours_micro, COALESCE(horodatage, 0), source FROM cours WHERE isin = ?1
                 ORDER BY date DESC, COALESCE(horodatage, 0) DESC LIMIT 1",
                [cle],
                |r| Ok(DernierCours { cours_micro: r.get(0)?, horodatage: r.get(1)?, source: r.get(2)? }),
            )
            .optional()?)
    }

    /// Enregistre un cours horodaté ; un seul cours conservé par jour et par titre.
    pub fn enregistrer_cotation(&self, cle: &str, cours_micro: i64, horodatage: i64, source: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO cours (isin, date, cours_micro, source, horodatage) VALUES (?1, date(?3, 'unixepoch'), ?2, ?4, ?3)
             ON CONFLICT(isin, date) DO UPDATE SET cours_micro = excluded.cours_micro, source = excluded.source,
                                                   horodatage = excluded.horodatage",
            params![cle, cours_micro, horodatage, source],
        )?;
        Ok(())
    }

    pub fn positions(&self, enveloppe: &str) -> Result<Vec<Position>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, enveloppe, isin, symbole, nom, quantite_micro, pru_micro FROM positions WHERE enveloppe = ?1 ORDER BY nom",
        )?;
        let rows = st.query_map([enveloppe], |r| {
            Ok(Position {
                id: r.get(0)?,
                enveloppe: r.get(1)?,
                isin: r.get(2)?,
                symbole: r.get(3)?,
                nom: r.get(4)?,
                quantite_micro: r.get(5)?,
                pru_micro: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn ajouter_position(&self, p: &Position) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO positions (enveloppe, isin, symbole, nom, quantite_micro, pru_micro) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![p.enveloppe, p.isin, p.symbole, p.nom, p.quantite_micro, p.pru_micro],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn modifier_position(&self, id: i64, quantite_micro: i64, pru_micro: i64) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE positions SET quantite_micro = ?2, pru_micro = ?3 WHERE id = ?1",
            params![id, quantite_micro, pru_micro],
        )?;
        Ok(())
    }

    pub fn supprimer_position(&self, id: i64) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM positions WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn parametre(&self, cle: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT valeur FROM parametres WHERE cle = ?1", [cle], |r| r.get(0))
            .optional()?)
    }

    pub fn definir_parametre(&self, cle: &str, valeur: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO parametres (cle, valeur) VALUES (?1, ?2) ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
            params![cle, valeur],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour() {
        let s = Store::en_memoire().unwrap();
        let id = s
            .ajouter_compte(&Compte {
                id: 0,
                nom: "Livret A".into(),
                type_: "LIVRET_A".into(),
                etablissement: "boursobank".into(),
                produit: Some("bourso-livret-a".into()),
                ouverture: "2024-01-10".into(),
            })
            .unwrap();
        s.ajouter_operation(&Operation {
            id: 0,
            compte_id: id,
            date: "2026-03-10".into(),
            type_: "VERSEMENT".into(),
            montant_cents: Some(100_000),
            isin: None,
            quantite_micro: None,
            cours_micro: None,
            frais_cents: 0,
            valeur_plan_cents: None,
            libelle: None,
        })
        .unwrap();
        assert_eq!(s.comptes().unwrap().len(), 1);
        assert_eq!(s.operations(id).unwrap()[0].montant_cents, Some(100_000));
        s.enregistrer_cotation("DCAM.PA", 6_108_000, 1_795_000_000, "Yahoo Finance").unwrap();
        s.enregistrer_cotation("DCAM.PA", 6_120_000, 1_795_000_600, "Yahoo Finance").unwrap();
        assert_eq!(s.dernier_cours("DCAM.PA").unwrap().unwrap().cours_micro, 6_120_000);
        let pid = s
            .ajouter_position(&Position {
                id: 0,
                enveloppe: "PEA".into(),
                isin: Some("FR001400U5Q4".into()),
                symbole: "DCAM.PA".into(),
                nom: "Amundi PEA Monde".into(),
                quantite_micro: 100_000_000,
                pru_micro: 5_500_000,
            })
            .unwrap();
        s.modifier_position(pid, 150_000_000, 5_600_000).unwrap();
        assert_eq!(s.positions("PEA").unwrap()[0].quantite_micro, 150_000_000);
    }

    #[test]
    fn sauvegarde_restauration_effacement() {
        let dossier = std::env::temp_dir().join(format!("escarcelle-test-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let sauvegarde = dossier.join("sauvegarde.db");
        let _ = std::fs::remove_file(&sauvegarde);

        let mut s = Store::ouvrir(&dossier.join("escarcelle.db")).unwrap();
        s.effacer_tout().unwrap();
        s.enregistrer_cotation("MC.PA", 600_000_000, 1_795_000_000, "test").unwrap();
        s.sauvegarder(&sauvegarde).unwrap();
        assert!(Store::est_sauvegarde(&sauvegarde));

        s.effacer_tout().unwrap();
        assert!(s.dernier_cours("MC.PA").unwrap().is_none());
        s.restaurer(&sauvegarde).unwrap();
        assert_eq!(s.dernier_cours("MC.PA").unwrap().unwrap().cours_micro, 600_000_000);
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
