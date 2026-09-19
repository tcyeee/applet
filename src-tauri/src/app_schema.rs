//! Rust-side mirror of the App Schema's data model (see `src/app-schema/types.ts`).
//!
//! This is deliberately NOT a validator. Per `AGENTS.md`, `validateAppDefinition`
//! (TypeScript, `src/app-schema/validate.ts`) is the only sanctioned place that
//! enforces App Schema business rules (id formats, cross-references, etc). The
//! frontend must validate a definition before ever invoking a Tauri command
//! with it. Everything in this module exists only so the Rust runtime can
//! deserialize an already-validated definition and generate SQL from its
//! `dataModel` — it must not grow into a second validator.
//!
//! `views` and `actions` are kept as opaque JSON (`serde_json::Value`) because
//! the runtime core does not need to interpret them yet (that's the UI Runtime
//! in TODO step 3 and the MCP/action-execution layer in step 4). `automations`
//! are typed just enough for the scheduler to read cron triggers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppDefinition {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "dataModel")]
    pub data_model: DataModel,
    #[serde(default)]
    pub views: Vec<serde_json::Value>,
    #[serde(default)]
    pub actions: Vec<serde_json::Value>,
    #[serde(default)]
    pub automations: Vec<Automation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataModel {
    pub entities: Vec<Entity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldBase {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub unique: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cardinality {
    One,
    Many,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Field {
    #[serde(rename = "string")]
    String(SimpleField),
    #[serde(rename = "number")]
    Number(SimpleField),
    #[serde(rename = "boolean")]
    Boolean(SimpleField),
    #[serde(rename = "date")]
    Date(SimpleField),
    #[serde(rename = "datetime")]
    Datetime(SimpleField),
    #[serde(rename = "enum")]
    Enum(EnumField),
    #[serde(rename = "reference")]
    Reference(ReferenceField),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleField {
    #[serde(flatten)]
    pub base: FieldBase,
    #[serde(default)]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumField {
    #[serde(flatten)]
    pub base: FieldBase,
    pub options: Vec<String>,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceField {
    #[serde(flatten)]
    pub base: FieldBase,
    #[serde(rename = "entityId")]
    pub entity_id: String,
    pub cardinality: Cardinality,
}

impl Field {
    pub fn base(&self) -> &FieldBase {
        match self {
            Field::String(f)
            | Field::Number(f)
            | Field::Boolean(f)
            | Field::Date(f)
            | Field::Datetime(f) => &f.base,
            Field::Enum(f) => &f.base,
            Field::Reference(f) => &f.base,
        }
    }

    pub fn id(&self) -> &str {
        &self.base().id
    }

    /// A coarse type discriminant used to detect "did the field's type change"
    /// during migration; deliberately ignores enum options / reference target
    /// so that e.g. adding an enum option isn't treated as a type change.
    pub fn type_tag(&self) -> &'static str {
        match self {
            Field::String(_) => "string",
            Field::Number(_) => "number",
            Field::Boolean(_) => "boolean",
            Field::Date(_) => "date",
            Field::Datetime(_) => "datetime",
            Field::Enum(_) => "enum",
            Field::Reference(ReferenceField {
                cardinality: Cardinality::One,
                ..
            }) => "reference:one",
            Field::Reference(ReferenceField {
                cardinality: Cardinality::Many,
                ..
            }) => "reference:many",
        }
    }

    /// SQLite column type for fields that get a real column (i.e. not a
    /// many-cardinality reference, which becomes a link table instead).
    pub fn sql_type(&self) -> &'static str {
        match self {
            Field::String(_) | Field::Date(_) | Field::Datetime(_) | Field::Enum(_) => "TEXT",
            Field::Number(_) => "REAL",
            Field::Boolean(_) => "INTEGER",
            Field::Reference(ReferenceField {
                cardinality: Cardinality::One,
                ..
            }) => "TEXT",
            Field::Reference(ReferenceField {
                cardinality: Cardinality::Many,
                ..
            }) => {
                unreachable!("many-cardinality references do not have a column")
            }
        }
    }

    pub fn is_many_reference(&self) -> bool {
        matches!(
            self,
            Field::Reference(ReferenceField {
                cardinality: Cardinality::Many,
                ..
            })
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Automation {
    pub id: String,
    pub trigger: AutomationTrigger,
    /// Kept opaque (`{ kind: "runAction" | "summarize" | "notify", ... }`);
    /// interpreting it is the job of the future action-execution engine.
    pub action: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationTrigger {
    #[serde(rename = "type")]
    pub kind: String,
    pub expression: String,
}
