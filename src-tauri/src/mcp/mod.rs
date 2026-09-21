//! MCP interface layer: exposes the Runtime Core (`crate::runtime`) as MCP
//! tools for an AI agent, over stdio (see `src/bin/mcp_server.rs`). This is
//! TODO step 4 ("MCP 接口层"). See `docs/mcp-server.md` for the tool
//! reference and AGENTS.md's "MCP Interface Layer" section for the
//! conventions new tools must follow.
//!
//! Deliberately **not** a Tauri command layer: every function here calls
//! straight into `runtime::{registry, appdb, storage, backup}`, the same
//! Tauri-independent functions `commands.rs` wraps for the GUI. It does not
//! run its own `runtime::scheduler::Scheduler` — that's a polling thread tied
//! to a `tauri::AppHandle` for emitting GUI events, and running a second copy
//! here would double-fire automations if the desktop app is open at the same
//! time. `list_automations` only reads the automations already stored on an
//! app's definition.

pub mod error;
mod params;
pub mod validator;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::{Implementation, InitializeResult, ServerCapabilities};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler};
use rusqlite::Connection;

use crate::app_schema::AppDefinition;
use crate::runtime::backup::{self, BackupInfo};
use crate::runtime::registry::{self, AppStatus};
use crate::runtime::{appdb, paths, storage};

use error::McpToolError;
use params::*;
use validator::ValidationOutcome;

#[derive(Clone)]
pub struct AppletMcpServer {
    base_dir: PathBuf,
    registry_conn: Arc<Mutex<Connection>>,
}

impl AppletMcpServer {
    pub fn new(base_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&base_dir).map_err(|e| e.to_string())?;
        let conn =
            Connection::open(paths::registry_db_path(&base_dir)).map_err(|e| e.to_string())?;
        registry::init_schema(&conn).map_err(|e| e.to_string())?;
        Ok(Self {
            base_dir,
            registry_conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn get_installed(&self, app_id: &str) -> Result<registry::AppRecord, McpToolError> {
        let conn = self.registry_conn.lock().unwrap();
        registry::get(&conn, app_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("app \"{app_id}\" is not installed"))
            .map_err(McpToolError)
    }

    fn find_entity<'a>(
        definition: &'a AppDefinition,
        entity_id: &str,
    ) -> Result<&'a crate::app_schema::Entity, McpToolError> {
        definition
            .data_model
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .ok_or_else(|| {
                McpToolError(format!("entity \"{entity_id}\" is not defined on this app"))
            })
    }
}

/// Rejects a destructive call unless `confirm` was explicitly passed as
/// `true`, describing exactly what would be lost. This is a structural
/// double-check (a second required argument), not just a warning in prose —
/// see AGENTS.md's MCP section and TODO step 4's "危险操作确认机制" item.
fn require_confirmation(
    confirm: bool,
    what_is_lost: impl FnOnce() -> String,
) -> Result<(), McpToolError> {
    if confirm {
        Ok(())
    } else {
        Err(McpToolError(format!(
            "{} Retry with confirm: true to proceed.",
            what_is_lost()
        )))
    }
}

fn to_json<T: serde::Serialize>(value: T) -> Result<Json<serde_json::Value>, McpToolError> {
    serde_json::to_value(value)
        .map(Json)
        .map_err(|e| McpToolError(e.to_string()))
}

fn as_object(
    value: serde_json::Value,
) -> Result<serde_json::Map<String, serde_json::Value>, McpToolError> {
    match value {
        serde_json::Value::Object(map) => Ok(map),
        _ => Err(McpToolError("values must be a JSON object".to_string())),
    }
}

#[tool_router]
impl AppletMcpServer {
    // --- App lifecycle -----------------------------------------------

    #[tool(
        description = "Create/install a new App from a declarative App definition. The definition is \
        validated against the App Schema first; on failure no state changes and the validation errors \
        are returned so the caller can self-correct."
    )]
    async fn install_app(
        &self,
        Parameters(InstallAppParams { definition }): Parameters<InstallAppParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let validated =
            match validator::validate_app_definition(&definition).map_err(McpToolError)? {
                ValidationOutcome::Valid(data) => data,
                ValidationOutcome::Invalid(errors) => {
                    return Ok(Json(
                        serde_json::json!({ "success": false, "errors": errors }),
                    ));
                }
            };
        let definition: AppDefinition =
            serde_json::from_value(validated).map_err(|e| McpToolError(e.to_string()))?;

        let record = {
            let conn = self.registry_conn.lock().unwrap();
            registry::install(&conn, &definition).map_err(|e| McpToolError(e.to_string()))?
        };
        let dir = paths::app_dir(&self.base_dir, &definition.id);
        std::fs::create_dir_all(&dir).map_err(|e| McpToolError(e.to_string()))?;
        let db_path = paths::app_db_path(&self.base_dir, &definition.id);
        let db_conn = appdb::open(&db_path).map_err(|e| McpToolError(e.to_string()))?;
        appdb::create_schema(&db_conn, &definition.data_model)
            .map_err(|e| McpToolError(e.to_string()))?;

        Ok(to_json(record)?)
    }

    #[tool(description = "List every installed App with its metadata and status.")]
    async fn list_apps(
        &self,
        Parameters(EmptyParams {}): Parameters<EmptyParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let conn = self.registry_conn.lock().unwrap();
        let apps = registry::list(&conn).map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(apps)?)
    }

    #[tool(description = "Get one installed App's full record (metadata, status, definition).")]
    async fn get_app(
        &self,
        Parameters(AppIdParams { app_id }): Parameters<AppIdParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        Ok(to_json(self.get_installed(&app_id)?)?)
    }

    #[tool(
        description = "Apply a new definition to an installed App, migrating its database. Additive \
        changes (new entity/field) apply automatically. Anything that would drop or reinterpret data \
        (removed entity/field, changed field type) is refused unless force+confirm are both true."
    )]
    async fn update_app(
        &self,
        Parameters(UpdateAppParams {
            app_id,
            definition,
            force,
            confirm,
        }): Parameters<UpdateAppParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        if force {
            require_confirmation(confirm, || {
                format!(
                    "force=true on app \"{app_id}\" may drop or reinterpret existing data for removed/retyped fields, permanently."
                )
            })?;
        }
        let validated =
            match validator::validate_app_definition(&definition).map_err(McpToolError)? {
                ValidationOutcome::Valid(data) => data,
                ValidationOutcome::Invalid(errors) => {
                    return Ok(Json(
                        serde_json::json!({ "success": false, "errors": errors }),
                    ));
                }
            };
        let definition: AppDefinition =
            serde_json::from_value(validated).map_err(|e| McpToolError(e.to_string()))?;

        let existing = self.get_installed(&app_id)?;
        let db_path = paths::app_db_path(&self.base_dir, &app_id);
        let mut db_conn = appdb::open(&db_path).map_err(|e| McpToolError(e.to_string()))?;
        appdb::migrate_schema(
            &mut db_conn,
            &existing.definition.data_model,
            &definition.data_model,
            force,
        )
        .map_err(|e| McpToolError(e.to_string()))?;

        let conn = self.registry_conn.lock().unwrap();
        let record = registry::update_definition(&conn, &app_id, &definition)
            .map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(record)?)
    }

    #[tool(description = "Start an installed App (enables its automations to be scheduled).")]
    async fn start_app(
        &self,
        Parameters(AppIdParams { app_id }): Parameters<AppIdParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let conn = self.registry_conn.lock().unwrap();
        let record = registry::set_status(&conn, &app_id, AppStatus::Running)
            .map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(record)?)
    }

    #[tool(description = "Stop an installed App.")]
    async fn stop_app(
        &self,
        Parameters(AppIdParams { app_id }): Parameters<AppIdParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let conn = self.registry_conn.lock().unwrap();
        let record = registry::set_status(&conn, &app_id, AppStatus::Stopped)
            .map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(record)?)
    }

    #[tool(
        description = "Uninstall an App. By default its database and files are left on disk (only the \
        registry entry is removed). purge_data=true also deletes them, permanently, and requires \
        confirm=true."
    )]
    async fn uninstall_app(
        &self,
        Parameters(UninstallAppParams {
            app_id,
            purge_data,
            confirm,
        }): Parameters<UninstallAppParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        if purge_data {
            require_confirmation(confirm, || {
                format!("purge_data=true will permanently delete app \"{app_id}\"'s database and files.")
            })?;
        }
        {
            let conn = self.registry_conn.lock().unwrap();
            registry::remove(&conn, &app_id).map_err(|e| McpToolError(e.to_string()))?;
        }
        if purge_data {
            let dir = paths::app_dir(&self.base_dir, &app_id);
            if dir.exists() {
                std::fs::remove_dir_all(dir).map_err(|e| McpToolError(e.to_string()))?;
            }
        }
        Ok(to_json(serde_json::json!({ "uninstalled": app_id }))?)
    }

    // --- Data CRUD (entity-scoped, not arbitrary SQL) -----------------

    #[tool(description = "List every record of one entity in an installed App.")]
    async fn list_records(
        &self,
        Parameters(ListRecordsParams { app_id, entity_id }): Parameters<ListRecordsParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let record = self.get_installed(&app_id)?;
        let entity = Self::find_entity(&record.definition, &entity_id)?;
        let db_conn = appdb::open(&paths::app_db_path(&self.base_dir, &app_id))
            .map_err(|e| McpToolError(e.to_string()))?;
        let rows =
            appdb::list_records(&db_conn, entity).map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(rows)?)
    }

    #[tool(description = "Create a new record of one entity in an installed App.")]
    async fn create_record(
        &self,
        Parameters(CreateRecordParams {
            app_id,
            entity_id,
            values,
        }): Parameters<CreateRecordParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let record = self.get_installed(&app_id)?;
        let entity = Self::find_entity(&record.definition, &entity_id)?;
        let values = as_object(values)?;
        let db_conn = appdb::open(&paths::app_db_path(&self.base_dir, &app_id))
            .map_err(|e| McpToolError(e.to_string()))?;
        let created = appdb::insert_record(&db_conn, entity, &values)
            .map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(created)?)
    }

    #[tool(
        description = "Update an existing record's fields; unspecified fields are left unchanged."
    )]
    async fn update_record(
        &self,
        Parameters(UpdateRecordParams {
            app_id,
            entity_id,
            record_id,
            values,
        }): Parameters<UpdateRecordParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let record = self.get_installed(&app_id)?;
        let entity = Self::find_entity(&record.definition, &entity_id)?;
        let values = as_object(values)?;
        let db_conn = appdb::open(&paths::app_db_path(&self.base_dir, &app_id))
            .map_err(|e| McpToolError(e.to_string()))?;
        let updated = appdb::update_record(&db_conn, entity, &record_id, &values)
            .map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(updated)?)
    }

    #[tool(description = "Delete a record by id.")]
    async fn delete_record(
        &self,
        Parameters(DeleteRecordParams {
            app_id,
            entity_id,
            record_id,
        }): Parameters<DeleteRecordParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let record = self.get_installed(&app_id)?;
        let entity = Self::find_entity(&record.definition, &entity_id)?;
        let db_conn = appdb::open(&paths::app_db_path(&self.base_dir, &app_id))
            .map_err(|e| McpToolError(e.to_string()))?;
        appdb::delete_record(&db_conn, entity, &record_id)
            .map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(serde_json::json!({ "deleted": record_id }))?)
    }

    // --- File storage ---------------------------------------------------

    #[tool(description = "Write a file into an App's private storage (base64-encoded contents).")]
    async fn write_app_file(
        &self,
        Parameters(WriteAppFileParams {
            app_id,
            path,
            contents_base64,
        }): Parameters<WriteAppFileParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let dir = paths::app_dir(&self.base_dir, &app_id);
        std::fs::create_dir_all(&dir).map_err(|e| McpToolError(e.to_string()))?;
        let contents = base64::engine::general_purpose::STANDARD
            .decode(contents_base64)
            .map_err(|e| McpToolError(format!("invalid base64: {e}")))?;
        let root = paths::app_files_dir(&self.base_dir, &app_id);
        storage::write_file(&root, &path, &contents).map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(serde_json::json!({ "written": path }))?)
    }

    #[tool(description = "Read a file from an App's private storage; contents are base64-encoded.")]
    async fn read_app_file(
        &self,
        Parameters(AppFilePathParams { app_id, path }): Parameters<AppFilePathParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let root = paths::app_files_dir(&self.base_dir, &app_id);
        let contents = storage::read_file(&root, &path).map_err(|e| McpToolError(e.to_string()))?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(contents);
        Ok(to_json(
            serde_json::json!({ "path": path, "contentsBase64": encoded }),
        )?)
    }

    #[tool(description = "Delete a file (or directory) from an App's private storage.")]
    async fn delete_app_file(
        &self,
        Parameters(AppFilePathParams { app_id, path }): Parameters<AppFilePathParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let root = paths::app_files_dir(&self.base_dir, &app_id);
        storage::delete_file(&root, &path).map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(serde_json::json!({ "deleted": path }))?)
    }

    #[tool(description = "List every file path in an App's private storage.")]
    async fn list_app_files(
        &self,
        Parameters(AppIdParams { app_id }): Parameters<AppIdParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let root = paths::app_files_dir(&self.base_dir, &app_id);
        let files = storage::list_files(&root).map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(files)?)
    }

    #[tool(
        description = "Write a file into the shared storage directory (readable by every installed \
        App — there is no per-App grant yet, see AGENTS.md)."
    )]
    async fn write_shared_file(
        &self,
        Parameters(WriteSharedFileParams {
            path,
            contents_base64,
        }): Parameters<WriteSharedFileParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let root = paths::shared_dir(&self.base_dir);
        std::fs::create_dir_all(&root).map_err(|e| McpToolError(e.to_string()))?;
        let contents = base64::engine::general_purpose::STANDARD
            .decode(contents_base64)
            .map_err(|e| McpToolError(format!("invalid base64: {e}")))?;
        storage::write_file(&root, &path, &contents).map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(serde_json::json!({ "written": path }))?)
    }

    #[tool(
        description = "Read a file from the shared storage directory; contents are base64-encoded."
    )]
    async fn read_shared_file(
        &self,
        Parameters(SharedFilePathParams { path }): Parameters<SharedFilePathParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let root = paths::shared_dir(&self.base_dir);
        let contents = storage::read_file(&root, &path).map_err(|e| McpToolError(e.to_string()))?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(contents);
        Ok(to_json(
            serde_json::json!({ "path": path, "contentsBase64": encoded }),
        )?)
    }

    #[tool(description = "List every file path in the shared storage directory.")]
    async fn list_shared_files(
        &self,
        Parameters(EmptyParams {}): Parameters<EmptyParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let root = paths::shared_dir(&self.base_dir);
        let files = storage::list_files(&root).map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(files)?)
    }

    // --- Backup / restore -------------------------------------------------

    #[tool(
        description = "Back up one App (registry record + database + files) into a portable .zip."
    )]
    async fn backup_app(
        &self,
        Parameters(AppIdParams { app_id }): Parameters<AppIdParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let conn = self.registry_conn.lock().unwrap();
        let path = backup::backup_app(&self.base_dir, &conn, &app_id)
            .map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(
            serde_json::json!({ "path": path.to_string_lossy() }),
        )?)
    }

    #[tool(description = "Back up every installed App in one pass.")]
    async fn backup_all_apps(
        &self,
        Parameters(EmptyParams {}): Parameters<EmptyParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let conn = self.registry_conn.lock().unwrap();
        let paths =
            backup::backup_all(&self.base_dir, &conn).map_err(|e| McpToolError(e.to_string()))?;
        let paths: Vec<String> = paths
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        Ok(to_json(paths)?)
    }

    #[tool(description = "List available backup archives for one App, oldest first.")]
    async fn list_backups(
        &self,
        Parameters(AppIdParams { app_id }): Parameters<AppIdParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let infos = backup::list_backups(&self.base_dir, &app_id)
            .map_err(|e| McpToolError(e.to_string()))?;
        let infos: Vec<serde_json::Value> = infos
            .into_iter()
            .map(|BackupInfo { path, created_at }| {
                serde_json::json!({ "path": path.to_string_lossy(), "createdAt": created_at })
            })
            .collect();
        Ok(to_json(infos)?)
    }

    #[tool(
        description = "Restore an App from a backup archive, re-registering it with its database and \
        files. Refuses to clobber a currently-installed App with the same id unless overwrite+confirm \
        are both true."
    )]
    async fn restore_app(
        &self,
        Parameters(RestoreAppParams {
            archive_path,
            overwrite,
            confirm,
        }): Parameters<RestoreAppParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        if overwrite {
            require_confirmation(confirm, || {
                "overwrite=true will replace an already-installed App's current data with the backup's."
                    .to_string()
            })?;
        }
        let conn = self.registry_conn.lock().unwrap();
        let record = backup::restore_app(
            &self.base_dir,
            &conn,
            std::path::Path::new(&archive_path),
            overwrite,
        )
        .map_err(|e| McpToolError(e.to_string()))?;
        Ok(to_json(record)?)
    }

    // --- Scheduler (query only — see module doc) ---------------------------

    #[tool(
        description = "List the automations (cron triggers) defined on an installed App. Automations \
        are edited via update_app (they live on the App definition); there is no separate CRUD for \
        them. This only reports what's defined, not whether/when a running desktop app's scheduler will \
        fire it."
    )]
    async fn list_automations(
        &self,
        Parameters(AppIdParams { app_id }): Parameters<AppIdParams>,
    ) -> Result<Json<serde_json::Value>, ErrorData> {
        let record = self.get_installed(&app_id)?;
        Ok(to_json(record.definition.automations)?)
    }
}

#[tool_handler]
impl ServerHandler for AppletMcpServer {
    fn get_info(&self) -> InitializeResult {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::from_build_env())
            .with_instructions(
                "Applet Runtime MCP server: create/manage declarative Apps (data model, records, \
                 files, backups) for the Applet desktop runtime. See docs/mcp-server.md for the full \
                 tool reference. Destructive operations (uninstall with purge_data, update with force, \
                 restore with overwrite) require an explicit confirm: true.",
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle_available() -> bool {
        validator::validate_app_definition(&serde_json::json!({})).is_ok()
    }

    fn server() -> (AppletMcpServer, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let server = AppletMcpServer::new(dir.path().to_path_buf()).unwrap();
        (server, dir)
    }

    fn bookkeeping_definition(id: &str) -> serde_json::Value {
        let json = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("src/app-schema/examples/bookkeeping.json"),
        )
        .unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value["id"] = serde_json::json!(id);
        value
    }

    fn unwrap_ok(result: Result<Json<serde_json::Value>, ErrorData>) -> serde_json::Value {
        result.unwrap_or_else(|e| panic!("tool call failed: {e}")).0
    }

    fn unwrap_err(result: Result<Json<serde_json::Value>, ErrorData>) -> ErrorData {
        match result {
            Ok(_) => panic!("expected the tool call to fail"),
            Err(e) => e,
        }
    }

    #[tokio::test]
    async fn install_list_get_app_roundtrip() {
        if !bundle_available() {
            eprintln!(
                "skipping: dist-cli/validate-app.mjs not built, run `pnpm build:mcp-validator`"
            );
            return;
        }
        let (server, _dir) = server();
        let definition = bookkeeping_definition("mcp-test-app");

        let installed = unwrap_ok(
            server
                .install_app(Parameters(InstallAppParams { definition }))
                .await,
        );
        assert_eq!(installed["id"], "mcp-test-app");
        assert_eq!(installed["status"], "stopped");

        let listed = unwrap_ok(server.list_apps(Parameters(EmptyParams {})).await);
        assert_eq!(listed.as_array().unwrap().len(), 1);

        let fetched = unwrap_ok(
            server
                .get_app(Parameters(AppIdParams {
                    app_id: "mcp-test-app".to_string(),
                }))
                .await,
        );
        assert_eq!(fetched["id"], "mcp-test-app");
    }

    #[tokio::test]
    async fn install_rejects_an_invalid_definition_without_side_effects() {
        if !bundle_available() {
            eprintln!(
                "skipping: dist-cli/validate-app.mjs not built, run `pnpm build:mcp-validator`"
            );
            return;
        }
        let (server, _dir) = server();
        let result = unwrap_ok(
            server
                .install_app(Parameters(InstallAppParams {
                    definition: serde_json::json!({ "id": "not-enough" }),
                }))
                .await,
        );
        assert_eq!(result["success"], false);
        assert!(!result["errors"].as_array().unwrap().is_empty());

        let listed = unwrap_ok(server.list_apps(Parameters(EmptyParams {})).await);
        assert!(listed.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn record_crud_roundtrip() {
        if !bundle_available() {
            eprintln!(
                "skipping: dist-cli/validate-app.mjs not built, run `pnpm build:mcp-validator`"
            );
            return;
        }
        let (server, _dir) = server();
        let definition = bookkeeping_definition("mcp-record-app");
        unwrap_ok(
            server
                .install_app(Parameters(InstallAppParams { definition }))
                .await,
        );

        let created = unwrap_ok(
            server
                .create_record(Parameters(CreateRecordParams {
                    app_id: "mcp-record-app".to_string(),
                    entity_id: "transaction".to_string(),
                    values: serde_json::json!({
                        "date": "2026-01-01",
                        "category": "food",
                        "amount": 42.0,
                    }),
                }))
                .await,
        );
        let record_id = created["id"].as_str().unwrap().to_string();

        let listed = unwrap_ok(
            server
                .list_records(Parameters(ListRecordsParams {
                    app_id: "mcp-record-app".to_string(),
                    entity_id: "transaction".to_string(),
                }))
                .await,
        );
        assert_eq!(listed.as_array().unwrap().len(), 1);

        let updated = unwrap_ok(
            server
                .update_record(Parameters(UpdateRecordParams {
                    app_id: "mcp-record-app".to_string(),
                    entity_id: "transaction".to_string(),
                    record_id: record_id.clone(),
                    values: serde_json::json!({ "amount": 99.0 }),
                }))
                .await,
        );
        assert_eq!(updated["amount"], 99.0);

        unwrap_ok(
            server
                .delete_record(Parameters(DeleteRecordParams {
                    app_id: "mcp-record-app".to_string(),
                    entity_id: "transaction".to_string(),
                    record_id,
                }))
                .await,
        );
        let listed = unwrap_ok(
            server
                .list_records(Parameters(ListRecordsParams {
                    app_id: "mcp-record-app".to_string(),
                    entity_id: "transaction".to_string(),
                }))
                .await,
        );
        assert!(listed.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn app_file_write_read_list_delete_roundtrip() {
        let (server, _dir) = server();
        let contents = base64::engine::general_purpose::STANDARD.encode(b"hello mcp");

        unwrap_ok(
            server
                .write_app_file(Parameters(WriteAppFileParams {
                    app_id: "files-app".to_string(),
                    path: "notes/a.txt".to_string(),
                    contents_base64: contents,
                }))
                .await,
        );

        let listed = unwrap_ok(
            server
                .list_app_files(Parameters(AppIdParams {
                    app_id: "files-app".to_string(),
                }))
                .await,
        );
        assert_eq!(listed, serde_json::json!(["notes/a.txt"]));

        let read = unwrap_ok(
            server
                .read_app_file(Parameters(AppFilePathParams {
                    app_id: "files-app".to_string(),
                    path: "notes/a.txt".to_string(),
                }))
                .await,
        );
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(read["contentsBase64"].as_str().unwrap())
            .unwrap();
        assert_eq!(decoded, b"hello mcp");

        unwrap_ok(
            server
                .delete_app_file(Parameters(AppFilePathParams {
                    app_id: "files-app".to_string(),
                    path: "notes/a.txt".to_string(),
                }))
                .await,
        );
        let listed = unwrap_ok(
            server
                .list_app_files(Parameters(AppIdParams {
                    app_id: "files-app".to_string(),
                }))
                .await,
        );
        assert!(listed.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn backup_and_restore_roundtrip() {
        if !bundle_available() {
            eprintln!(
                "skipping: dist-cli/validate-app.mjs not built, run `pnpm build:mcp-validator`"
            );
            return;
        }
        let (server, _dir) = server();
        let definition = bookkeeping_definition("mcp-backup-app");
        unwrap_ok(
            server
                .install_app(Parameters(InstallAppParams { definition }))
                .await,
        );

        let backup_result = unwrap_ok(
            server
                .backup_app(Parameters(AppIdParams {
                    app_id: "mcp-backup-app".to_string(),
                }))
                .await,
        );
        let archive_path = backup_result["path"].as_str().unwrap().to_string();

        let backups = unwrap_ok(
            server
                .list_backups(Parameters(AppIdParams {
                    app_id: "mcp-backup-app".to_string(),
                }))
                .await,
        );
        assert_eq!(backups.as_array().unwrap().len(), 1);

        // Restoring without overwrite over an already-installed app must fail.
        let err = unwrap_err(
            server
                .restore_app(Parameters(RestoreAppParams {
                    archive_path: archive_path.clone(),
                    overwrite: false,
                    confirm: false,
                }))
                .await,
        );
        assert!(err.message.contains("already installed"));

        // overwrite=true without confirm=true is rejected before touching anything.
        let err = unwrap_err(
            server
                .restore_app(Parameters(RestoreAppParams {
                    archive_path: archive_path.clone(),
                    overwrite: true,
                    confirm: false,
                }))
                .await,
        );
        assert!(err.message.contains("confirm"));

        let restored = unwrap_ok(
            server
                .restore_app(Parameters(RestoreAppParams {
                    archive_path,
                    overwrite: true,
                    confirm: true,
                }))
                .await,
        );
        assert_eq!(restored["id"], "mcp-backup-app");
    }

    #[tokio::test]
    async fn update_app_force_without_confirm_is_rejected() {
        let (server, _dir) = server();
        let err = unwrap_err(
            server
                .update_app(Parameters(UpdateAppParams {
                    app_id: "does-not-matter".to_string(),
                    definition: serde_json::json!({}),
                    force: true,
                    confirm: false,
                }))
                .await,
        );
        assert!(err.message.contains("confirm"));
    }

    #[tokio::test]
    async fn uninstall_purge_without_confirm_is_rejected() {
        let (server, _dir) = server();
        let err = unwrap_err(
            server
                .uninstall_app(Parameters(UninstallAppParams {
                    app_id: "does-not-matter".to_string(),
                    purge_data: true,
                    confirm: false,
                }))
                .await,
        );
        assert!(err.message.contains("confirm"));
    }

    #[tokio::test]
    async fn list_automations_returns_the_apps_automations() {
        if !bundle_available() {
            eprintln!(
                "skipping: dist-cli/validate-app.mjs not built, run `pnpm build:mcp-validator`"
            );
            return;
        }
        let (server, _dir) = server();
        let definition = bookkeeping_definition("mcp-automations-app");
        unwrap_ok(
            server
                .install_app(Parameters(InstallAppParams { definition }))
                .await,
        );

        let automations = unwrap_ok(
            server
                .list_automations(Parameters(AppIdParams {
                    app_id: "mcp-automations-app".to_string(),
                }))
                .await,
        );
        assert!(!automations.as_array().unwrap().is_empty());
    }
}
