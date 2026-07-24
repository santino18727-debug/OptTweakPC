//! Executeur PowerShell partage : un seul point d'entree pour lancer un script
//! PowerShell depuis l'app. Centralise (1) la resolution du chemin absolu de
//! powershell.exe et (2) la recuperation stdout/stderr, evitant la duplication
//! qui existait dans backup / system / startup / performance.

use std::process::Command;

/// Chemin ABSOLU de PowerShell, resolu via `%SystemRoot%` (variable fixee par
/// Windows, donc fiable) plutot qu'un litteral `C:` — supporte ainsi un Windows
/// installe sur un autre disque. On garde IMPERATIVEMENT un chemin absolu :
/// invoquer `powershell.exe` via le PATH rouvrirait la faille de detournement
/// (l'app peut tourner en administrateur). Le litteral `C:\Windows` ne sert que
/// d'ultime repli si `SystemRoot` etait absente (cas anormal).
pub fn powershell_path() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    format!(r"{}\System32\WindowsPowerShell\v1.0\powershell.exe", root)
}

/// Execute un script PowerShell et renvoie stdout (ou stderr en cas d'echec).
///
/// Args `-NoProfile -Command` : identiques au comportement historique de
/// backup / system / performance. L'ancien module `startup` ajoutait
/// `-ExecutionPolicy Bypass`, sans effet ici : la policy ne gouverne que les
/// fichiers `.ps1`, pas un script inline passe a `-Command`. Retire donc sans
/// changement de comportement.
pub fn run_ps(script: &str) -> Result<String, String> {
    let output = Command::new(powershell_path())
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| format!("Echec d'execution de PowerShell : {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Execute un script PowerShell qui produit du JSON et le normalise en tableau.
pub fn run_ps_json(script: &str) -> Result<Vec<serde_json::Value>, String> {
    parse_ps_json(&run_ps(script)?)
}

/// Normalise une sortie JSON PowerShell en tableau de valeurs. Extrait de
/// `run_ps_json` pour etre testable sans lancer PowerShell.
/// PowerShell 5.1 serialise un objet unique sans crochets : on tolere objet,
/// tableau ou sortie vide pour ne jamais casser cote front.
fn parse_ps_json(raw: &str) -> Result<Vec<serde_json::Value>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(vec![]);
    }
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("Reponse PowerShell illisible (JSON) : {} — {}", e, trimmed))?;
    Ok(match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Null => vec![],
        other => vec![other],
    })
}

#[cfg(test)]
mod tests {
    use super::parse_ps_json;
    use serde_json::json;

    fn vide() -> Vec<serde_json::Value> {
        Vec::new()
    }

    #[test]
    fn chaine_vide_donne_liste_vide() {
        assert_eq!(parse_ps_json("").unwrap(), vide());
    }

    #[test]
    fn espaces_seuls_donnent_liste_vide() {
        assert_eq!(parse_ps_json("   \n\t ").unwrap(), vide());
    }

    #[test]
    fn null_donne_liste_vide() {
        // PowerShell peut serialiser une collection vide en 'null'.
        assert_eq!(parse_ps_json("null").unwrap(), vide());
    }

    #[test]
    fn tableau_vide_donne_liste_vide() {
        assert_eq!(parse_ps_json("[]").unwrap(), vide());
    }

    #[test]
    fn objet_unique_est_enveloppe_en_tableau() {
        // Cas critique : PowerShell 5.1 serialise un objet unique SANS crochets.
        let out = parse_ps_json(r#"{"nom":"svc","etat":"cible"}"#).unwrap();
        assert_eq!(out, vec![json!({"nom": "svc", "etat": "cible"})]);
    }

    #[test]
    fn tableau_de_deux_objets_est_preserve() {
        let out = parse_ps_json(r#"[{"a":1},{"b":2}]"#).unwrap();
        assert_eq!(out, vec![json!({"a": 1}), json!({"b": 2})]);
    }

    #[test]
    fn espaces_autour_du_json_sont_ignores() {
        let out = parse_ps_json("  \n {\"a\":1} \n ").unwrap();
        assert_eq!(out, vec![json!({"a": 1})]);
    }

    #[test]
    fn json_invalide_donne_erreur() {
        let err = parse_ps_json("pas du json").unwrap_err();
        assert!(err.contains("illisible"), "message inattendu : {err}");
    }
}
