//! Stockage local SQLite (un seul fichier `escarcell.db`).
//! Montants en centimes, quantités et cours en millionièmes : aucun flottant.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("base de données : {0}")]
    Sql(#[from] rusqlite::Error),
}

const MIGRATIONS: &[&str] = &[
    r#"
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
"#,
    r#"
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
"#,
    r#"
-- CTO, dividendes, alertes.
ALTER TABLE positions ADD COLUMN pays TEXT;
CREATE TABLE dividendes (
    symbole       TEXT NOT NULL,
    date          TEXT NOT NULL,
    montant_micro INTEGER NOT NULL,
    PRIMARY KEY (symbole, date)
);
CREATE TABLE infos_titres (
    symbole           TEXT PRIMARY KEY,
    cours_un_an_micro INTEGER,
    maj               INTEGER NOT NULL
);
CREATE TABLE evenements (
    id      INTEGER PRIMARY KEY,
    symbole TEXT NOT NULL,
    type    TEXT NOT NULL CHECK (type IN ('AG', 'AUTRE')),
    date    TEXT NOT NULL,
    note    TEXT
);
"#,
    r#"
-- Devise de cotation des lignes (CTO : dollar, franc suisse, livre…). Le PRU reste en euros.
ALTER TABLE positions ADD COLUMN devise TEXT NOT NULL DEFAULT 'EUR';
"#,
    r#"
-- Liste Euronext des sociétés éligibles au PEA-PME.
CREATE TABLE pea_pme (
    isin  TEXT PRIMARY KEY,
    nom   TEXT NOT NULL,
    mnemo TEXT
);
"#,
    r#"
-- Version 0.3 : le compte-titres et le PEA-PME sont supprimés. Seul le PEA est suivi.
DELETE FROM positions WHERE enveloppe <> 'PEA';
DROP TABLE IF EXISTS pea_pme;
DELETE FROM parametres WHERE cle = 'pea_pme.maj';
DELETE FROM parametres WHERE cle IN
    ('retenue.US', 'retenue.CH', 'retenue.GB', 'retenue.CA', 'retenue.JP', 'retenue.HK', 'retenue.AU');
-- Taux de change (EURUSD=X…) devenus inutiles.
DELETE FROM cours WHERE isin LIKE 'EUR%=X';
DELETE FROM evenements WHERE symbole NOT IN (SELECT symbole FROM positions);
DELETE FROM dividendes WHERE symbole NOT IN (SELECT symbole FROM positions);
DELETE FROM infos_titres WHERE symbole NOT IN (SELECT symbole FROM positions);
"#,
    r#"
-- Version 0.3 : le cours d'il y a un an est désormais pris sur l'historique journalier (et non mensuel).
-- On oublie les valeurs déjà enregistrées pour qu'elles soient recalculées à la prochaine actualisation.
DELETE FROM infos_titres;
"#,
    r#"
-- Version 0.4 : import CSV des opérations. Chaque opération importée garde le numéro de son import,
-- ce qui permet d'annuler un import en bloc.
CREATE TABLE imports (
    id            INTEGER PRIMARY KEY,
    date          TEXT NOT NULL DEFAULT (datetime('now')),
    fichier       TEXT NOT NULL,
    lignes        INTEGER NOT NULL,
    -- Comptes ouverts par l'import (identifiants séparés par des virgules), refermés s'il est annulé.
    comptes_crees TEXT NOT NULL DEFAULT ''
);
-- Lignes du PEA saisies à la main remplacées par le calcul du journal, restaurées si l'import est annulé.
CREATE TABLE imports_positions (
    import_id      INTEGER NOT NULL REFERENCES imports(id) ON DELETE CASCADE,
    isin           TEXT,
    symbole        TEXT NOT NULL,
    nom            TEXT NOT NULL,
    quantite_micro INTEGER NOT NULL,
    pru_micro      INTEGER NOT NULL,
    pays           TEXT
);
ALTER TABLE operations ADD COLUMN import_id INTEGER REFERENCES imports(id);
CREATE INDEX operations_par_import ON operations(import_id);
-- 1 : quantité et PRU calculés à partir des achats et ventes du journal (non modifiables à la main).
ALTER TABLE positions ADD COLUMN journal INTEGER NOT NULL DEFAULT 0;
"#,
    r#"
-- Version 0.4 : import d'une photo du portefeuille (export Moning, BoursoBank, tableur…). Pour annuler, on
-- garde dans imports_positions les lignes remplacées ou supprimées, et une marque (quantité 0) pour chaque
-- ligne écrite.
ALTER TABLE imports ADD COLUMN genre TEXT NOT NULL DEFAULT 'OPERATIONS';
"#,
    r#"
-- Version 0.4 : l'annulation des imports est retirée (une copie de sécurité est faite avant chaque import et se
-- restaure dans Paramètres). Les lignes mises de côté pour l'annuler ne servent plus. La table imports reste :
-- elle date les imports d'opérations (colonne import_id des opérations).
DROP TABLE IF EXISTS imports_positions;
"#,
    r#"
-- Version 0.4 : calendrier des dividendes. Un dividende enregistré depuis le calendrier garde le symbole de
-- cotation de la ligne (une ligne saisie par son nom n'a pas toujours d'ISIN) ; un détachement que l'utilisateur
-- choisit de ne pas enregistrer n'est plus proposé.
ALTER TABLE operations ADD COLUMN symbole TEXT;
CREATE TABLE dividendes_ignores (
    symbole TEXT NOT NULL,
    date    TEXT NOT NULL,
    PRIMARY KEY (symbole, date)
);
"#,
];

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

/// Ligne du PEA.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub id: i64,
    pub isin: Option<String>,
    pub symbole: String,
    pub nom: String,
    pub quantite_micro: i64,
    pub pru_micro: i64,
    /// Pays de la société (code ISO), pour la retenue à la source sur les dividendes.
    pub pays: Option<String>,
    /// Ligne calculée à partir du journal des achats et ventes importés.
    #[serde(default)]
    pub journal: bool,
}

/// Dividende encaissé dans le PEA (opération de type dividende : montant net crédité).
#[derive(Clone, Debug, Serialize)]
pub struct DividendePercu {
    pub id: i64,
    pub date: String,
    pub isin: Option<String>,
    pub symbole: Option<String>,
    pub montant_cents: i64,
    pub libelle: Option<String>,
}

/// État final d'une ligne du PEA après rejeu du journal (quantité 0 : ligne soldée, supprimée).
#[derive(Clone, Debug)]
pub struct LignePeaCalculee {
    pub isin: String,
    pub quantite_micro: i64,
    pub pru_micro: i64,
    /// Symbole et nom d'une nouvelle ligne (une ligne existante garde les siens).
    pub symbole: String,
    pub nom: String,
}

/// Tout ce qu'un import écrit, préparé et contrôlé à l'avance : il est appliqué en une transaction.
#[derive(Clone, Debug, Default)]
pub struct PlanImport {
    pub fichier: String,
    pub lignes: i64,
    /// Comptes à ouvrir s'ils n'existent pas (type, nom, date d'ouverture).
    pub comptes: Vec<Compte>,
    /// Opérations avec le type de leur compte (le `compte_id` est retrouvé à l'écriture).
    pub operations: Vec<(String, Operation)>,
    /// État final des lignes du PEA touchées par l'import.
    pub lignes_pea: Vec<LignePeaCalculee>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Evenement {
    pub id: i64,
    pub symbole: String,
    /// "AG" ou "AUTRE"
    #[serde(rename = "type")]
    pub type_: String,
    pub date: String,
    pub note: Option<String>,
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

    /// Version du schéma que cette version d'€scarcell sait lire (nombre de migrations).
    pub fn version_schema_connue() -> i64 {
        MIGRATIONS.len() as i64
    }

    /// Version du schéma de la base ouverte.
    pub fn version_schema(&self) -> Result<i64, StoreError> {
        Ok(self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// Version du schéma d'un fichier de sauvegarde (None si illisible).
    pub fn version_schema_fichier(chemin: &Path) -> Option<i64> {
        let c = Connection::open_with_flags(chemin, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
        c.pragma_query_value(None, "user_version", |r| r.get(0)).ok()
    }

    /// Vérifie qu'un fichier est bien une sauvegarde €scarcell.
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
            "DELETE FROM operations; DELETE FROM imports; DELETE FROM positions; \
             DELETE FROM comptes; DELETE FROM cours; DELETE FROM parametres; \
             DELETE FROM dividendes; DELETE FROM dividendes_ignores; DELETE FROM infos_titres; DELETE FROM evenements;",
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
    pub fn enregistrer_cotation(
        &self,
        cle: &str,
        cours_micro: i64,
        horodatage: i64,
        source: &str,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO cours (isin, date, cours_micro, source, horodatage) VALUES (?1, date(?3, 'unixepoch'), ?2, ?4, ?3)
             ON CONFLICT(isin, date) DO UPDATE SET cours_micro = excluded.cours_micro, source = excluded.source,
                                                   horodatage = excluded.horodatage",
            params![cle, cours_micro, horodatage, source],
        )?;
        Ok(())
    }

    /// Lignes du PEA. Les colonnes `enveloppe` (toujours « PEA ») et `devise` (toujours « EUR ») sont
    /// conservées dans la base pour la compatibilité des sauvegardes, mais ne sont plus lues.
    pub fn positions(&self) -> Result<Vec<Position>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT id, isin, symbole, nom, quantite_micro, pru_micro, pays, journal FROM positions WHERE enveloppe = 'PEA' ORDER BY nom, id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(Position {
                id: r.get(0)?,
                isin: r.get(1)?,
                symbole: r.get(2)?,
                nom: r.get(3)?,
                quantite_micro: r.get(4)?,
                pru_micro: r.get(5)?,
                pays: r.get(6)?,
                journal: r.get::<_, i64>(7)? != 0,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn ajouter_position(&self, p: &Position) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO positions (enveloppe, isin, symbole, nom, quantite_micro, pru_micro, pays, devise, journal)
             VALUES ('PEA', ?1, ?2, ?3, ?4, ?5, ?6, 'EUR', ?7)",
            params![p.isin, p.symbole, p.nom, p.quantite_micro, p.pru_micro, p.pays, p.journal as i64],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn modifier_position(
        &self,
        id: i64,
        quantite_micro: i64,
        pru_micro: i64,
        pays: Option<&str>,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE positions SET quantite_micro = ?2, pru_micro = ?3, pays = ?4 WHERE id = ?1",
            params![id, quantite_micro, pru_micro, pays],
        )?;
        Ok(())
    }

    /// Remplace l'historique des dividendes d'un titre (date ISO, montant par action en millionièmes).
    pub fn remplacer_dividendes(&mut self, symbole: &str, versements: &[(String, i64)]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM dividendes WHERE symbole = ?1", [symbole])?;
        for (date, montant) in versements {
            tx.execute(
                "INSERT OR REPLACE INTO dividendes (symbole, date, montant_micro) VALUES (?1, ?2, ?3)",
                params![symbole, date, montant],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn dividendes(&self, symbole: &str) -> Result<Vec<(String, i64)>, StoreError> {
        let mut st =
            self.conn.prepare("SELECT date, montant_micro FROM dividendes WHERE symbole = ?1 ORDER BY date")?;
        let rows = st.query_map([symbole], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn definir_infos_titre(
        &self,
        symbole: &str,
        cours_un_an_micro: Option<i64>,
        maj: i64,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO infos_titres (symbole, cours_un_an_micro, maj) VALUES (?1, ?2, ?3) \
             ON CONFLICT(symbole) DO UPDATE SET cours_un_an_micro = excluded.cours_un_an_micro, maj = excluded.maj",
            params![symbole, cours_un_an_micro, maj],
        )?;
        Ok(())
    }

    /// Cours d'il y a un an et date de mise à jour des informations du titre.
    pub fn infos_titre(&self, symbole: &str) -> Result<Option<(Option<i64>, i64)>, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT cours_un_an_micro, maj FROM infos_titres WHERE symbole = ?1", [symbole], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .optional()?)
    }

    pub fn evenements(&self) -> Result<Vec<Evenement>, StoreError> {
        let mut st = self.conn.prepare("SELECT id, symbole, type, date, note FROM evenements ORDER BY date")?;
        let rows = st.query_map([], |r| {
            Ok(Evenement { id: r.get(0)?, symbole: r.get(1)?, type_: r.get(2)?, date: r.get(3)?, note: r.get(4)? })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn ajouter_evenement(&self, e: &Evenement) -> Result<i64, StoreError> {
        self.conn.execute(
            "INSERT INTO evenements (symbole, type, date, note) VALUES (?1, ?2, ?3, ?4)",
            params![e.symbole, e.type_, e.date, e.note],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn supprimer_evenement(&self, id: i64) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM evenements WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn supprimer_position(&self, id: i64) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM positions WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Paramètres dont la clé commence par `prefixe`, triés par clé.
    pub fn parametres(&self, prefixe: &str) -> Result<Vec<(String, String)>, StoreError> {
        let mut st = self
            .conn
            .prepare("SELECT cle, valeur FROM parametres WHERE substr(cle, 1, length(?1)) = ?1 ORDER BY cle")?;
        let rows = st.query_map([prefixe], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn supprimer_parametre(&self, cle: &str) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM parametres WHERE cle = ?1", [cle])?;
        Ok(())
    }

    // ---------- Import CSV ----------

    /// Applique un import en une seule transaction : tout ou rien. Rend le numéro de l'import.
    pub fn importer(&mut self, plan: &PlanImport) -> Result<i64, StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO imports (fichier, lignes) VALUES (?1, ?2)", params![plan.fichier, plan.lignes])?;
        let import_id = tx.last_insert_rowid();
        for c in &plan.comptes {
            let existe: i64 =
                tx.query_row("SELECT count(*) FROM comptes WHERE type = ?1 AND cloture IS NULL", [&c.type_], |r| {
                    r.get(0)
                })?;
            if existe == 0 {
                tx.execute(
                    "INSERT INTO comptes (nom, type, etablissement, produit, ouverture) VALUES (?1, ?2, '', NULL, ?3)",
                    params![c.nom, c.type_, c.ouverture],
                )?;
            }
        }
        for (type_compte, o) in &plan.operations {
            let compte_id: i64 = tx.query_row(
                "SELECT id FROM comptes WHERE type = ?1 AND cloture IS NULL ORDER BY id LIMIT 1",
                [type_compte],
                |r| r.get(0),
            )?;
            tx.execute(
                "INSERT INTO operations (compte_id, date, type, montant_cents, isin, quantite_micro, cours_micro, frais_cents, libelle, import_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    compte_id,
                    o.date,
                    o.type_,
                    o.montant_cents,
                    o.isin,
                    o.quantite_micro,
                    o.cours_micro,
                    o.frais_cents,
                    o.libelle,
                    import_id
                ],
            )?;
        }
        for l in &plan.lignes_pea {
            // Les lignes saisies à la main pour ce titre sont remplacées par la ligne calculée (pays gardé).
            let pays: Option<String> = tx
                .query_row(
                    "SELECT pays FROM positions WHERE isin = ?1 AND pays IS NOT NULL ORDER BY journal DESC, id LIMIT 1",
                    [&l.isin],
                    |r| r.get(0),
                )
                .optional()?;
            let ancien: Option<(String, String)> = tx
                .query_row(
                    "SELECT symbole, nom FROM positions WHERE isin = ?1 ORDER BY journal DESC, id LIMIT 1",
                    [&l.isin],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            ecrire_ligne_journal(&tx, l, ancien, pays)?;
        }
        tx.commit()?;
        Ok(import_id)
    }

    /// Applique l'import d'une photo du portefeuille en une transaction : lignes mises à jour (les anciennes,
    /// identifiées, sont remplacées), nouvelles lignes, lignes supprimées. Seules les lignes saisies à la main
    /// sont touchées.
    pub fn importer_positions(
        &mut self,
        mises_a_jour: &[(Vec<i64>, Position)],
        nouvelles: &[Position],
        supprimees: &[i64],
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        let ecrites = mises_a_jour.iter().map(|(a, p)| (a.as_slice(), p)).chain(nouvelles.iter().map(|p| (&[][..], p)));
        for id in supprimees.iter().chain(mises_a_jour.iter().flat_map(|(a, _)| a)) {
            tx.execute("DELETE FROM positions WHERE id = ?1 AND journal = 0", [id])?;
        }
        for (_, p) in ecrites {
            tx.execute(
                "INSERT INTO positions (enveloppe, isin, symbole, nom, quantite_micro, pru_micro, pays, devise, journal)
                 VALUES ('PEA', ?1, ?2, ?3, ?4, ?5, ?6, 'EUR', 0)",
                params![p.isin, p.symbole, p.nom, p.quantite_micro, p.pru_micro, p.pays],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    // ---------- Dividendes encaissés ----------

    /// Dividendes encaissés dans le PEA, par date.
    pub fn dividendes_percus(&self) -> Result<Vec<DividendePercu>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT o.id, o.date, o.isin, o.symbole, o.montant_cents, o.libelle
             FROM operations o JOIN comptes c ON c.id = o.compte_id
             WHERE c.type = 'PEA' AND o.type = 'DIVIDENDE' AND o.montant_cents IS NOT NULL
             ORDER BY o.date, o.id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(DividendePercu {
                id: r.get(0)?,
                date: r.get(1)?,
                isin: r.get(2)?,
                symbole: r.get(3)?,
                montant_cents: r.get(4)?,
                libelle: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Enregistre un dividende encaissé (montant net) ; le compte PEA est ouvert s'il n'existe pas.
    pub fn ajouter_dividende(
        &mut self,
        date: &str,
        isin: Option<&str>,
        symbole: &str,
        montant_cents: i64,
        libelle: &str,
    ) -> Result<i64, StoreError> {
        let tx = self.conn.transaction()?;
        let compte: Option<i64> = tx
            .query_row("SELECT id FROM comptes WHERE type = 'PEA' AND cloture IS NULL ORDER BY id LIMIT 1", [], |r| {
                r.get(0)
            })
            .optional()?;
        let compte = match compte {
            Some(id) => id,
            None => {
                tx.execute(
                    "INSERT INTO comptes (nom, type, etablissement, produit, ouverture) VALUES ('PEA', 'PEA', '', NULL, ?1)",
                    [date],
                )?;
                tx.last_insert_rowid()
            }
        };
        tx.execute(
            "INSERT INTO operations (compte_id, date, type, montant_cents, isin, symbole, frais_cents, libelle)
             VALUES (?1, ?2, 'DIVIDENDE', ?3, ?4, ?5, 0, ?6)",
            params![compte, date, montant_cents, isin, symbole, libelle],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }

    /// Supprime un dividende encaissé (et rien d'autre).
    pub fn supprimer_dividende(&self, id: i64) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM operations WHERE id = ?1 AND type = 'DIVIDENDE'", [id])?;
        Ok(())
    }

    /// Détachement que l'utilisateur ne veut pas enregistrer : il n'est plus proposé.
    pub fn ignorer_dividende(&self, symbole: &str, date: &str) -> Result<(), StoreError> {
        self.conn
            .execute("INSERT OR IGNORE INTO dividendes_ignores (symbole, date) VALUES (?1, ?2)", [symbole, date])?;
        Ok(())
    }

    pub fn dividendes_ignores(&self) -> Result<Vec<(String, String)>, StoreError> {
        let mut st = self.conn.prepare("SELECT symbole, date FROM dividendes_ignores")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Supprime une ligne du PEA calculée par le journal, avec les achats et ventes de ce titre.
    pub fn supprimer_ligne_journal(&mut self, id: i64) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        let isin: Option<String> =
            tx.query_row("SELECT isin FROM positions WHERE id = ?1", [id], |r| r.get(0)).optional()?.flatten();
        if let Some(isin) = isin {
            tx.execute(
                "DELETE FROM operations WHERE isin = ?1 AND type IN ('ACHAT', 'VENTE')
                 AND compte_id IN (SELECT id FROM comptes WHERE type = 'PEA')",
                [&isin],
            )?;
            tx.execute("DELETE FROM positions WHERE isin = ?1 AND journal = 1", [&isin])?;
        }
        tx.execute("DELETE FROM positions WHERE id = ?1", [id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn parametre(&self, cle: &str) -> Result<Option<String>, StoreError> {
        Ok(self.conn.query_row("SELECT valeur FROM parametres WHERE cle = ?1", [cle], |r| r.get(0)).optional()?)
    }

    pub fn definir_parametre(&self, cle: &str, valeur: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO parametres (cle, valeur) VALUES (?1, ?2) ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
            params![cle, valeur],
        )?;
        Ok(())
    }
}

/// Remplace les lignes d'un titre par la ligne calculée (aucune si elle est soldée).
fn ecrire_ligne_journal(
    tx: &rusqlite::Transaction<'_>,
    l: &LignePeaCalculee,
    ancien: Option<(String, String)>,
    pays: Option<String>,
) -> Result<(), StoreError> {
    tx.execute("DELETE FROM positions WHERE isin = ?1", [&l.isin])?;
    if l.quantite_micro > 0 {
        let (symbole, nom) = ancien.unwrap_or_else(|| (l.symbole.clone(), l.nom.clone()));
        tx.execute(
            "INSERT INTO positions (enveloppe, isin, symbole, nom, quantite_micro, pru_micro, pays, devise, journal)
             VALUES ('PEA', ?1, ?2, ?3, ?4, ?5, ?6, 'EUR', 1)",
            params![l.isin, symbole, nom, l.quantite_micro, l.pru_micro, pays],
        )?;
    }
    Ok(())
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
                isin: Some("FR001400U5Q4".into()),
                symbole: "DCAM.PA".into(),
                nom: "Amundi PEA Monde".into(),
                quantite_micro: 100_000_000,
                pru_micro: 5_500_000,
                pays: None,
                journal: false,
            })
            .unwrap();
        s.modifier_position(pid, 150_000_000, 5_600_000, Some("FR")).unwrap();
        assert_eq!(s.positions().unwrap()[0].quantite_micro, 150_000_000);
        assert_eq!(s.positions().unwrap()[0].pays.as_deref(), Some("FR"));
        let mut s = s;
        s.remplacer_dividendes("DCAM.PA", &[("2026-05-20".into(), 1_200_000)]).unwrap();
        assert_eq!(s.dividendes("DCAM.PA").unwrap().len(), 1);
        s.ajouter_evenement(&Evenement {
            id: 0,
            symbole: "DCAM.PA".into(),
            type_: "AG".into(),
            date: "2027-05-12".into(),
            note: None,
        })
        .unwrap();
        assert_eq!(s.evenements().unwrap()[0].type_, "AG");
    }

    /// Base de la version 0.2 avec des lignes de CTO et de PEA-PME : la migration ne garde que le PEA.
    #[test]
    fn migration_suppression_cto_et_pea_pme() {
        let conn = Connection::open_in_memory().unwrap();
        for sql in &MIGRATIONS[..5] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", 5i64).unwrap();
        conn.execute_batch(
            "INSERT INTO positions (enveloppe, symbole, nom, quantite_micro, pru_micro, devise) VALUES
                 ('PEA', 'MC.PA', 'LVMH', 1000000, 600000000, 'EUR'),
                 ('CTO', 'NVDA', 'NVIDIA', 1000000, 100000000, 'USD'),
                 ('PEA_PME', 'ALMDT.PA', 'Median', 1000000, 2000000, 'EUR');
             INSERT INTO pea_pme VALUES ('FR0011049824', 'MEDIAN TECHNOLOGIES', 'ALMDT');
             INSERT INTO parametres VALUES ('retenue.US', '150000'), ('retenue.DE', '263750'), ('pea_pme.maj', '2026-11-24');
             INSERT INTO cours (isin, date, cours_micro, source) VALUES ('EURUSD=X', '2026-11-24', 1050000, 'Yahoo Finance');
             INSERT INTO evenements (symbole, type, date) VALUES ('NVDA', 'AG', '2027-06-01'), ('MC.PA', 'AG', '2027-04-17');
             INSERT INTO dividendes VALUES ('NVDA', '2026-09-01', 10000), ('MC.PA', '2026-04-28', 7500000);",
        )
        .unwrap();
        let s = Store::initialiser(conn).unwrap();
        let lignes = s.positions().unwrap();
        assert_eq!(lignes.len(), 1);
        assert_eq!(lignes[0].symbole, "MC.PA");
        assert_eq!(s.parametre("retenue.US").unwrap(), None);
        assert_eq!(s.parametre("retenue.DE").unwrap().as_deref(), Some("263750"));
        assert_eq!(s.parametre("pea_pme.maj").unwrap(), None);
        assert!(s.dernier_cours("EURUSD=X").unwrap().is_none());
        assert_eq!(s.evenements().unwrap().len(), 1);
        assert!(s.dividendes("NVDA").unwrap().is_empty());
        assert_eq!(s.dividendes("MC.PA").unwrap().len(), 1);
        let pea_pme: i64 =
            s.conn.query_row("SELECT count(*) FROM sqlite_master WHERE name = 'pea_pme'", [], |r| r.get(0)).unwrap();
        assert_eq!(pea_pme, 0);
    }

    #[test]
    fn sauvegarde_restauration_effacement() {
        let dossier = std::env::temp_dir().join(format!("escarcell-test-{}", std::process::id()));
        std::fs::create_dir_all(&dossier).unwrap();
        let sauvegarde = dossier.join("sauvegarde.db");
        let _ = std::fs::remove_file(&sauvegarde);

        let mut s = Store::ouvrir(&dossier.join("escarcell.db")).unwrap();
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
