//! Identifiants des titres : contrôle des codes ISIN.

/// Vrai si `s` est un code ISIN valide : 2 lettres (pays), 9 caractères alphanumériques et une clé
/// de contrôle calculée selon la norme ISO 6166 (lettres converties en nombres, puis algorithme de Luhn).
pub fn isin_valide(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 12
        || !b[..2].iter().all(u8::is_ascii_uppercase)
        || !b[2..11].iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        || !b[11].is_ascii_digit()
    {
        return false;
    }
    // Chaque lettre devient deux chiffres (A = 10 … Z = 35).
    let mut chiffres: Vec<u32> = Vec::with_capacity(24);
    for &c in &b[..11] {
        if c.is_ascii_digit() {
            chiffres.push((c - b'0') as u32);
        } else {
            let v = (c - b'A') as u32 + 10;
            chiffres.push(v / 10);
            chiffres.push(v % 10);
        }
    }
    // Luhn : on double un chiffre sur deux en partant de la droite (le plus à droite est doublé).
    let somme: u32 = chiffres
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 0 {
                let x = d * 2;
                x / 10 + x % 10
            } else {
                d
            }
        })
        .sum();
    (10 - somme % 10) % 10 == (b[11] - b'0') as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isin_valides_et_invalides() {
        for ok in ["FR0000121014", "FR0000120073", "NL0000235190", "US0378331005", "FR0011049824", "FR0007080254"] {
            assert!(isin_valide(ok), "{ok}");
        }
        for ko in ["FR0000121015", "FR000012101", "fr0000121014", "FR00001210A0", "1R0000121014", ""] {
            assert!(!isin_valide(ko), "{ko}");
        }
    }
}
