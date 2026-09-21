//! End-to-end MVP acceptance test (TODO step 5, "端到端打通").
//!
//! Exercises the same Runtime Core functions the MCP layer (TODO step 4) and
//! the desktop app's Tauri commands call — `registry`, `appdb`, `backup` —
//! against real file-backed SQLite under a temp directory, simulating an AI
//! agent iteratively building a personal bookkeeping app exactly the way each
//! TODO step 5 scenario describes. It intentionally does not go through the
//! MCP stdio transport or the Tauri command layer: both are thin wrappers
//! around these same functions (see AGENTS.md "MCP Interface Layer"), so
//! calling them directly is a faithful, hermetic stand-in that runs under
//! plain `cargo test` without a display server or a spawned MCP client.
//!
//! One `#[test]` per scenario, run in sequence against a single shared temp
//! `base_dir` (via `std::sync::OnceLock`-free manual sequencing inside a
//! single test) because scenario 2-5 each depend on state the previous
//! scenario left behind — that dependency chain *is* the thing being tested.

use std::path::{Path, PathBuf};

use applet_lib::app_schema::AppDefinition;
use applet_lib::runtime::{appdb, backup, paths, registry};
use rusqlite::Connection;
use serde_json::{json, Value};
use tempfile::TempDir;

const APP_ID: &str = "personal-bookkeeping";

fn definition(value: Value) -> AppDefinition {
    serde_json::from_value(value).unwrap()
}

/// Scenario 1 seed: a minimal bookkeeping app with just transactions — what
/// an AI agent would generate from "创建一个个人记账工具" before any
/// follow-up requests.
fn v1_definition() -> AppDefinition {
    definition(json!({
        "schemaVersion": "1",
        "id": APP_ID,
        "name": "个人记账工具",
        "description": "记录日常收支",
        "dataModel": {
            "entities": [{
                "id": "transaction",
                "name": "交易记录",
                "fields": [
                    { "id": "date", "type": "date", "label": "日期", "required": true },
                    { "id": "category", "type": "enum", "label": "分类", "required": true,
                      "options": ["food", "transport", "income", "other"] },
                    { "id": "amount", "type": "number", "label": "金额", "required": true }
                ]
            }]
        },
        "views": [
            { "id": "transactionList", "type": "list", "entityId": "transaction", "title": "交易列表",
              "fields": [{ "field": "date" }, { "field": "category" }, { "field": "amount" }] },
            { "id": "transactionForm", "type": "form", "entityId": "transaction", "title": "记一笔",
              "fields": [{ "field": "date" }, { "field": "category" }, { "field": "amount" }] }
        ],
        "actions": [
            { "id": "createTransaction", "type": "create", "entityId": "transaction" }
        ]
    }))
}

/// Scenario 2: "增加月度统计" — adds a `note` field to `transaction` (an
/// additive dataModel change requiring migration) plus a chart view. The
/// chart itself needs no schema support (it's client-side aggregation over
/// existing fields, see `src/ui-runtime/aggregate.ts`); the field addition is
/// what actually exercises the migration path this scenario is meant to
/// verify.
fn v2_definition() -> AppDefinition {
    definition(json!({
        "schemaVersion": "1",
        "id": APP_ID,
        "name": "个人记账工具",
        "description": "记录日常收支，按分类查看月度统计",
        "dataModel": {
            "entities": [{
                "id": "transaction",
                "name": "交易记录",
                "fields": [
                    { "id": "date", "type": "date", "label": "日期", "required": true },
                    { "id": "category", "type": "enum", "label": "分类", "required": true,
                      "options": ["food", "transport", "income", "other"] },
                    { "id": "amount", "type": "number", "label": "金额", "required": true },
                    { "id": "note", "type": "string", "label": "备注" }
                ]
            }]
        },
        "views": [
            { "id": "transactionList", "type": "list", "entityId": "transaction", "title": "交易列表",
              "fields": [{ "field": "date" }, { "field": "category" }, { "field": "amount" }] },
            { "id": "transactionForm", "type": "form", "entityId": "transaction", "title": "记一笔",
              "fields": [{ "field": "date" }, { "field": "category" }, { "field": "amount" }, { "field": "note" }] },
            { "id": "monthlyCategoryChart", "type": "chart", "entityId": "transaction", "title": "本月分类支出统计",
              "chart": { "groupBy": "category", "aggregate": "sum", "metricField": "amount" } }
        ],
        "actions": [
            { "id": "createTransaction", "type": "create", "entityId": "transaction" }
        ]
    }))
}

/// Scenario 3: "增加预算功能" — adds a whole new `budget` entity on top of
/// v2's already-migrated `transaction` entity, then a further iteration
/// (v3b) adds a field to *that* new entity too, to prove the migration path
/// stays safe across repeated iterations, not just a single one-shot change.
fn v3_definition() -> AppDefinition {
    definition(json!({
        "schemaVersion": "1",
        "id": APP_ID,
        "name": "个人记账工具",
        "description": "记录日常收支，按分类查看月度统计，并对预算超支进行提醒",
        "dataModel": {
            "entities": [
                {
                    "id": "transaction",
                    "name": "交易记录",
                    "fields": [
                        { "id": "date", "type": "date", "label": "日期", "required": true },
                        { "id": "category", "type": "enum", "label": "分类", "required": true,
                          "options": ["food", "transport", "income", "other"] },
                        { "id": "amount", "type": "number", "label": "金额", "required": true },
                        { "id": "note", "type": "string", "label": "备注" }
                    ]
                },
                {
                    "id": "budget",
                    "name": "预算",
                    "fields": [
                        { "id": "category", "type": "enum", "label": "分类", "required": true,
                          "options": ["food", "transport", "income", "other"] },
                        { "id": "monthlyLimit", "type": "number", "label": "月度上限", "required": true }
                    ]
                }
            ]
        },
        "views": [
            { "id": "transactionList", "type": "list", "entityId": "transaction", "title": "交易列表",
              "fields": [{ "field": "date" }, { "field": "category" }, { "field": "amount" }] },
            { "id": "budgetList", "type": "list", "entityId": "budget", "title": "预算设置",
              "fields": [{ "field": "category" }, { "field": "monthlyLimit" }] }
        ],
        "actions": [
            { "id": "createTransaction", "type": "create", "entityId": "transaction" }
        ]
    }))
}

fn v3b_definition() -> AppDefinition {
    definition(json!({
        "schemaVersion": "1",
        "id": APP_ID,
        "name": "个人记账工具",
        "description": "记录日常收支，按分类查看月度统计，并对预算超支进行提醒",
        "dataModel": {
            "entities": [
                {
                    "id": "transaction",
                    "name": "交易记录",
                    "fields": [
                        { "id": "date", "type": "date", "label": "日期", "required": true },
                        { "id": "category", "type": "enum", "label": "分类", "required": true,
                          "options": ["food", "transport", "income", "other"] },
                        { "id": "amount", "type": "number", "label": "金额", "required": true },
                        { "id": "note", "type": "string", "label": "备注" }
                    ]
                },
                {
                    "id": "budget",
                    "name": "预算",
                    "fields": [
                        { "id": "category", "type": "enum", "label": "分类", "required": true,
                          "options": ["food", "transport", "income", "other"] },
                        { "id": "monthlyLimit", "type": "number", "label": "月度上限", "required": true },
                        { "id": "note", "type": "string", "label": "备注" }
                    ]
                }
            ]
        },
        "views": [
            { "id": "transactionList", "type": "list", "entityId": "transaction", "title": "交易列表",
              "fields": [{ "field": "date" }, { "field": "category" }, { "field": "amount" }] },
            { "id": "budgetList", "type": "list", "entityId": "budget", "title": "预算设置",
              "fields": [{ "field": "category" }, { "field": "monthlyLimit" }, { "field": "note" }] }
        ],
        "actions": [
            { "id": "createTransaction", "type": "create", "entityId": "transaction" }
        ]
    }))
}

fn open_registry(base_dir: &Path) -> Connection {
    let conn = Connection::open(paths::registry_db_path(base_dir)).unwrap();
    registry::init_schema(&conn).unwrap();
    conn
}

fn open_app_db(base_dir: &Path) -> Connection {
    appdb::open(&paths::app_db_path(base_dir, APP_ID)).unwrap()
}

fn insert_transaction(conn: &Connection, date: &str, category: &str, amount: f64) -> Value {
    let entity = &v1_definition().data_model.entities[0];
    let mut values = serde_json::Map::new();
    values.insert("date".into(), json!(date));
    values.insert("category".into(), json!(category));
    values.insert("amount".into(), json!(amount));
    appdb::insert_record(conn, entity, &values).unwrap()
}

#[test]
fn mvp_end_to_end_scenarios() {
    let tmp = TempDir::new().unwrap();
    let base_dir: PathBuf = tmp.path().to_path_buf();

    // ---- Scenario 1: create the app via a declarative definition, with a
    // data table + entry form + list page. ----
    let registry_conn = open_registry(&base_dir);
    let record = registry::install(&registry_conn, &v1_definition()).unwrap();
    assert_eq!(record.revision, 1);
    assert_eq!(record.status, registry::AppStatus::Stopped);

    let mut app_conn = open_app_db(&base_dir);
    appdb::create_schema(&app_conn, &record.definition.data_model).unwrap();

    insert_transaction(&app_conn, "2026-01-05", "food", 32.5);
    insert_transaction(&app_conn, "2026-01-08", "transport", 12.0);

    let transaction_entity = &v1_definition().data_model.entities[0];
    let records = appdb::list_records(&app_conn, transaction_entity).unwrap();
    assert_eq!(records.len(), 2);

    // ---- Scenario 2: "增加月度统计" — additive dataModel change (new
    // `note` field) must migrate without dropping the two rows just entered. ----
    appdb::migrate_schema(
        &mut app_conn,
        &v1_definition().data_model,
        &v2_definition().data_model,
        false,
    )
    .expect("additive migration must not be refused");
    registry::update_definition(&registry_conn, APP_ID, &v2_definition()).unwrap();

    let transaction_entity_v2 = &v2_definition().data_model.entities[0];
    let records = appdb::list_records(&app_conn, transaction_entity_v2).unwrap();
    assert_eq!(records.len(), 2, "existing rows must survive the migration");
    let amounts: Vec<f64> = records
        .iter()
        .map(|r| r["amount"].as_f64().unwrap())
        .collect();
    assert!(amounts.contains(&32.5) && amounts.contains(&12.0));
    assert!(
        records.iter().all(|r| r["note"].is_null()),
        "new column should default to null, not synthesize data"
    );

    let after_v2 = registry::get(&registry_conn, APP_ID).unwrap().unwrap();
    assert_eq!(after_v2.revision, 2);

    // ---- Scenario 3: "增加预算功能" — a whole new entity, then a further
    // iteration adds a field to *that* entity, proving repeated iterations
    // stay safe (not just a single migration). ----
    appdb::migrate_schema(
        &mut app_conn,
        &v2_definition().data_model,
        &v3_definition().data_model,
        false,
    )
    .expect("new entity is an additive change");
    registry::update_definition(&registry_conn, APP_ID, &v3_definition()).unwrap();

    let budget_entity = &v3_definition().data_model.entities[1];
    let mut budget_values = serde_json::Map::new();
    budget_values.insert("category".into(), json!("food"));
    budget_values.insert("monthlyLimit".into(), json!(500));
    appdb::insert_record(&app_conn, budget_entity, &budget_values).unwrap();

    appdb::migrate_schema(
        &mut app_conn,
        &v3_definition().data_model,
        &v3b_definition().data_model,
        false,
    )
    .expect("second additive iteration on the new entity must also succeed");
    registry::update_definition(&registry_conn, APP_ID, &v3b_definition()).unwrap();

    let after_v3b = registry::get(&registry_conn, APP_ID).unwrap().unwrap();
    assert_eq!(
        after_v3b.revision, 4,
        "three update_definition calls since install (v2, v3, v3b)"
    );

    let transaction_entity_v3b = &v3b_definition().data_model.entities[0];
    let budget_entity_v3b = &v3b_definition().data_model.entities[1];
    assert_eq!(
        appdb::list_records(&app_conn, transaction_entity_v3b)
            .unwrap()
            .len(),
        2,
        "transaction rows from scenario 1 must still be there after two more migrations"
    );
    let budget_records = appdb::list_records(&app_conn, budget_entity_v3b).unwrap();
    assert_eq!(budget_records.len(), 1);
    assert_eq!(budget_records[0]["monthlyLimit"].as_f64(), Some(500.0));
    assert!(budget_records[0]["note"].is_null());

    // ---- Scenario 4: "帮我备份数据" — round-trip through backup_app / restore_app. ----
    let pre_backup_transactions = appdb::list_records(&app_conn, transaction_entity_v3b).unwrap();
    let pre_backup_budgets = appdb::list_records(&app_conn, budget_entity_v3b).unwrap();

    let archive_path = backup::backup_app(&base_dir, &registry_conn, APP_ID).unwrap();
    assert!(archive_path.exists());

    // Simulate catastrophic data loss: drop the connection and delete the
    // live database file entirely (not just its rows) before restoring.
    drop(app_conn);
    std::fs::remove_file(paths::app_db_path(&base_dir, APP_ID)).unwrap();

    let restored_record =
        backup::restore_app(&base_dir, &registry_conn, &archive_path, true).unwrap();
    assert_eq!(restored_record.id, APP_ID);
    assert_eq!(restored_record.revision, 4);

    let app_conn = open_app_db(&base_dir);
    let restored_transactions = appdb::list_records(&app_conn, transaction_entity_v3b).unwrap();
    let restored_budgets = appdb::list_records(&app_conn, budget_entity_v3b).unwrap();
    assert_eq!(restored_transactions.len(), pre_backup_transactions.len());
    assert_eq!(restored_budgets.len(), pre_backup_budgets.len());
    assert_eq!(
        restored_budgets[0]["monthlyLimit"],
        pre_backup_budgets[0]["monthlyLimit"]
    );
    drop(app_conn);

    // ---- Scenario 5: kill the runtime and reopen it — nothing above may
    // have depended on process state, only on what's on disk under `base_dir`. ----
    drop(registry_conn);

    let reopened_registry = open_registry(&base_dir);
    let apps = registry::list(&reopened_registry).unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].id, APP_ID);
    assert_eq!(apps[0].revision, 4);
    assert_eq!(apps[0].definition.data_model.entities.len(), 2);

    let reopened_app_conn = open_app_db(&base_dir);
    let final_transactions =
        appdb::list_records(&reopened_app_conn, transaction_entity_v3b).unwrap();
    let final_budgets = appdb::list_records(&reopened_app_conn, budget_entity_v3b).unwrap();
    assert_eq!(final_transactions.len(), 2);
    assert_eq!(final_budgets.len(), 1);
}
