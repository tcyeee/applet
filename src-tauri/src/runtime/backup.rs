//! Backup and restore: an app's registry record (including its full
//! definition), its SQLite database, and its private files, packed into a
//! single portable `.zip` under `backups/<app_id>/`.

use std::fmt;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::Connection;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::runtime::paths;
use crate::runtime::registry::{self, AppRecord, RegistryError};

#[derive(Debug)]
pub enum BackupError {
    NotFound(String),
    AlreadyInstalled(String),
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Serde(serde_json::Error),
    Registry(RegistryError),
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupError::NotFound(id) => write!(f, "app \"{id}\" is not installed"),
            BackupError::AlreadyInstalled(id) => write!(
                f,
                "app \"{id}\" is already installed; pass overwrite=true to restore over it"
            ),
            BackupError::Io(e) => write!(f, "backup io error: {e}"),
            BackupError::Zip(e) => write!(f, "backup archive error: {e}"),
            BackupError::Serde(e) => write!(f, "backup manifest error: {e}"),
            BackupError::Registry(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for BackupError {}
impl From<std::io::Error> for BackupError {
    fn from(e: std::io::Error) -> Self {
        BackupError::Io(e)
    }
}
impl From<zip::result::ZipError> for BackupError {
    fn from(e: zip::result::ZipError) -> Self {
        BackupError::Zip(e)
    }
}
impl From<serde_json::Error> for BackupError {
    fn from(e: serde_json::Error) -> Self {
        BackupError::Serde(e)
    }
}
impl From<RegistryError> for BackupError {
    fn from(e: RegistryError) -> Self {
        BackupError::Registry(e)
    }
}

pub fn backup_app(
    base_dir: &Path,
    conn: &Connection,
    app_id: &str,
) -> Result<PathBuf, BackupError> {
    let record =
        registry::get(conn, app_id)?.ok_or_else(|| BackupError::NotFound(app_id.to_string()))?;

    let backups_dir = paths::backups_dir(base_dir, app_id);
    std::fs::create_dir_all(&backups_dir)?;
    let timestamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ").to_string();
    let archive_path = backups_dir.join(format!("{timestamp}.zip"));

    let file = std::fs::File::create(&archive_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    zip.start_file("manifest.json", options)?;
    zip.write_all(serde_json::to_string_pretty(&record)?.as_bytes())?;

    let db_path = paths::app_db_path(base_dir, app_id);
    if db_path.exists() {
        zip.start_file("data.sqlite", options)?;
        zip.write_all(&std::fs::read(&db_path)?)?;
    }

    let files_dir = paths::app_files_dir(base_dir, app_id);
    if files_dir.exists() {
        for entry in WalkDir::new(&files_dir).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&files_dir)
                .unwrap_or(entry.path());
            let name = format!(
                "files/{}",
                rel.to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            );
            zip.start_file(name, options)?;
            zip.write_all(&std::fs::read(entry.path())?)?;
        }
    }

    zip.finish()?;
    Ok(archive_path)
}

/// Backs up every installed app in one pass ("full backup").
pub fn backup_all(base_dir: &Path, conn: &Connection) -> Result<Vec<PathBuf>, BackupError> {
    registry::list(conn)?
        .iter()
        .map(|record| backup_app(base_dir, conn, &record.id))
        .collect()
}

pub struct BackupInfo {
    pub path: PathBuf,
    pub created_at: String,
}

pub fn list_backups(base_dir: &Path, app_id: &str) -> Result<Vec<BackupInfo>, BackupError> {
    let dir = paths::backups_dir(base_dir, app_id);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|e| e == "zip") {
            let created_at = entry
                .path()
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            out.push(BackupInfo {
                path: entry.path(),
                created_at,
            });
        }
    }
    out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    Ok(out)
}

/// Restores an app from a backup archive, re-registering it and writing back
/// its database and files. Refuses to clobber a currently-installed app
/// unless `overwrite` is set.
pub fn restore_app(
    base_dir: &Path,
    conn: &Connection,
    archive_path: &Path,
    overwrite: bool,
) -> Result<AppRecord, BackupError> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)?;

    let record: AppRecord = {
        let mut entry = archive.by_name("manifest.json")?;
        let mut buf = String::new();
        entry.read_to_string(&mut buf)?;
        serde_json::from_str(&buf)?
    };

    if !overwrite && registry::get(conn, &record.id)?.is_some() {
        return Err(BackupError::AlreadyInstalled(record.id.clone()));
    }

    let app_dir = paths::app_dir(base_dir, &record.id);
    std::fs::create_dir_all(&app_dir)?;

    if let Ok(mut entry) = archive.by_name("data.sqlite") {
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        std::fs::write(paths::app_db_path(base_dir, &record.id), buf)?;
    }

    let files_dir = paths::app_files_dir(base_dir, &record.id);
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let Ok(rel) = name.strip_prefix("files") else {
            continue;
        };
        if rel.as_os_str().is_empty() {
            continue;
        }
        let dest = files_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        std::fs::write(dest, buf)?;
    }

    registry::upsert_record(conn, &record)?;
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{appdb, registry};

    fn sample_definition(id: &str) -> crate::app_schema::AppDefinition {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": "1",
            "id": id,
            "name": "Test App",
            "dataModel": { "entities": [
                { "id": "item", "name": "Item", "fields": [
                    { "id": "title", "type": "string", "required": true }
                ] }
            ] },
            "views": [],
            "actions": [],
            "automations": []
        }))
        .unwrap()
    }

    #[test]
    fn backup_then_restore_round_trip() {
        let base = tempfile::tempdir().unwrap();
        let base_dir = base.path();
        let conn = Connection::open(paths::registry_db_path(base_dir)).unwrap();
        registry::init_schema(&conn).unwrap();

        let def = sample_definition("bk-app");
        registry::install(&conn, &def).unwrap();
        std::fs::create_dir_all(paths::app_dir(base_dir, "bk-app")).unwrap();
        let db_conn = appdb::open(&paths::app_db_path(base_dir, "bk-app")).unwrap();
        appdb::create_schema(&db_conn, &def.data_model).unwrap();
        db_conn
            .execute("INSERT INTO item (id, createdAt, updatedAt, title) VALUES ('1','now','now','Widget')", [])
            .unwrap();
        drop(db_conn);
        storage_write(base_dir, "bk-app", "notes.txt", b"hello");

        let archive = backup_app(base_dir, &conn, "bk-app").unwrap();
        assert!(archive.exists());

        // Simulate uninstall with data purge, then restore from the archive.
        registry::remove(&conn, "bk-app").unwrap();
        std::fs::remove_dir_all(paths::app_dir(base_dir, "bk-app")).unwrap();

        let restored = restore_app(base_dir, &conn, &archive, false).unwrap();
        assert_eq!(restored.id, "bk-app");
        assert_eq!(restored.definition.data_model.entities.len(), 1);

        let restored_db = Connection::open(paths::app_db_path(base_dir, "bk-app")).unwrap();
        let title: String = restored_db
            .query_row("SELECT title FROM item WHERE id='1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Widget");

        let notes =
            std::fs::read(paths::app_files_dir(base_dir, "bk-app").join("notes.txt")).unwrap();
        assert_eq!(notes, b"hello");

        assert!(registry::get(&conn, "bk-app").unwrap().is_some());
    }

    #[test]
    fn restore_refuses_to_overwrite_installed_app_without_flag() {
        let base = tempfile::tempdir().unwrap();
        let base_dir = base.path();
        let conn = Connection::open(paths::registry_db_path(base_dir)).unwrap();
        registry::init_schema(&conn).unwrap();

        let def = sample_definition("bk-app-2");
        registry::install(&conn, &def).unwrap();
        std::fs::create_dir_all(paths::app_dir(base_dir, "bk-app-2")).unwrap();
        let db_conn = appdb::open(&paths::app_db_path(base_dir, "bk-app-2")).unwrap();
        appdb::create_schema(&db_conn, &def.data_model).unwrap();
        drop(db_conn);

        let archive = backup_app(base_dir, &conn, "bk-app-2").unwrap();

        let err = restore_app(base_dir, &conn, &archive, false).unwrap_err();
        assert!(matches!(err, BackupError::AlreadyInstalled(_)));

        // With overwrite=true it should succeed.
        restore_app(base_dir, &conn, &archive, true).unwrap();
    }

    fn storage_write(base_dir: &Path, app_id: &str, relative: &str, contents: &[u8]) {
        let root = paths::app_files_dir(base_dir, app_id);
        crate::runtime::storage::write_file(&root, relative, contents).unwrap();
    }

    #[test]
    fn backup_all_backs_up_every_installed_app() {
        let base = tempfile::tempdir().unwrap();
        let base_dir = base.path();
        let conn = Connection::open(paths::registry_db_path(base_dir)).unwrap();
        registry::init_schema(&conn).unwrap();

        for id in ["one", "two"] {
            let def = sample_definition(id);
            registry::install(&conn, &def).unwrap();
            let db_conn = appdb::open(&paths::app_db_path(base_dir, id)).unwrap();
            appdb::create_schema(&db_conn, &def.data_model).unwrap();
        }

        let archives = backup_all(base_dir, &conn).unwrap();
        assert_eq!(archives.len(), 2);
        assert_eq!(list_backups(base_dir, "one").unwrap().len(), 1);
        assert_eq!(list_backups(base_dir, "two").unwrap().len(), 1);
    }
}
