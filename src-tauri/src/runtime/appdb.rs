//! Per-app SQLite database: turns an App Schema `DataModel` into real tables,
//! and safely migrates them when a `DataModel` changes.
//!
//! Every entity gets a table with a runtime-owned `id`/`createdAt`/`updatedAt`
//! system columns plus one column per non-many-cardinality field. A
//! many-cardinality reference field becomes a link table instead of a column,
//! since SQLite has no array column type.

use std::fmt;
use std::path::Path;

use rusqlite::Connection;

use crate::app_schema::{DataModel, Entity, Field};

#[derive(Debug)]
pub enum AppDbError {
    UnsafeIdentifier(String),
    Db(rusqlite::Error),
    Io(std::io::Error),
    /// Migration would drop or reinterpret existing data; refused unless the
    /// caller explicitly opts in with `force`.
    UnsafeMigration(Vec<String>),
}

impl fmt::Display for AppDbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppDbError::UnsafeIdentifier(id) => write!(f, "unsafe identifier in schema: \"{id}\""),
            AppDbError::Db(e) => write!(f, "app database error: {e}"),
            AppDbError::Io(e) => write!(f, "app database io error: {e}"),
            AppDbError::UnsafeMigration(changes) => {
                write!(
                    f,
                    "migration would lose data unless forced: {}",
                    changes.join("; ")
                )
            }
        }
    }
}

impl std::error::Error for AppDbError {}
impl From<rusqlite::Error> for AppDbError {
    fn from(e: rusqlite::Error) -> Self {
        AppDbError::Db(e)
    }
}
impl From<std::io::Error> for AppDbError {
    fn from(e: std::io::Error) -> Self {
        AppDbError::Io(e)
    }
}

/// Defense-in-depth: entity/field ids are also enforced as camelCase by the
/// TS Zod schema, but since SQLite has no way to parameterize identifiers in
/// DDL, we re-check here before splicing them into SQL — a Tauri command is
/// invocable directly from the frontend, bypassing `validateAppDefinition`.
fn check_identifier(id: &str) -> Result<(), AppDbError> {
    let ok = !id.is_empty()
        && id.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && id.chars().all(|c| c.is_ascii_alphanumeric());
    if ok {
        Ok(())
    } else {
        Err(AppDbError::UnsafeIdentifier(id.to_string()))
    }
}

fn link_table_name(entity_id: &str, field_id: &str) -> String {
    format!("link__{entity_id}__{field_id}")
}

pub fn open(db_path: &Path) -> Result<Connection, AppDbError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Connection::open(db_path)?)
}

/// A required field with no schema-declared default still needs *some* seed
/// value so existing rows survive a table rebuild (SQLite refuses to add a
/// `NOT NULL` column with no default to a non-empty table, and a fresh
/// `CREATE TABLE` must stay consistent with that same rule).
fn column_default_clause(field: &Field) -> &'static str {
    if !field.base().required.unwrap_or(false) {
        return "";
    }
    match field {
        Field::Number(_) => " NOT NULL DEFAULT 0",
        Field::Boolean(_) => " NOT NULL DEFAULT 0",
        _ => " NOT NULL DEFAULT ''",
    }
}

fn create_entity_table(conn: &Connection, entity: &Entity) -> Result<(), AppDbError> {
    check_identifier(&entity.id)?;
    let mut columns = vec![
        "id TEXT PRIMARY KEY".to_string(),
        "createdAt TEXT NOT NULL".to_string(),
        "updatedAt TEXT NOT NULL".to_string(),
    ];
    for field in &entity.fields {
        check_identifier(field.id())?;
        if field.is_many_reference() {
            continue;
        }
        columns.push(format!(
            "\"{}\" {}{}",
            field.id(),
            field.sql_type(),
            column_default_clause(field)
        ));
    }
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS \"{}\" ({})",
        entity.id,
        columns.join(", ")
    );
    conn.execute(&sql, [])?;

    for field in &entity.fields {
        if field.is_many_reference() {
            let table = link_table_name(&entity.id, field.id());
            let sql = format!(
                "CREATE TABLE IF NOT EXISTS \"{table}\" (sourceId TEXT NOT NULL, targetId TEXT NOT NULL, PRIMARY KEY (sourceId, targetId))"
            );
            conn.execute(&sql, [])?;
        }
    }
    Ok(())
}

/// Creates all tables for a brand-new app install.
pub fn create_schema(conn: &Connection, data_model: &DataModel) -> Result<(), AppDbError> {
    for entity in &data_model.entities {
        create_entity_table(conn, entity)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct MigrationPlan {
    pub created_entities: Vec<String>,
    pub added_columns: Vec<(String, String)>,
    pub dropped_entities: Vec<String>,
    pub retyped_columns: Vec<(String, String)>,
}

fn find_entity<'a>(model: &'a DataModel, id: &str) -> Option<&'a Entity> {
    model.entities.iter().find(|e| e.id == id)
}

fn find_field<'a>(entity: &'a Entity, id: &str) -> Option<&'a Field> {
    entity.fields.iter().find(|f| f.id() == id)
}

/// Diffs `old_model` -> `new_model` and applies the safe subset (new entities,
/// new columns) in a transaction. Destructive changes (removed entities,
/// removed fields, changed field types) are reported but not applied unless
/// `force` is true, in which case the affected table is rebuilt via SQLite's
/// standard "create new table, copy what still fits, swap" technique.
pub fn migrate_schema(
    conn: &mut Connection,
    old_model: &DataModel,
    new_model: &DataModel,
    force: bool,
) -> Result<MigrationPlan, AppDbError> {
    let mut plan = MigrationPlan::default();
    let mut unsafe_changes = Vec::new();

    for entity in &old_model.entities {
        if find_entity(new_model, &entity.id).is_none() {
            unsafe_changes.push(format!("entity \"{}\" removed", entity.id));
            plan.dropped_entities.push(entity.id.clone());
        }
    }

    let mut retype_plan: Vec<(&Entity, &Entity)> = Vec::new();

    for new_entity in &new_model.entities {
        match find_entity(old_model, &new_entity.id) {
            None => {
                plan.created_entities.push(new_entity.id.clone());
            }
            Some(old_entity) => {
                let mut needs_retype = false;
                for old_field in &old_entity.fields {
                    if old_field.is_many_reference() {
                        continue;
                    }
                    match find_field(new_entity, old_field.id()) {
                        None => {
                            unsafe_changes.push(format!(
                                "field \"{}.{}\" removed",
                                new_entity.id,
                                old_field.id()
                            ));
                            needs_retype = true;
                        }
                        Some(new_field) => {
                            if new_field.type_tag() != old_field.type_tag() {
                                unsafe_changes.push(format!(
                                    "field \"{}.{}\" type changed ({} -> {})",
                                    new_entity.id,
                                    old_field.id(),
                                    old_field.type_tag(),
                                    new_field.type_tag()
                                ));
                                plan.retyped_columns.push((
                                    format!("{}.{}", new_entity.id, old_field.id()),
                                    new_field.type_tag().to_string(),
                                ));
                                needs_retype = true;
                            }
                        }
                    }
                }
                for new_field in &new_entity.fields {
                    if new_field.is_many_reference() {
                        continue;
                    }
                    if find_field(old_entity, new_field.id()).is_none() {
                        plan.added_columns
                            .push((new_entity.id.clone(), new_field.id().to_string()));
                    }
                }
                if needs_retype {
                    retype_plan.push((old_entity, new_entity));
                }
            }
        }
    }

    if !unsafe_changes.is_empty() && !force {
        return Err(AppDbError::UnsafeMigration(unsafe_changes));
    }

    let tx = conn.transaction()?;

    for new_entity in &new_model.entities {
        if find_entity(old_model, &new_entity.id).is_none() {
            create_entity_table(&tx, new_entity)?;
        }
    }

    for new_entity in &new_model.entities {
        let Some(old_entity) = find_entity(old_model, &new_entity.id) else {
            continue;
        };
        if retype_plan.iter().any(|(_, e)| e.id == new_entity.id) {
            continue;
        }
        for new_field in &new_entity.fields {
            if new_field.is_many_reference() {
                let table = link_table_name(&new_entity.id, new_field.id());
                tx.execute(
                    &format!("CREATE TABLE IF NOT EXISTS \"{table}\" (sourceId TEXT NOT NULL, targetId TEXT NOT NULL, PRIMARY KEY (sourceId, targetId))"),
                    [],
                )?;
                continue;
            }
            if find_field(old_entity, new_field.id()).is_none() {
                check_identifier(new_field.id())?;
                let sql = format!(
                    "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}{}",
                    new_entity.id,
                    new_field.id(),
                    new_field.sql_type(),
                    column_default_clause(new_field)
                );
                tx.execute(&sql, [])?;
            }
        }
    }

    // Rebuild tables with removed/retyped columns (only reached with force =
    // true): rename the old table aside, create a fresh one with the new
    // schema, copy across the columns both versions still share (casting
    // retyped ones), then drop the old data.
    for (old_entity, new_entity) in &retype_plan {
        check_identifier(&new_entity.id)?;
        let backup_name = format!("__migrate_backup_{}", new_entity.id);
        tx.execute(
            &format!(
                "ALTER TABLE \"{}\" RENAME TO \"{backup_name}\"",
                new_entity.id
            ),
            [],
        )?;
        create_entity_table(&tx, new_entity)?;

        let mut shared_columns = vec![
            "id".to_string(),
            "createdAt".to_string(),
            "updatedAt".to_string(),
        ];
        for new_field in &new_entity.fields {
            if !new_field.is_many_reference() && find_field(old_entity, new_field.id()).is_some() {
                shared_columns.push(new_field.id().to_string());
            }
        }
        let dest_list = shared_columns
            .iter()
            .map(|c| format!("\"{c}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let source_list = shared_columns
            .iter()
            .map(|c| {
                let target_type = new_entity
                    .fields
                    .iter()
                    .find(|f| f.id() == c)
                    .map(|f| f.sql_type())
                    .unwrap_or("TEXT");
                format!("CAST(\"{c}\" AS {target_type}) AS \"{c}\"")
            })
            .collect::<Vec<_>>()
            .join(", ");
        tx.execute(
            &format!(
                "INSERT INTO \"{}\" ({dest_list}) SELECT {source_list} FROM \"{backup_name}\"",
                new_entity.id
            ),
            [],
        )?;
        tx.execute(&format!("DROP TABLE \"{backup_name}\""), [])?;
    }

    tx.commit()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(json: serde_json::Value) -> DataModel {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn create_schema_makes_tables_and_link_table() {
        let conn = Connection::open_in_memory().unwrap();
        let data_model = model(serde_json::json!({
            "entities": [
                { "id": "item", "name": "Item", "fields": [
                    { "id": "title", "type": "string", "required": true },
                    { "id": "price", "type": "number" },
                    { "id": "tags", "type": "reference", "entityId": "tag", "cardinality": "many" }
                ] },
                { "id": "tag", "name": "Tag", "fields": [
                    { "id": "name", "type": "string", "required": true }
                ] }
            ]
        }));
        create_schema(&conn, &data_model).unwrap();

        conn.execute("INSERT INTO item (id, createdAt, updatedAt, title, price) VALUES ('1','now','now','Widget',9.5)", [])
            .unwrap();
        let title: String = conn
            .query_row("SELECT title FROM item WHERE id='1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Widget");

        conn.execute(
            "INSERT INTO \"link__item__tags\" (sourceId, targetId) VALUES ('1','t1')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn migrate_adds_new_entity_and_column() {
        let conn = Connection::open_in_memory().unwrap();
        let mut conn = conn;
        let old = model(serde_json::json!({
            "entities": [
                { "id": "item", "name": "Item", "fields": [
                    { "id": "title", "type": "string", "required": true }
                ] }
            ]
        }));
        create_schema(&conn, &old).unwrap();
        conn.execute(
            "INSERT INTO item (id, createdAt, updatedAt, title) VALUES ('1','now','now','Widget')",
            [],
        )
        .unwrap();

        let new = model(serde_json::json!({
            "entities": [
                { "id": "item", "name": "Item", "fields": [
                    { "id": "title", "type": "string", "required": true },
                    { "id": "note", "type": "string" }
                ] },
                { "id": "tag", "name": "Tag", "fields": [
                    { "id": "name", "type": "string", "required": true }
                ] }
            ]
        }));

        let plan = migrate_schema(&mut conn, &old, &new, false).unwrap();
        assert_eq!(plan.created_entities, vec!["tag".to_string()]);
        assert_eq!(
            plan.added_columns,
            vec![("item".to_string(), "note".to_string())]
        );

        let title: String = conn
            .query_row("SELECT title FROM item WHERE id='1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Widget");
        conn.execute(
            "INSERT INTO tag (id, createdAt, updatedAt, name) VALUES ('t1','now','now','x')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn migrate_refuses_destructive_change_without_force() {
        let mut conn = Connection::open_in_memory().unwrap();
        let old = model(serde_json::json!({
            "entities": [
                { "id": "item", "name": "Item", "fields": [
                    { "id": "title", "type": "string" },
                    { "id": "count", "type": "number" }
                ] }
            ]
        }));
        create_schema(&conn, &old).unwrap();

        let new = model(serde_json::json!({
            "entities": [
                { "id": "item", "name": "Item", "fields": [
                    { "id": "title", "type": "string" }
                ] }
            ]
        }));

        let err = migrate_schema(&mut conn, &old, &new, false).unwrap_err();
        assert!(matches!(err, AppDbError::UnsafeMigration(_)));

        // Refused migration must not have touched the table.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('item') WHERE name='count'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migrate_with_force_rebuilds_and_preserves_shared_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        let old = model(serde_json::json!({
            "entities": [
                { "id": "item", "name": "Item", "fields": [
                    { "id": "title", "type": "string" },
                    { "id": "quantity", "type": "string" }
                ] }
            ]
        }));
        create_schema(&conn, &old).unwrap();
        conn.execute("INSERT INTO item (id, createdAt, updatedAt, title, quantity) VALUES ('1','now','now','Widget','3')", [])
            .unwrap();

        let new = model(serde_json::json!({
            "entities": [
                { "id": "item", "name": "Item", "fields": [
                    { "id": "title", "type": "string" },
                    { "id": "quantity", "type": "number" }
                ] }
            ]
        }));

        migrate_schema(&mut conn, &old, &new, true).unwrap();

        let (title, quantity): (String, f64) = conn
            .query_row("SELECT title, quantity FROM item WHERE id='1'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(title, "Widget");
        assert_eq!(quantity, 3.0);
    }

    #[test]
    fn unsafe_identifier_is_rejected() {
        let conn = Connection::open_in_memory().unwrap();
        // A hostile Tauri command call could bypass TS validation entirely,
        // so appdb must not trust ids blindly when splicing them into DDL.
        let data_model = DataModel {
            entities: vec![Entity {
                id: "item; DROP TABLE apps;--".to_string(),
                name: "Item".to_string(),
                fields: vec![],
            }],
        };
        let err = create_schema(&conn, &data_model).unwrap_err();
        assert!(matches!(err, AppDbError::UnsafeIdentifier(_)));
    }
}
