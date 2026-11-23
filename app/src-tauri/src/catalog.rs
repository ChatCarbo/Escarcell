//! Lecture du catalogue `catalogue/etablissements.json` (embarqué dans le binaire).
//! Les champs détaillés restent en JSON libre (`reste`) : seul ce que l'application
//! utilise est lu de façon typée.

use escarcelle_core::comparateur::Contrat;
use escarcelle_core::Rate;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const EMBARQUE: &str = include_str!("../../../catalogue/etablissements.json");

#[derive(Debug, thiserror::Error)]
pub enum CatalogueError {
    #[error("catalogue illisible : {0}")]
    Json(#[from] serde_json::Error),
    #[error("version de catalogue non prise en charge : {0}")]
    Version(u32),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Catalogue {
    pub schema_version: u32,
    pub publie_le: String,
    pub etablissements: Vec<Etablissement>,
    #[serde(default)]
    pub etf: Vec<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Etablissement {
    pub id: String,
    pub nom: String,
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub couleur: Option<String>,
    pub produits: Vec<Produit>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Produit {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub nom: String,
    #[serde(flatten)]
    pub reste: Map<String, Value>,
}

fn taux(v: Option<&Value>) -> Option<Rate> {
    v.and_then(Value::as_str).and_then(Rate::parse_fraction)
}

impl Produit {
    fn frais(&self, cle: &str) -> Option<Rate> {
        taux(self.reste.get("frais").and_then(|f| f.get(cle)))
    }

    /// Rendement net du premier fonds euros pour l'année donnée.
    fn rendement_fonds_euros(&self, annee: &str) -> Option<Rate> {
        self.reste
            .get("fonds_euros")?
            .as_array()?
            .iter()
            .find_map(|f| taux(f.get("rendement_net").and_then(|r| r.get(annee))))
    }

    /// Conditions utilisables par le comparateur (gestion libre), si les données sont complètes.
    pub fn contrat_comparable(&self, annee: &str) -> Option<Contrat> {
        Some(Contrat {
            nom: self.nom.clone(),
            frais_versement: self.frais("versement").unwrap_or(Rate::ZERO),
            frais_gestion_uc: self.frais("gestion_uc")?,
            frais_transaction_uc: self.frais("transaction_etf").unwrap_or(Rate::ZERO),
            rendement_fonds_euros: self.rendement_fonds_euros(annee)?,
            part_fonds_euros_max: taux(
                self.reste.get("contraintes_fonds_euros").and_then(|c| c.get("gestion_libre_part_max")),
            ),
        })
    }
}

impl Catalogue {
    pub fn embarque() -> Result<Self, CatalogueError> {
        Self::lire(EMBARQUE)
    }

    pub fn lire(json: &str) -> Result<Self, CatalogueError> {
        let c: Catalogue = serde_json::from_str(json)?;
        if c.schema_version != 1 {
            return Err(CatalogueError::Version(c.schema_version));
        }
        Ok(c)
    }

    pub fn produits(&self, type_: &str) -> impl Iterator<Item = (&Etablissement, &Produit)> {
        let type_ = type_.to_string();
        self.etablissements
            .iter()
            .flat_map(|e| e.produits.iter().map(move |p| (e, p)))
            .filter(move |(_, p)| p.type_ == type_)
    }

    pub fn couleur(&self, etablissement: &str) -> Option<&str> {
        self.etablissements.iter().find(|e| e.id == etablissement).and_then(|e| e.couleur.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_embarque_lisible() {
        let c = Catalogue::embarque().unwrap();
        assert_eq!(c.etablissements.len(), 3);
        let per: Vec<_> = c.produits("PER").filter_map(|(_, p)| p.contrat_comparable("2025")).collect();
        assert_eq!(per.len(), 5, "les 5 PER de la V1 doivent être comparables");
        let matla = per.iter().find(|p| p.nom.contains("Matla")).unwrap();
        assert!(matla.part_fonds_euros_max.is_some());
    }
}
