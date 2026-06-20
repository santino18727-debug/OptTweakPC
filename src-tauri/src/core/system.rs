use crate::core::backup::POWERSHELL;
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Execute un script PowerShell et renvoie stdout (ou stderr en cas d'echec).
fn run_ps(script: &str) -> Result<String, String> {
    let output = Command::new(POWERSHELL)
        .args(["-NoProfile", "-Command", script])
        .output()
        .map_err(|e| format!("Echec d'execution de PowerShell : {}", e))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Indique si le processus courant dispose des droits administrateur.
/// Permet au frontend d'avertir l'utilisateur AVANT toute action qui en a
/// besoin (clean_startup, optimize_performance), plutot que d'echouer avec un
/// message systeme cryptique en anglais.
#[tauri::command]
pub fn check_admin_rights() -> Result<bool, String> {
    let stdout = run_ps(
        "([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)",
    )?;
    Ok(stdout.trim().eq_ignore_ascii_case("True"))
}

/// Contexte systeme leger affiche en tete d'application.
#[derive(Serialize, Deserialize)]
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
    let raw = run_ps(script)?;
    serde_json::from_str(raw.trim()).map_err(|e| {
        format!(
            "Lecture des infos systeme impossible : {} — {}",
            e,
            raw.trim()
        )
    })
}
