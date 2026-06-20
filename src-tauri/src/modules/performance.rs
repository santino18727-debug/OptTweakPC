use crate::core::backup::POWERSHELL;
use std::process::Command;

#[tauri::command]
pub fn optimize_performance() -> Result<String, String> {
    // This is a placeholder for real performance optimizations.
    // E.g., setting the power plan to 'High Performance'
    let output = Command::new(POWERSHELL)
        .args(&[
            "-NoProfile",
            "-Command",
            "powercfg -setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c", // High performance power plan GUID
        ])
        .output()
        .map_err(|e| format!("Failed to execute powershell: {}", e))?;

    if output.status.success() {
        Ok("Performances optimisées (Mode Performances Élevées activé).".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Erreur lors de l'optimisation des performances : {}", stderr))
    }
}
