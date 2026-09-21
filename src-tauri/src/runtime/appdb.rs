//! Per-app SQLite database: turns an App Schema `DataModel` into real tables,
//! and safely migrates them when a `DataModel` changes.
//!
//! Every entity gets a table with a runtime-owned `id`/`createdAt`/`updatedAt`
//! system columns plus one column per non-many-cardinality field. A
//! many-cardinality reference field becomes a link table instead of a column,
//! since SQLite has no array column type.

use std::fmt;
use std::path::Path;

use chrono::Utc;
use rusqlite::types::{Value as SqlValue, ValueRef};
use rusqlite::Connection;
use uuid::Uuid;

use crate::app_schema::{DataModel, Entity, Field};

#[derive(Debug)]
pub enum AppDbError {
    UnsafeIdentifier(String),
    Db(rusqlite::Error),
    Io(std::io::Error),
    /// Migration would drop or reinterpret existing data; refused unless the
    /// caller explicitly opts in with `force`.
    UnsafeMigration(Vec<String>),
    /// A record CRUD call referenced a field id that isn't part of the
    /// entity's schema — rejected instead of silently dropped so a caller
    /// that bypassed TS validation gets a clear error, not partial data.
    UnknownField(String),
    RecordNotFound(String),
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
            AppDbError::UnknownField(id) => write!(f, "unknown field \"{id}\" for entity"),
            AppDbError::RecordNotFound(id) => write!(f, "record \"{id}\" not found"),
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

/// Non-many-cardinality fields, in the order they were declared — the same
/// set that `create_entity_table` gives real columns to.
fn column_fields(entity: &Entity) -> impl Iterator<Item = &Field> {
    entity.fields.iter().filter(|f| !f.is_many_reference())
}

fn find_column_field<'a>(entity: &'a Entity, id: &str) -> Result<&'a Field, AppDbError> {
    column_fields(entity)
        .find(|f| f.id() == id)
        .ok_or_else(|| AppDbError::UnknownField(id.to_string()))
}

/// Converts a JSON value supplied by a CRUD caller into the SQLite value for
/// `field`'s column, per the same type mapping `sql_type` uses for DDL.
fn field_to_sql(field: &Field, value: &serde_json::Value) -> Result<SqlValue, AppDbError> {
    if value.is_null() {
        return Ok(SqlValue::Null);
    }
    Ok(match field {
        Field::Boolean(_) => SqlValue::Integer(if value.as_bool().unwrap_or(false) {
            1
        } else {
            0
        }),
        Field::Number(_) => SqlValue::Real(value.as_f64().unwrap_or(0.0)),
        _ => SqlValue::Text(
            value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string()),
        ),
    })
}

/// Converts one column's SQLite value back to JSON, per `field`'s type.
fn field_from_sql(field: &Field, raw: ValueRef<'_>) -> rusqlite::Result<serde_json::Value> {
    if matches!(raw, ValueRef::Null) {
        return Ok(serde_json::Value::Null);
    }
    Ok(match field {
        Field::Boolean(_) => serde_json::Value::Bool(raw.as_i64()? != 0),
        Field::Number(_) => serde_json::json!(raw.as_f64()?),
        _ => serde_json::Value::String(raw.as_str()?.to_string()),
    })
}

fn row_to_json(row: &rusqlite::Row, entity: &Entity) -> rusqlite::Result<serde_json::Value> {
    let mut map = serde_json::Map::new();
    map.insert(
        "id".to_string(),
        serde_json::json!(row.get::<_, String>("id")?),
    );
    map.insert(
        "createdAt".to_string(),
        serde_json::json!(row.get::<_, String>("createdAt")?),
    );
    map.insert(
        "updatedAt".to_string(),
        serde_json::json!(row.get::<_, String>("updatedAt")?),
    );
    for field in column_fields(entity) {
        let raw = row.get_ref(field.id())?;
        map.insert(field.id().to_string(), field_from_sql(field, raw)?);
    }
    Ok(serde_json::Value::Object(map))
}

fn select_columns(entity: &Entity) -> String {
    let mut columns = vec![
        "id".to_string(),
        "createdAt".to_string(),
        "updatedAt".to_string(),
    ];
    columns.extend(column_fields(entity).map(|f| format!("\"{}\"", f.id())));
    columns.join(", ")
}

pub fn list_records(
    conn: &Connection,
    entity: &Entity,
) -> Result<Vec<serde_json::Value>, AppDbError> {
    check_identifier(&entity.id)?;
    let sql = format!(
        "SELECT {} FROM \"{}\" ORDER BY createdAt",
        select_columns(entity),
        entity.id
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map([], |row| row_to_json(row, entity))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_record(
    conn: &Connection,
    entity: &Entity,
    id: &str,
) -> Result<Option<serde_json::Value>, AppDbError> {
    check_identifier(&entity.id)?;
    let sql = format!(
        "SELECT {} FROM \"{}\" WHERE id = ?1",
        select_columns(entity),
        entity.id
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], |row| row_to_json(row, entity))?;
    rows.next().transpose().map_err(AppDbError::from)
}

/// Inserts a new record. Unknown keys in `values` are rejected; missing keys
/// fall back to the column's schema-declared default (or SQL `NULL`).
pub fn insert_record(
    conn: &Connection,
    entity: &Entity,
    values: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, AppDbError> {
    check_identifier(&entity.id)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let mut columns = vec![
        "id".to_string(),
        "createdAt".to_string(),
        "updatedAt".to_string(),
    ];
    let mut params: Vec<SqlValue> = vec![
        SqlValue::Text(id.clone()),
        SqlValue::Text(now.clone()),
        SqlValue::Text(now),
    ];

    for (key, value) in values {
        let field = find_column_field(entity, key)?;
        columns.push(format!("\"{key}\""));
        params.push(field_to_sql(field, value)?);
    }

    let placeholders = (1..=params.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO \"{}\" ({}) VALUES ({placeholders})",
        entity.id,
        columns.join(", ")
    );
    conn.execute(&sql, rusqlite::params_from_iter(params))?;

    get_record(conn, entity, &id)?.ok_or(AppDbError::RecordNotFound(id))
}

/// Updates an existing record's provided fields and refreshes `updatedAt`.
pub fn update_record(
    conn: &Connection,
    entity: &Entity,
    id: &str,
    values: &serde_json::Map<String, serde_json::Value>,
) -> Result<serde_json::Value, AppDbError> {
    check_identifier(&entity.id)?;
    if get_record(conn, entity, id)?.is_none() {
        return Err(AppDbError::RecordNotFound(id.to_string()));
    }

    let mut assignments = vec!["updatedAt = ?1".to_string()];
    let mut params: Vec<SqlValue> = vec![SqlValue::Text(Utc::now().to_rfc3339())];

    for (key, value) in values {
        let field = find_column_field(entity, key)?;
        params.push(field_to_sql(field, value)?);
        assignments.push(format!("\"{key}\" = ?{}", params.len()));
    }
    params.push(SqlValue::Text(id.to_string()));

    let sql = format!(
        "UPDATE \"{}\" SET {} WHERE id = ?{}",
        entity.id,
        assignments.join(", "),
        params.len()
    );
    conn.execute(&sql, rusqlite::params_from_iter(params))?;

    get_record(conn, entity, id)?.ok_or_else(|| AppDbError::RecordNotFound(id.to_string()))
}

pub fn delete_record(conn: &Connection, entity: &Entity, id: &str) -> Result<(), AppDbError> {
    check_identifier(&entity.id)?;
    let changed = conn.execute(
        &format!("DELETE FROM \"{}\" WHERE id = ?1", entity.id),
        [id],
    )?;
    if changed == 0 {
        return Err(AppDbError::RecordNotFound(id.to_string()));
    }
    Ok(())
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

    fn item_entity() -> Entity {
        serde_json::from_value(serde_json::json!({
            "id": "item", "name": "Item", "fields": [
                { "id": "title", "type": "string", "required": true },
                { "id": "price", "type": "number" },
                { "id": "archived", "type": "boolean" }
            ]
        }))
        .unwrap()
    }

    #[test]
    fn insert_list_update_delete_record_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        let entity = item_entity();
        create_entity_table(&conn, &entity).unwrap();

        let mut values = serde_json::Map::new();
        values.insert("title".to_string(), serde_json::json!("Widget"));
        values.insert("price".to_string(), serde_json::json!(9.5));
        values.insert("archived".to_string(), serde_json::json!(false));
        let created = insert_record(&conn, &entity, &values).unwrap();
        let id = created["id"].as_str().unwrap().to_string();
        assert_eq!(created["title"], serde_json::json!("Widget"));
        assert_eq!(created["price"], serde_json::json!(9.5));
        assert_eq!(created["archived"], serde_json::json!(false));

        let listed = list_records(&conn, &entity).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0]["id"], serde_json::json!(id));

        let mut update = serde_json::Map::new();
        update.insert("price".to_string(), serde_json::json!(12.0));
        update.insert("archived".to_string(), serde_json::json!(true));
        let updated = update_record(&conn, &entity, &id, &update).unwrap();
        assert_eq!(updated["price"], serde_json::json!(12.0));
        assert_eq!(updated["archived"], serde_json::json!(true));
        assert_eq!(updated["title"], serde_json::json!("Widget"));
        assert_ne!(updated["updatedAt"], updated["createdAt"]);

        delete_record(&conn, &entity, &id).unwrap();
        assert!(list_records(&conn, &entity).unwrap().is_empty());
        assert!(matches!(
            delete_record(&conn, &entity, &id).unwrap_err(),
            AppDbError::RecordNotFound(_)
        ));
    }

    #[test]
    fn crud_rejects_unknown_field() {
        let conn = Connection::open_in_memory().unwrap();
        let entity = item_entity();
        create_entity_table(&conn, &entity).unwrap();

        let mut values = serde_json::Map::new();
        values.insert("nope".to_string(), serde_json::json!("x"));
        let err = insert_record(&conn, &entity, &values).unwrap_err();
        assert!(matches!(err, AppDbError::UnknownField(_)));
    }

    #[test]
    fn get_record_returns_none_for_missing_id() {
        let conn = Connection::open_in_memory().unwrap();
        let entity = item_entity();
        create_entity_table(&conn, &entity).unwrap();
        assert!(get_record(&conn, &entity, "missing").unwrap().is_none());
    }
}
