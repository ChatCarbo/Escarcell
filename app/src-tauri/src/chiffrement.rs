//! Sauvegarde chiffrée : format de fichier `.escarcelle`.
//!
//! ```text
//! En-tête (57 octets, en clair mais authentifié) :
//!   "ESCA" | version du format (1 octet) | Argon2id m (u32 LE, en Kio) | t (u32 LE) | p (u32 LE)
//!   | sel (16 octets) | nonce (24 octets)
//! Puis : XChaCha20-Poly1305( longueur des métadonnées (u32 LE) | métadonnées UTF-8 | base SQLite ),
//!        avec l'en-tête en données associées : toute modification de l'en-tête ou du contenu est détectée.
//! ```
//!
//! La clé (32 octets) est dérivée du mot de passe par Argon2id ; les paramètres sont lus dans l'en-tête, ce qui
//! permettra de les augmenter plus tard sans rendre illisibles les anciennes sauvegardes. Le mot de passe n'est
//! jamais stocké ; la clé est effacée de la mémoire après usage. Aucune récupération n'est possible sans lui.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroize;

pub const SIGNATURE: &[u8; 4] = b"ESCA";
pub const VERSION_FORMAT: u8 = 1;
const TAILLE_SEL: usize = 16;
const TAILLE_NONCE: usize = 24;
pub const TAILLE_ENTETE: usize = 4 + 1 + 12 + TAILLE_SEL + TAILLE_NONCE;

/// Paramètres Argon2id des nouvelles sauvegardes : 64 Mio, 3 itérations, 1 fil.
const M_COUT: u32 = 64 * 1024;
const T_COUT: u32 = 3;
const P_COUT: u32 = 1;

/// Longueur minimale du mot de passe, en caractères.
pub const LONGUEUR_MIN: usize = 12;

/// Message unique pour un mot de passe faux ou un fichier altéré : on ne dit pas lequel.
pub const ECHEC: &str = "Mot de passe incorrect ou fichier endommagé.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadonnees {
    /// Date et heure de création (UTC), AAAA-MM-JJ HH:MM.
    pub cree_le: String,
    pub version_escarcelle: String,
    pub version_schema: i64,
    pub appareil: String,
}

impl Metadonnees {
    fn en_texte(&self) -> String {
        let propre = |s: &str| s.replace(['\n', '\r'], " ");
        format!(
            "cree_le={}\nversion_escarcelle={}\nversion_schema={}\nappareil={}\n",
            propre(&self.cree_le),
            propre(&self.version_escarcelle),
            self.version_schema,
            propre(&self.appareil)
        )
    }

    fn depuis_texte(t: &str) -> Option<Self> {
        let valeur = |cle: &str| {
            t.lines().find_map(|l| l.strip_prefix(cle).and_then(|r| r.strip_prefix('='))).map(str::to_string)
        };
        Some(Metadonnees {
            cree_le: valeur("cree_le")?,
            version_escarcelle: valeur("version_escarcelle")?,
            version_schema: valeur("version_schema")?.parse().ok()?,
            appareil: valeur("appareil").unwrap_or_default(),
        })
    }
}

/// Vérifie la robustesse minimale d'un mot de passe.
pub fn verifier_mot_de_passe(mdp: &str) -> Result<(), String> {
    if mdp.chars().count() < LONGUEUR_MIN {
        return Err(format!("Le mot de passe doit faire au moins {LONGUEUR_MIN} caractères."));
    }
    Ok(())
}

fn deriver(mdp: &str, sel: &[u8], m: u32, t: u32, p: u32) -> Result<[u8; 32], String> {
    let params = Params::new(m, t, p, Some(32)).map_err(|_| ECHEC.to_string())?;
    let mut cle = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(mdp.as_bytes(), sel, &mut cle)
        .map_err(|_| ECHEC.to_string())?;
    Ok(cle)
}

fn aleatoire(buf: &mut [u8]) -> Result<(), String> {
    getrandom::getrandom(buf).map_err(|e| format!("générateur aléatoire indisponible : {e}"))
}

/// Chiffre une base SQLite et ses métadonnées.
pub fn chiffrer(base: &[u8], meta: &Metadonnees, mdp: &str) -> Result<Vec<u8>, String> {
    verifier_mot_de_passe(mdp)?;
    let mut sel = [0u8; TAILLE_SEL];
    let mut nonce = [0u8; TAILLE_NONCE];
    aleatoire(&mut sel)?;
    aleatoire(&mut nonce)?;
    chiffrer_avec(base, meta, mdp, (M_COUT, T_COUT, P_COUT), &sel, &nonce)
}

fn chiffrer_avec(
    base: &[u8],
    meta: &Metadonnees,
    mdp: &str,
    (m, t, p): (u32, u32, u32),
    sel: &[u8; TAILLE_SEL],
    nonce: &[u8; TAILLE_NONCE],
) -> Result<Vec<u8>, String> {
    let mut entete = Vec::with_capacity(TAILLE_ENTETE);
    entete.extend_from_slice(SIGNATURE);
    entete.push(VERSION_FORMAT);
    entete.extend_from_slice(&m.to_le_bytes());
    entete.extend_from_slice(&t.to_le_bytes());
    entete.extend_from_slice(&p.to_le_bytes());
    entete.extend_from_slice(sel);
    entete.extend_from_slice(nonce);

    let texte = meta.en_texte();
    let mut clair = Vec::with_capacity(4 + texte.len() + base.len());
    clair.extend_from_slice(&(texte.len() as u32).to_le_bytes());
    clair.extend_from_slice(texte.as_bytes());
    clair.extend_from_slice(base);

    let mut cle = deriver(mdp, sel, m, t, p)?;
    let chiffreur = XChaCha20Poly1305::new_from_slice(&cle).map_err(|_| "Échec du chiffrement.".to_string());
    cle.zeroize();
    let chiffre = chiffreur?
        .encrypt(XNonce::from_slice(nonce), Payload { msg: &clair, aad: &entete })
        .map_err(|_| "Échec du chiffrement.".to_string());
    clair.zeroize();
    let mut fichier = entete;
    fichier.extend_from_slice(&chiffre?);
    Ok(fichier)
}

/// Vrai si le fichier commence par la signature d'une sauvegarde chiffrée.
pub fn est_chiffre(fichier: &[u8]) -> bool {
    fichier.len() >= 4 && &fichier[..4] == SIGNATURE
}

/// Déchiffre une sauvegarde : métadonnées et base SQLite.
pub fn dechiffrer(fichier: &[u8], mdp: &str) -> Result<(Metadonnees, Vec<u8>), String> {
    if !est_chiffre(fichier) {
        return Err("Ce fichier n'est pas une sauvegarde chiffrée Escarcelle.".into());
    }
    if fichier.len() < TAILLE_ENTETE + 16 {
        return Err(ECHEC.into());
    }
    if fichier[4] != VERSION_FORMAT {
        return Err("Cette sauvegarde vient d'une version plus récente d'Escarcelle : mettez Escarcelle à jour.".into());
    }
    let u32_a = |i: usize| u32::from_le_bytes([fichier[i], fichier[i + 1], fichier[i + 2], fichier[i + 3]]);
    let (m, t, p) = (u32_a(5), u32_a(9), u32_a(13));
    // Bornes : un en-tête modifié ne doit pas pouvoir réclamer des gigaoctets de mémoire ou des heures de calcul.
    if !(8..=1024 * 1024).contains(&m) || !(1..=10).contains(&t) || !(1..=8).contains(&p) || m < 8 * p {
        return Err(ECHEC.into());
    }
    let entete = &fichier[..TAILLE_ENTETE];
    let sel = &fichier[17..17 + TAILLE_SEL];
    let nonce = &fichier[17 + TAILLE_SEL..TAILLE_ENTETE];

    let mut cle = deriver(mdp, sel, m, t, p)?;
    let dechiffreur = XChaCha20Poly1305::new_from_slice(&cle).map_err(|_| ECHEC.to_string());
    cle.zeroize();
    let clair = dechiffreur?
        .decrypt(XNonce::from_slice(nonce), Payload { msg: &fichier[TAILLE_ENTETE..], aad: entete })
        .map_err(|_| ECHEC.to_string());
    let mut clair = clair?;

    let resultat = (|| {
        if clair.len() < 4 {
            return Err(ECHEC.to_string());
        }
        let n = u32::from_le_bytes([clair[0], clair[1], clair[2], clair[3]]) as usize;
        let texte = clair.get(4..4 + n).and_then(|b| std::str::from_utf8(b).ok()).ok_or(ECHEC)?;
        let meta = Metadonnees::depuis_texte(texte).ok_or(ECHEC)?;
        Ok((meta, clair[4 + n..].to_vec()))
    })();
    clair.zeroize();
    resultat
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta() -> Metadonnees {
        Metadonnees {
            cree_le: "2026-11-25 21:00".into(),
            version_escarcelle: "0.4.0".into(),
            version_schema: 7,
            appareil: "PC-SALON".into(),
        }
    }

    /// Paramètres Argon2id réduits pour que les tests restent rapides (le format est le même).
    fn rapide(base: &[u8], mdp: &str) -> Vec<u8> {
        chiffrer_avec(base, &meta(), mdp, (64, 1, 1), &[7; TAILLE_SEL], &[9; TAILLE_NONCE]).unwrap()
    }

    const MDP: &str = "cheval-agrafe-batterie";

    #[test]
    fn aller_retour() {
        let base = b"SQLite format 3\0 contenu".to_vec();
        let f = rapide(&base, MDP);
        assert!(est_chiffre(&f));
        assert_eq!(f.len(), TAILLE_ENTETE + 4 + meta().en_texte().len() + base.len() + 16);
        let (m, b) = dechiffrer(&f, MDP).unwrap();
        assert_eq!(m, meta());
        assert_eq!(b, base);
    }

    #[test]
    fn contenu_illisible_sans_mot_de_passe() {
        let base = b"PRU 140,99 Air Liquide".to_vec();
        let f = rapide(&base, MDP);
        assert!(!f.windows(base.len()).any(|w| w == base.as_slice()));
        assert!(!f.windows(8).any(|w| w == b"PC-SALON"));
    }

    #[test]
    fn mauvais_mot_de_passe_meme_message_que_fichier_altere() {
        let f = rapide(b"donnees", MDP);
        assert_eq!(dechiffrer(&f, "autre-mot-de-passe").unwrap_err(), ECHEC);
        // Chaque octet modifié, en-tête compris, est détecté.
        for i in [0usize, 5, 20, 40, TAILLE_ENTETE, f.len() - 1] {
            let mut g = f.clone();
            g[i] ^= 1;
            let e = dechiffrer(&g, MDP).unwrap_err();
            assert!(e == ECHEC || e.contains("n'est pas une sauvegarde chiffrée"), "octet {i} : {e}");
        }
        assert_eq!(dechiffrer(&f[..TAILLE_ENTETE + 3], MDP).unwrap_err(), ECHEC);
    }

    #[test]
    fn en_tete_aux_parametres_demesures_refuse() {
        let mut f = rapide(b"donnees", MDP);
        f[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(dechiffrer(&f, MDP).unwrap_err(), ECHEC);
    }

    #[test]
    fn version_de_format_inconnue() {
        let mut f = rapide(b"donnees", MDP);
        f[4] = 2;
        assert!(dechiffrer(&f, MDP).unwrap_err().contains("plus récente"));
    }

    #[test]
    fn mot_de_passe_trop_court() {
        assert!(verifier_mot_de_passe("court").is_err());
        assert!(verifier_mot_de_passe("douze-caract").is_ok());
        assert!(chiffrer(b"x", &meta(), "court").is_err());
    }

    #[test]
    fn metadonnees_sans_saut_de_ligne_injecte() {
        let mut m = meta();
        m.appareil = "PC\nversion_schema=99".into();
        let lu = Metadonnees::depuis_texte(&m.en_texte()).unwrap();
        assert_eq!(lu.version_schema, 7);
    }

    /// Vecteur de référence produit par une implémentation indépendante (Python, bibliothèque `cryptography` :
    /// Argon2id, puis HChaCha20 + ChaCha20-Poly1305, HChaCha20 contrôlé sur le vecteur du draft IRTF XChaCha).
    /// Garantit que le fichier suit bien le format documenté, et pas seulement qu'il se relit lui-même.
    #[test]
    fn vecteur_de_reference_independant() {
        let attendu = concat!(
            "455343410140000000010000000100000007070707070707070707070707070707090909090909090909090909090909",
            "090909090909090909da050c29fd74c610ea77e4e2152ec7a146341c2e8bff2eb63c4bb645471067734e0e0061a280ca",
            "f4909778d299f7228240abf6eb8a2ec7d64be1358176b8965845672c395e11b05684733e72cfa30a2a4309fc2becfda0",
            "2c3980896df63544eca9fe18d93d4cfe9894aa2e06b52a2b6440a2c47e0d033d9baa5f98377af6cb8170",
        );
        let hex: String = rapide(b"SQLite format 3\0 contenu", MDP).iter().map(|o| format!("{o:02x}")).collect();
        assert_eq!(hex, attendu);
    }
}
