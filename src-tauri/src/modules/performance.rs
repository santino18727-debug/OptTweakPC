use crate::core::ps;

#[tauri::command]
pub fn optimize_performance() -> Result<String, String> {
    // Bascule le plan d'alimentation sur "Performances elevees" (GUID Windows standard).
    ps::run_ps("powercfg -setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c")
        .map(|_| "Performances optimisées (Mode Performances Élevées activé).".to_string())
        .map_err(|stderr| {
            format!(
                "Erreur lors de l'optimisation des performances : {}",
                stderr
            )
        })
}
