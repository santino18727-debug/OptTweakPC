use crate::core::ps;
use serde::{Deserialize, Serialize};

/// Indique si le processus courant dispose des droits administrateur.
/// Permet au frontend d'avertir l'utilisateur AVANT toute action qui en a
/// besoin (clean_startup, optimize_performance), plutot que d'echouer avec un
/// message systeme cryptique en anglais.
#[tauri::command]
pub fn check_admin_rights() -> Result<bool, String> {
    let stdout = ps::run_ps(
        "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)",
    )?;
    Ok(interpret_admin_output(&stdout))
}

/// Interprete la sortie de la commande de verification admin. Extrait pour
/// etre testable sans lancer PowerShell : PowerShell ecrit 'True'/'False'
/// (souvent suivi d'un CRLF), on trim et on tolere la casse.
fn interpret_admin_output(raw: &str) -> bool {
    raw.trim().eq_ignore_ascii_case("True")
}

/// Contexte systeme leger affiche en tete d'application.
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SystemInfo {
    os: String,
    cpu: String,
    ram_total_gb: f64,
    ram_free_gb: f64,
    power_plan: String,
    startup_count: u32,
}

/// Quelques informations read-only pour contextualiser les optimisations
/// (aucune modification systeme). Version volontairement minimale.
#[tauri::command]
pub fn get_system_info() -> Result<SystemInfo, String> {
    let script = r#"
        $os = Get-CimInstance Win32_OperatingSystem
        $cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name
        $plan = ((powercfg /getactivescheme) -replace '.*\((.*)\).*','$1').Trim()
        $startup = @(Get-CimInstance Win32_StartupCommand).Count
        [pscustomobject]@{
            os           = $os.Caption
            cpu          = $cpu
            ram_total_gb = [math]::Round($os.TotalVisibleMemorySize / 1MB, 1)
            ram_free_gb  = [math]::Round($os.FreePhysicalMemory / 1MB, 1)
            power_plan   = $plan
            startup_count = $startup
        } | ConvertTo-Json -Compress
    "#;
    parse_system_info(&ps::run_ps(script)?)
}

/// Deserialise la sortie JSON des infos systeme en `SystemInfo`. Extrait de
/// `get_system_info` pour etre testable sans lancer PowerShell.
fn parse_system_info(raw: &str) -> Result<SystemInfo, String> {
    serde_json::from_str(raw.trim()).map_err(|e| {
        format!(
            "Lecture des infos systeme impossible : {} — {}",
            e,
            raw.trim()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::{interpret_admin_output, parse_system_info, SystemInfo};

    #[test]
    fn admin_vrai_toutes_casses_et_espaces() {
        assert!(interpret_admin_output("True"));
        assert!(interpret_admin_output("true"));
        assert!(interpret_admin_output("TRUE"));
        // PowerShell renvoie typiquement la valeur suivie d'un CRLF.
        assert!(interpret_admin_output("  True\r\n"));
    }

    #[test]
    fn admin_faux_vide_ou_autre() {
        assert!(!interpret_admin_output("False"));
        assert!(!interpret_admin_output(""));
        assert!(!interpret_admin_output("   \n"));
        assert!(!interpret_admin_output("Yes"));
    }

    #[test]
    fn system_info_json_valide_est_deserialise() {
        let raw = r#"{"os":"Windows 11","cpu":"Ryzen 7","ram_total_gb":32.0,"ram_free_gb":12.5,"power_plan":"Equilibre","startup_count":7}"#;
        let info = parse_system_info(raw).unwrap();
        assert_eq!(
            info,
            SystemInfo {
                os: "Windows 11".into(),
                cpu: "Ryzen 7".into(),
                ram_total_gb: 32.0,
                ram_free_gb: 12.5,
                power_plan: "Equilibre".into(),
                startup_count: 7,
            }
        );
    }

    #[test]
    fn system_info_espaces_autour_sont_ignores() {
        let raw = "  \n {\"os\":\"W\",\"cpu\":\"C\",\"ram_total_gb\":1.0,\"ram_free_gb\":0.5,\"power_plan\":\"P\",\"startup_count\":0} \n ";
        assert!(parse_system_info(raw).is_ok());
    }

    #[test]
    fn system_info_json_invalide_donne_erreur() {
        let err = parse_system_info("pas du json").unwrap_err();
        assert!(err.contains("impossible"), "message inattendu : {err}");
    }

    #[test]
    fn system_info_champ_manquant_donne_erreur() {
        // 'cpu' absent -> la deserialisation doit echouer.
        let raw = r#"{"os":"W","ram_total_gb":1.0,"ram_free_gb":0.5,"power_plan":"P","startup_count":0}"#;
        assert!(parse_system_info(raw).is_err());
    }
}
