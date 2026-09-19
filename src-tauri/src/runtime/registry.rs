//! App Registry: metadata for locally installed Apps (version/revision,
//! status, the validated definition itself). Backed by its own SQLite
//! database (`registry.sqlite`), separate from any app's own data.

use std::fmt;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::app_schema::AppDefinition;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AppStatus {
    Stopped,
    Running,
}

impl AppStatus {
    fn as_str(&self) -> &'static str {
        match self {
            AppStatus::Stopped => "stopped",
            AppStatus::Running => "running",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "running" => AppStatus::Running,
            _ => AppStatus::Stopped,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppRecord {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub definition: AppDefinition,
    pub status: AppStatus,
    pub revision: i64,
    #[serde(rename = "installedAt")]
    pub installed_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Debug)]
pub enum RegistryError {
    NotFound(String),
    AlreadyInstalled(String),
    IdMismatch { expected: String, actual: String },
    UnsupportedSchemaVersion(String),
    Db(rusqlite::Error),
    Serde(serde_json::Error),
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::NotFound(id) => write!(f, "app \"{id}\" is not installed"),
            RegistryError::AlreadyInstalled(id) => write!(f, "app \"{id}\" is already installed"),
            RegistryError::IdMismatch { expected, actual } => {
                write!(
                    f,
                    "definition id \"{actual}\" does not match target app \"{expected}\""
                )
            }
            RegistryError::UnsupportedSchemaVersion(v) => {
                write!(f, "unsupported schemaVersion \"{v}\"")
            }
            RegistryError::Db(e) => write!(f, "registry database error: {e}"),
            RegistryError::Serde(e) => write!(f, "failed to (de)serialize app definition: {e}"),
        }
    }
}

impl std::error::Error for RegistryError {}
impl From<rusqlite::Error> for RegistryError {
    fn from(e: rusqlite::Error) -> Self {
        RegistryError::Db(e)
    }
}
impl From<serde_json::Error> for RegistryError {
    fn from(e: serde_json::Error) -> Self {
        RegistryError::Serde(e)
    }
}

pub fn init_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS apps (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            schema_version TEXT NOT NULL,
            definition_json TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'stopped',
            revision INTEGER NOT NULL DEFAULT 1,
            installed_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );",
    )
}

fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<AppRecord> {
    let definition_json: String = row.get("definition_json")?;
    let definition: AppDefinition = serde_json::from_str(&definition_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(AppRecord {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        schema_version: row.get("schema_version")?,
        definition,
        status: AppStatus::parse(&row.get::<_, String>("status")?),
        revision: row.get("revision")?,
        installed_at: row.get("installed_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Registers a new App from an already-validated definition (the caller must
/// have run it through `validateAppDefinition` on the TS side first).
pub fn install(conn: &Connection, definition: &AppDefinition) -> Result<AppRecord, RegistryError> {
    if get(conn, &definition.id)?.is_some() {
        return Err(RegistryError::AlreadyInstalled(definition.id.clone()));
    }
    let now = Utc::now().to_rfc3339();
    let definition_json = serde_json::to_string(definition)?;
    conn.execute(
        "INSERT INTO apps (id, name, description, schema_version, definition_json, status, revision, installed_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'stopped', 1, ?6, ?6)",
        params![
            definition.id,
            definition.name,
            definition.description,
            definition.schema_version,
            definition_json,
            now,
        ],
    )?;
    get(conn, &definition.id)?.ok_or_else(|| RegistryError::NotFound(definition.id.clone()))
}

pub fn get(conn: &Connection, app_id: &str) -> Result<Option<AppRecord>, RegistryError> {
    conn.query_row(
        "SELECT * FROM apps WHERE id = ?1",
        params![app_id],
        row_to_record,
    )
    .optional()
    .map_err(RegistryError::from)
}

pub fn list(conn: &Connection) -> Result<Vec<AppRecord>, RegistryError> {
    let mut stmt = conn.prepare("SELECT * FROM apps ORDER BY installed_at ASC")?;
    let rows = stmt.query_map([], row_to_record)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(RegistryError::from)
}

pub fn set_status(
    conn: &Connection,
    app_id: &str,
    status: AppStatus,
) -> Result<AppRecord, RegistryError> {
    let existing = get(conn, app_id)?.ok_or_else(|| RegistryError::NotFound(app_id.to_string()))?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE apps SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status.as_str(), now, app_id],
    )?;
    let _ = existing;
    get(conn, app_id)?.ok_or_else(|| RegistryError::NotFound(app_id.to_string()))
}

/// Replaces an installed app's definition, e.g. after a caller-approved data
/// migration. Only same-`schemaVersion` updates are supported for now — a
/// version bump requires the new version's own validation path to exist
/// first (see `AGENTS.md` versioning note).
pub fn update_definition(
    conn: &Connection,
    app_id: &str,
    new_definition: &AppDefinition,
) -> Result<AppRecord, RegistryError> {
    if new_definition.id != app_id {
        return Err(RegistryError::IdMismatch {
            expected: app_id.to_string(),
            actual: new_definition.id.clone(),
        });
    }
    let existing = get(conn, app_id)?.ok_or_else(|| RegistryError::NotFound(app_id.to_string()))?;
    if new_definition.schema_version != existing.schema_version {
        return Err(RegistryError::UnsupportedSchemaVersion(
            new_definition.schema_version.clone(),
        ));
    }
    let now = Utc::now().to_rfc3339();
    let definition_json = serde_json::to_string(new_definition)?;
    conn.execute(
        "UPDATE apps SET name = ?1, description = ?2, definition_json = ?3, revision = revision + 1, updated_at = ?4
         WHERE id = ?5",
        params![new_definition.name, new_definition.description, definition_json, now, app_id],
    )?;
    get(conn, app_id)?.ok_or_else(|| RegistryError::NotFound(app_id.to_string()))
}

/// Re-inserts a record as-is, preserving its original `installed_at`/`revision`.
/// Used by restore-from-backup.
pub fn upsert_record(conn: &Connection, record: &AppRecord) -> Result<(), RegistryError> {
    let definition_json = serde_json::to_string(&record.definition)?;
    conn.execute(
        "INSERT INTO apps (id, name, description, schema_version, definition_json, status, revision, installed_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            schema_version = excluded.schema_version,
            definition_json = excluded.definition_json,
            status = excluded.status,
            revision = excluded.revision,
            installed_at = excluded.installed_at,
            updated_at = excluded.updated_at",
        params![
            record.id,
            record.name,
            record.description,
            record.schema_version,
            definition_json,
            record.status.as_str(),
            record.revision,
            record.installed_at,
            record.updated_at,
        ],
    )?;
    Ok(())
}

pub fn remove(conn: &Connection, app_id: &str) -> Result<(), RegistryError> {
    let affected = conn.execute("DELETE FROM apps WHERE id = ?1", params![app_id])?;
    if affected == 0 {
        return Err(RegistryError::NotFound(app_id.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_definition(id: &str) -> AppDefinition {
        serde_json::from_value(serde_json::json!({
            "schemaVersion": "1",
            "id": id,
            "name": "Test App",
            "dataModel": {
                "entities": [
                    { "id": "item", "name": "Item", "fields": [
                        { "id": "title", "type": "string", "required": true }
                    ] }
                ]
            },
            "views": [],
            "actions": [],
            "automations": []
        }))
        .unwrap()
    }

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn install_and_get_roundtrip() {
        let conn = conn();
        let def = sample_definition("my-app");
        let record = install(&conn, &def).unwrap();
        assert_eq!(record.status, AppStatus::Stopped);
        assert_eq!(record.revision, 1);

        let fetched = get(&conn, "my-app").unwrap().unwrap();
        assert_eq!(fetched.id, "my-app");
        assert_eq!(fetched.definition.data_model.entities.len(), 1);
    }

    #[test]
    fn install_twice_fails() {
        let conn = conn();
        let def = sample_definition("dup-app");
        install(&conn, &def).unwrap();
        let err = install(&conn, &def).unwrap_err();
        assert!(matches!(err, RegistryError::AlreadyInstalled(id) if id == "dup-app"));
    }

    #[test]
    fn list_returns_all_installed() {
        let conn = conn();
        install(&conn, &sample_definition("a")).unwrap();
        install(&conn, &sample_definition("b")).unwrap();
        let apps = list(&conn).unwrap();
        assert_eq!(apps.len(), 2);
    }

    #[test]
    fn set_status_updates_and_rejects_unknown_app() {
        let conn = conn();
        install(&conn, &sample_definition("app")).unwrap();
        let record = set_status(&conn, "app", AppStatus::Running).unwrap();
        assert_eq!(record.status, AppStatus::Running);

        let err = set_status(&conn, "missing", AppStatus::Running).unwrap_err();
        assert!(matches!(err, RegistryError::NotFound(_)));
    }

    #[test]
    fn update_definition_rejects_id_mismatch() {
        let conn = conn();
        install(&conn, &sample_definition("app")).unwrap();
        let other = sample_definition("other");
        let err = update_definition(&conn, "app", &other).unwrap_err();
        assert!(matches!(err, RegistryError::IdMismatch { .. }));
    }

    #[test]
    fn update_definition_bumps_revision() {
        let conn = conn();
        install(&conn, &sample_definition("app")).unwrap();
        let updated = update_definition(&conn, "app", &sample_definition("app")).unwrap();
        assert_eq!(updated.revision, 2);
    }

    #[test]
    fn remove_deletes_record() {
        let conn = conn();
        install(&conn, &sample_definition("app")).unwrap();
        remove(&conn, "app").unwrap();
        assert!(get(&conn, "app").unwrap().is_none());
        assert!(matches!(
            remove(&conn, "app").unwrap_err(),
            RegistryError::NotFound(_)
        ));
    }
}
