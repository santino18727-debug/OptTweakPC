use crate::core::backup::POWERSHELL;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;

/// Execute un script PowerShell et renvoie stdout (ou stderr en cas d'echec).
fn run_ps(script: &str) -> Result<String, String> {
    let output = Command::new(POWERSHELL)
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script])
        .output()
        .map_err(|e| format!("Echec d'execution de PowerShell : {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

/// Execute un script PowerShell qui produit du JSON et le normalise en tableau.
/// PowerShell 5.1 serialise un objet unique sans crochets : on tolere objet,
/// tableau ou sortie vide pour ne jamais casser cote front.
fn run_ps_json(script: &str) -> Result<Vec<serde_json::Value>, String> {
    let raw = run_ps(script)?;
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

/// Une entree du demarrage Windows.
#[derive(Serialize, Deserialize)]
pub struct StartupEntry {
    programme: String,
    emplacement: String,
}

/// Resultat (ou apercu) d'une operation de nettoyage du demarrage.
#[derive(Serialize, Deserialize, Clone)]
pub struct CleanupItem {
    nom: String,
    /// "service" ou "tache"
    categorie: String,
    /// "cible" (apercu), "modifie", "absent", "echec", "restaure"
    etat: String,
    detail: String,
    /// Etat precedent (avant modification) : sert a l'affichage avant/apres et
    /// a la restauration. Absent pour les elements non installes.
    #[serde(default)]
    ancien: Option<String>,
}

#[derive(Serialize)]
pub struct CleanupResult {
    items: Vec<CleanupItem>,
    /// Nombre d'elements reellement modifies (0 en mode apercu).
    modifies: usize,
    /// true si au moins un changement necessite un redemarrage.
    requires_reboot: bool,
}

/// Services d'updaters / monitoring non essentiels, candidats au passage en
/// "Manuel". 'chromoting' (Bureau a distance Chrome) est volontairement EXCLU.
/// Liste curee : les services absents de la machine sont simplement ignores
/// (rapportes "absent"), ce qui rend l'app portable d'un poste a l'autre.
fn service_candidates() -> &'static [&'static str] {
    &[
        "MTAgentService",
        "MTSchedulerService",
        "LGHUBUpdaterService",
        "gupdate",
        "gupdatem",
        "edgeupdate",
        "edgeupdatem",
        "BraveElevationService",
    ]
}

/// Taches planifiees de monitoring / updaters a desactiver.
fn task_candidates() -> &'static [&'static str] {
    &["GBTECService", "GraphicsCardEngine", "SIV", "SIV-VGA"]
}

/// Construit le script de nettoyage, en mode apercu (`apply = false`) ou
/// application reelle (`apply = true`). Les deux modes partagent EXACTEMENT la
/// meme liste de cibles : l'apercu ne peut donc pas mentir sur l'action reelle.
fn build_cleanup_script(apply: bool) -> String {
    let services = service_candidates()
        .iter()
        .map(|s| format!("'{}'", s))
        .collect::<Vec<_>>()
        .join(",");
    let tasks = task_candidates()
        .iter()
        .map(|t| format!("'{}'", t))
        .collect::<Vec<_>>()
        .join(",");
    let apply_lit = if apply { "$true" } else { "$false" };

    format!(
        r#"
        $apply = {apply}
        $services = @({services})
        $tasks = @({tasks})
        $items = @()

        foreach ($s in $services) {{
            $svc = Get-Service -Name $s -ErrorAction SilentlyContinue
            if (-not $svc) {{
                $items += [pscustomobject]@{{ nom=$s; categorie='service'; etat='absent'; detail='Service non installe sur ce poste'; ancien=$null }}
                continue
            }}
            $old = $svc.StartType.ToString()
            if (-not $apply) {{
                $items += [pscustomobject]@{{ nom=$s; categorie='service'; etat='cible'; detail='Sera passe en demarrage Manuel'; ancien=$old }}
                continue
            }}
            try {{
                Set-Service -Name $s -StartupType Manual -ErrorAction Stop
                $items += [pscustomobject]@{{ nom=$s; categorie='service'; etat='modifie'; detail='Passe en Manuel'; ancien=$old }}
            }} catch {{
                $items += [pscustomobject]@{{ nom=$s; categorie='service'; etat='echec'; detail='Echec (droits admin requis ?)'; ancien=$old }}
            }}
        }}

        foreach ($t in $tasks) {{
            $task = Get-ScheduledTask -TaskName $t -ErrorAction SilentlyContinue
            if (-not $task) {{
                $items += [pscustomobject]@{{ nom=$t; categorie='tache'; etat='absent'; detail='Tache introuvable sur ce poste'; ancien=$null }}
                continue
            }}
            $old = $task.State.ToString()
            if (-not $apply) {{
                $items += [pscustomobject]@{{ nom=$t; categorie='tache'; etat='cible'; detail='Sera desactivee'; ancien=$old }}
                continue
            }}
            try {{
                Disable-ScheduledTask -TaskName $t -TaskPath $task.TaskPath -ErrorAction Stop | Out-Null
                $items += [pscustomobject]@{{ nom=$t; categorie='tache'; etat='modifie'; detail='Desactivee'; ancien=$old }}
            }} catch {{
                $items += [pscustomobject]@{{ nom=$t; categorie='tache'; etat='echec'; detail='Echec'; ancien=$old }}
            }}
        }}

        ConvertTo-Json @($items) -Compress
        "#,
        apply = apply_lit,
        services = services,
        tasks = tasks,
    )
}

fn parse_cleanup(values: Vec<serde_json::Value>) -> CleanupResult {
    let items: Vec<CleanupItem> = values
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    let modifies = items.iter().filter(|i| i.etat == "modifie").count();
    CleanupResult {
        requires_reboot: modifies > 0,
        modifies,
        items,
    }
}

/// Audit : liste structuree des programmes lances au demarrage de Windows.
#[tauri::command]
pub fn audit_startup() -> Result<Vec<StartupEntry>, String> {
    let script = r#"
        Get-CimInstance Win32_StartupCommand |
            Select-Object @{n='programme';e={$_.Name}}, @{n='emplacement';e={$_.Location}} |
            Sort-Object programme | ConvertTo-Json -Compress
    "#;
    let values = run_ps_json(script)?;
    let entries: Vec<StartupEntry> = values
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    Ok(entries)
}

/// Apercu (dry-run) : ce que `clean_startup` modifierait, SANS rien modifier.
#[tauri::command]
pub fn preview_startup_cleanup() -> Result<CleanupResult, String> {
    let values = run_ps_json(&build_cleanup_script(false))?;
    Ok(parse_cleanup(values))
}

/// Emplacement du snapshot d'etat (pour l'annulation) :
/// %LOCALAPPDATA%\OptTweakPC\startup_snapshot.json
fn snapshot_path() -> Result<PathBuf, String> {
    let base = std::env::var("LOCALAPPDATA")
        .map_err(|_| "Variable LOCALAPPDATA introuvable.".to_string())?;
    let mut p = PathBuf::from(base);
    p.push("OptTweakPC");
    std::fs::create_dir_all(&p).map_err(|e| format!("Creation du dossier impossible : {}", e))?;
    p.push("startup_snapshot.json");
    Ok(p)
}

/// Clean : allege le demarrage (services updaters/monitoring -> Manuel, taches
/// planifiees non essentielles -> desactivees). 'chromoting' est EXCLU.
/// Requiert les droits administrateur. Sauvegarde l'etat AVANT modification
/// dans un snapshot pour permettre l'annulation. Renvoie un rapport structure.
#[tauri::command]
pub fn clean_startup() -> Result<CleanupResult, String> {
    let values = run_ps_json(&build_cleanup_script(true))?;
    let result = parse_cleanup(values);

    // Sauvegarde des elements reellement modifies pour pouvoir les restaurer.
    let modified: Vec<&CleanupItem> = result
        .items
        .iter()
        .filter(|i| i.etat == "modifie" && i.ancien.is_some())
        .collect();
    if !modified.is_empty() {
        if let Ok(path) = snapshot_path() {
            if let Ok(json) = serde_json::to_string_pretty(&modified) {
                let _ = std::fs::write(path, json); // best-effort : n'echoue pas le clean
            }
        }
    }

    Ok(result)
}

/// Indique si une sauvegarde restaurable existe.
#[tauri::command]
pub fn has_startup_snapshot() -> Result<bool, String> {
    Ok(snapshot_path().map(|p| p.exists()).unwrap_or(false))
}

/// Annule un nettoyage precedent : restaure le type de demarrage d'origine des
/// services et reactive les taches qui etaient actives. Lit le snapshot ecrit
/// par `clean_startup`. Requiert les droits administrateur.
#[tauri::command]
pub fn restore_startup() -> Result<CleanupResult, String> {
    let path = snapshot_path()?;
    if !path.exists() {
        return Err("Aucune sauvegarde a restaurer.".to_string());
    }
    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Lecture du snapshot : {}", e))?;
    let saved: Vec<CleanupItem> =
        serde_json::from_str(&content).map_err(|e| format!("Snapshot illisible : {}", e))?;

    // Construit dynamiquement le script de restauration depuis le snapshot.
    let mut blocks = String::from("$items = @()\n");
    for it in &saved {
        let ancien = it.ancien.clone().unwrap_or_default();
        if it.categorie == "service" {
            blocks.push_str(&format!(
                r#"
                try {{
                    Set-Service -Name '{nom}' -StartupType {ancien} -ErrorAction Stop
                    $items += [pscustomobject]@{{ nom='{nom}'; categorie='service'; etat='restaure'; detail='Restaure en {ancien}'; ancien='{ancien}' }}
                }} catch {{
                    $items += [pscustomobject]@{{ nom='{nom}'; categorie='service'; etat='echec'; detail='Echec de restauration'; ancien='{ancien}' }}
                }}
                "#,
                nom = it.nom,
                ancien = ancien,
            ));
        } else if it.categorie == "tache" {
            // On ne reactive que les taches qui etaient actives ('Ready').
            if ancien.eq_ignore_ascii_case("Ready") {
                blocks.push_str(&format!(
                    r#"
                    $tk = Get-ScheduledTask -TaskName '{nom}' -ErrorAction SilentlyContinue
                    if ($tk) {{
                        try {{
                            Enable-ScheduledTask -TaskName '{nom}' -TaskPath $tk.TaskPath -ErrorAction Stop | Out-Null
                            $items += [pscustomobject]@{{ nom='{nom}'; categorie='tache'; etat='restaure'; detail='Reactivee'; ancien='{ancien}' }}
                        }} catch {{
                            $items += [pscustomobject]@{{ nom='{nom}'; categorie='tache'; etat='echec'; detail='Echec de restauration'; ancien='{ancien}' }}
                        }}
                    }}
                    "#,
                    nom = it.nom,
                    ancien = ancien,
                ));
            }
        }
    }
    blocks.push_str("\nConvertTo-Json @($items) -Compress\n");

    let values = run_ps_json(&blocks)?;
    let result = parse_cleanup_restored(values);

    // Le snapshot est consomme : on l'efface pour eviter une double restauration.
    let _ = std::fs::remove_file(&path);
    Ok(result)
}

fn parse_cleanup_restored(values: Vec<serde_json::Value>) -> CleanupResult {
    let items: Vec<CleanupItem> = values
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    let modifies = items.iter().filter(|i| i.etat == "restaure").count();
    CleanupResult {
        requires_reboot: modifies > 0,
        modifies,
        items,
    }
}
