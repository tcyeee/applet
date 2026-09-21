//! Input parameter types for every MCP tool in `mcp::AppletMcpServer`.
//!
//! `serde_json::Value` is used for App-defined shapes we don't want a second
//! Rust type for (`AppDefinition`, a record's field `values`) — the
//! `app_schema`/`appdb` layers already parse and validate those; duplicating
//! a schemars-typed mirror here would just be a second place to keep in sync.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema, Default)]
pub struct EmptyParams {}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppIdParams {
    /// The installed app's id (`AppDefinition.id`).
    pub app_id: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallAppParams {
    /// A full App definition (validated against the App Schema before install).
    pub definition: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAppParams {
    pub app_id: String,
    /// The new full App definition (validated before anything is applied).
    pub definition: serde_json::Value,
    /// Allow a migration that drops or reinterprets existing data (removed
    /// entity/field, changed field type). Requires `confirm: true`.
    #[serde(default)]
    pub force: bool,
    /// Must be `true` when `force` is `true` — a structural acknowledgement
    /// that this can lose data, not just a description of the risk.
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UninstallAppParams {
    pub app_id: String,
    /// Also delete the app's database and files (irreversible). Requires
    /// `confirm: true`.
    #[serde(default)]
    pub purge_data: bool,
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListRecordsParams {
    pub app_id: String,
    pub entity_id: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateRecordParams {
    pub app_id: String,
    pub entity_id: String,
    /// A JSON object of `{ fieldId: value }`. Unknown field ids are rejected;
    /// omitted fields fall back to their schema-declared default.
    pub values: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRecordParams {
    pub app_id: String,
    pub entity_id: String,
    pub record_id: String,
    pub values: serde_json::Value,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRecordParams {
    pub app_id: String,
    pub entity_id: String,
    pub record_id: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct WriteAppFileParams {
    pub app_id: String,
    /// Slash-separated path relative to the app's private file storage.
    pub path: String,
    /// File contents, base64-encoded (MCP tool payloads are JSON/text).
    pub contents_base64: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppFilePathParams {
    pub app_id: String,
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct WriteSharedFileParams {
    pub path: String,
    pub contents_base64: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct SharedFilePathParams {
    pub path: String,
}

#[derive(Debug, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestoreAppParams {
    /// Absolute path to a `.zip` produced by `backup_app`/`backup_all_apps`.
    pub archive_path: String,
    /// Overwrite an already-installed app with the same id (irreversible for
    /// its current data). Requires `confirm: true`.
    #[serde(default)]
    pub overwrite: bool,
    #[serde(default)]
    pub confirm: bool,
}
