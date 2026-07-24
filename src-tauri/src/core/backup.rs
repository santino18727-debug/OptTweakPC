use crate::core::ps;

/// Cree un point de restauration systeme (Checkpoint-Computer). Requiert les
/// droits administrateur, que l'app obtient via son manifeste UAC.
#[tauri::command]
pub fn create_restore_point() -> Result<String, String> {
    ps::run_ps(
        "Checkpoint-Computer -Description 'TauriOptimizerBackup' -RestorePointType 'MODIFY_SETTINGS'",
    )
    .map(|_| "Point de restauration créé avec succès.".to_string())
    .map_err(|stderr| {
        format!(
            "Erreur lors de la création du point de restauration : {}",
            stderr
        )
    })
}
