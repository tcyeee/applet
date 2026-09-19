//! Tauri commands exposing the Runtime Core to the frontend.
//!
//! Every command that accepts an `AppDefinition` trusts that the caller
//! already ran it through `validateAppDefinition` (TS, `src/app-schema/validate.ts`)
//! — see `AGENTS.md` and the doc comment on `app_schema.rs`. Errors are
//! returned as `String` (the Tauri command convention) built from the
//! underlying error types' `Display` impl.

use tauri::{AppHandle, State};

use crate::app_schema::AppDefinition;
use crate::runtime::backup::{self, BackupInfo};
use crate::runtime::registry::{self, AppRecord, AppStatus};
use crate::runtime::scheduler::Scheduler;
use crate::runtime::{appdb, paths, storage, RuntimeState};

fn app_dir_ensured(state: &RuntimeState, app_id: &str) -> Result<std::path::PathBuf, String> {
    let dir = paths::app_dir(&state.base_dir, app_id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[tauri::command]
pub fn install_app(
    state: State<RuntimeState>,
    definition: AppDefinition,
) -> Result<AppRecord, String> {
    let conn = state.registry_conn.lock().unwrap();
    let record = registry::install(&conn, &definition).map_err(|e| e.to_string())?;

    app_dir_ensured(&state, &definition.id)?;
    let db_path = paths::app_db_path(&state.base_dir, &definition.id);
    let db_conn = appdb::open(&db_path).map_err(|e| e.to_string())?;
    appdb::create_schema(&db_conn, &definition.data_model).map_err(|e| e.to_string())?;

    Ok(record)
}

#[tauri::command]
pub fn list_apps(state: State<RuntimeState>) -> Result<Vec<AppRecord>, String> {
    let conn = state.registry_conn.lock().unwrap();
    registry::list(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_app(state: State<RuntimeState>, app_id: String) -> Result<AppRecord, String> {
    let conn = state.registry_conn.lock().unwrap();
    registry::get(&conn, &app_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("app \"{app_id}\" is not installed"))
}

/// Applies a new definition to an installed app, migrating its database.
/// Returns an error (without changing anything) if the migration would drop
/// or reinterpret existing data, unless `force` is set.
#[tauri::command]
pub fn update_app(
    state: State<RuntimeState>,
    app_id: String,
    definition: AppDefinition,
    force: bool,
) -> Result<AppRecord, String> {
    let conn = state.registry_conn.lock().unwrap();
    let existing = registry::get(&conn, &app_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("app \"{app_id}\" is not installed"))?;

    let db_path = paths::app_db_path(&state.base_dir, &app_id);
    let mut db_conn = appdb::open(&db_path).map_err(|e| e.to_string())?;
    appdb::migrate_schema(
        &mut db_conn,
        &existing.definition.data_model,
        &definition.data_model,
        force,
    )
    .map_err(|e| e.to_string())?;

    registry::update_definition(&conn, &app_id, &definition).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_app(state: State<RuntimeState>, app_id: String) -> Result<AppRecord, String> {
    let conn = state.registry_conn.lock().unwrap();
    registry::set_status(&conn, &app_id, AppStatus::Running).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_app(state: State<RuntimeState>, app_id: String) -> Result<AppRecord, String> {
    let conn = state.registry_conn.lock().unwrap();
    registry::set_status(&conn, &app_id, AppStatus::Stopped).map_err(|e| e.to_string())
}

/// Uninstalls an app. When `purge_data` is false (the default a caller
/// should prefer), the app's database and files are left on disk so the
/// uninstall is recoverable; only the registry entry is removed.
#[tauri::command]
pub fn uninstall_app(
    state: State<RuntimeState>,
    app_id: String,
    purge_data: bool,
) -> Result<(), String> {
    let conn = state.registry_conn.lock().unwrap();
    registry::remove(&conn, &app_id).map_err(|e| e.to_string())?;
    if purge_data {
        let dir = paths::app_dir(&state.base_dir, &app_id);
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn write_app_file(
    state: State<RuntimeState>,
    app_id: String,
    path: String,
    contents: Vec<u8>,
) -> Result<(), String> {
    let root =
        app_dir_ensured(&state, &app_id).map(|_| paths::app_files_dir(&state.base_dir, &app_id))?;
    storage::write_file(&root, &path, &contents).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_app_file(
    state: State<RuntimeState>,
    app_id: String,
    path: String,
) -> Result<Vec<u8>, String> {
    let root = paths::app_files_dir(&state.base_dir, &app_id);
    storage::read_file(&root, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_app_file(
    state: State<RuntimeState>,
    app_id: String,
    path: String,
) -> Result<(), String> {
    let root = paths::app_files_dir(&state.base_dir, &app_id);
    storage::delete_file(&root, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_app_files(state: State<RuntimeState>, app_id: String) -> Result<Vec<String>, String> {
    let root = paths::app_files_dir(&state.base_dir, &app_id);
    storage::list_files(&root).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_shared_file(
    state: State<RuntimeState>,
    path: String,
    contents: Vec<u8>,
) -> Result<(), String> {
    let root = paths::shared_dir(&state.base_dir);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    storage::write_file(&root, &path, &contents).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_shared_file(state: State<RuntimeState>, path: String) -> Result<Vec<u8>, String> {
    let root = paths::shared_dir(&state.base_dir);
    storage::read_file(&root, &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_shared_files(state: State<RuntimeState>) -> Result<Vec<String>, String> {
    let root = paths::shared_dir(&state.base_dir);
    storage::list_files(&root).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn backup_app(state: State<RuntimeState>, app_id: String) -> Result<String, String> {
    let conn = state.registry_conn.lock().unwrap();
    backup::backup_app(&state.base_dir, &conn, &app_id)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct BackupSummary {
    path: String,
    #[serde(rename = "createdAt")]
    created_at: String,
}
impl From<BackupInfo> for BackupSummary {
    fn from(info: BackupInfo) -> Self {
        Self {
            path: info.path.to_string_lossy().to_string(),
            created_at: info.created_at,
        }
    }
}

#[tauri::command]
pub fn backup_all_apps(state: State<RuntimeState>) -> Result<Vec<String>, String> {
    let conn = state.registry_conn.lock().unwrap();
    backup::backup_all(&state.base_dir, &conn)
        .map(|paths| {
            paths
                .into_iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect()
        })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_backups(
    state: State<RuntimeState>,
    app_id: String,
) -> Result<Vec<BackupSummary>, String> {
    backup::list_backups(&state.base_dir, &app_id)
        .map(|v| v.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restore_app(
    state: State<RuntimeState>,
    archive_path: String,
    overwrite: bool,
) -> Result<AppRecord, String> {
    let conn = state.registry_conn.lock().unwrap();
    backup::restore_app(
        &state.base_dir,
        &conn,
        std::path::Path::new(&archive_path),
        overwrite,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn start_scheduler(app_handle: AppHandle, state: State<RuntimeState>) -> Result<(), String> {
    let mut guard = state.scheduler.lock().unwrap();
    if guard.is_some() {
        return Ok(());
    }
    *guard = Some(Scheduler::spawn(app_handle, state.base_dir.clone()));
    Ok(())
}

#[tauri::command]
pub fn stop_scheduler(state: State<RuntimeState>) -> Result<(), String> {
    let mut guard = state.scheduler.lock().unwrap();
    if let Some(mut scheduler) = guard.take() {
        scheduler.stop();
    }
    Ok(())
}
