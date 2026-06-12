use std::process::Command;

#[tauri::command]
pub fn create_restore_point() -> Result<String, String> {
    // In a real application, you would need admin privileges to do this.
    // This is a simplified example.
    let output = Command::new("powershell")
        .args(&[
            "-Command",
            "Checkpoint-Computer -Description 'TauriOptimizerBackup' -RestorePointType 'MODIFY_SETTINGS'",
        ])
        .output()
        .map_err(|e| format!("Failed to execute powershell: {}", e))?;

    if output.status.success() {
        Ok("Point de restauration créé avec succès.".to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Erreur lors de la création du point de restauration : {}", stderr))
    }
}
