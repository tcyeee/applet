//! Automation scheduler: watches each *running* app's `automations` (cron
//! triggers on the App Schema, see `src/app-schema/types.ts`) and, once a
//! minute, fires a `runtime://automation` event for every trigger whose cron
//! expression matches the current minute.
//!
//! Interpreting the fired automation's `action` (running a CRUD action,
//! computing a summary, showing a notification) is deliberately out of
//! scope here — that's the action-execution engine that TODO step 4 (MCP
//! layer) / step 3 (UI runtime) will own. This module's job is purely
//! "detect the trigger and dispatch it", per the TODO step 2 Scheduler item.
//!
//! App Schema cron expressions are plain 5-field Vixie cron (`min hour dom
//! month dow`, see the example apps). The `cron` crate requires a leading
//! seconds field, so we prepend `"0 "` before parsing.

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use chrono::{Timelike, Utc};
use cron::Schedule;
use rusqlite::Connection;
use tauri::{AppHandle, Emitter};

use crate::runtime::paths;
use crate::runtime::registry::{self, AppStatus};

#[derive(Debug)]
pub struct CronError(pub String);

impl std::fmt::Display for CronError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid cron expression: {}", self.0)
    }
}
impl std::error::Error for CronError {}

pub fn parse_cron(expression: &str) -> Result<Schedule, CronError> {
    let with_seconds = format!("0 {}", expression.trim());
    Schedule::from_str(&with_seconds).map_err(|e| CronError(e.to_string()))
}

pub struct Scheduler {
    stop_flag: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Scheduler {
    pub fn spawn(app_handle: AppHandle, base_dir: std::path::PathBuf) -> Self {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_thread = stop_flag.clone();
        // (app_id, automation_id) -> minute string last fired for, so a
        // one-second poll granularity doesn't fire the same trigger twice.
        let last_fired: Mutex<std::collections::HashMap<(String, String), String>> =
            Mutex::new(std::collections::HashMap::new());

        let handle = std::thread::spawn(move || {
            while !stop_flag_thread.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_secs(1));
                let now = Utc::now();
                if now.second() != 0 {
                    continue;
                }
                let minute_key = now.format("%Y-%m-%dT%H:%M").to_string();
                if let Err(e) = tick(&app_handle, &base_dir, &now, &minute_key, &last_fired) {
                    eprintln!("scheduler tick failed: {e}");
                }
            }
        });

        Self {
            stop_flag,
            handle: Some(handle),
        }
    }

    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.stop();
    }
}

fn tick(
    app_handle: &AppHandle,
    base_dir: &std::path::Path,
    now: &chrono::DateTime<Utc>,
    minute_key: &str,
    last_fired: &Mutex<std::collections::HashMap<(String, String), String>>,
) -> Result<(), String> {
    let conn = Connection::open(paths::registry_db_path(base_dir)).map_err(|e| e.to_string())?;
    let apps = registry::list(&conn).map_err(|e| e.to_string())?;

    for app in apps.iter().filter(|a| a.status == AppStatus::Running) {
        for automation in &app.definition.automations {
            if automation.trigger.kind != "cron" {
                continue;
            }
            let schedule = match parse_cron(&automation.trigger.expression) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("app \"{}\" automation \"{}\": {e}", app.id, automation.id);
                    continue;
                }
            };
            if !schedule.includes(*now) {
                continue;
            }
            let key = (app.id.clone(), automation.id.clone());
            {
                let mut guard = last_fired.lock().unwrap();
                if guard.get(&key).map(|s| s.as_str()) == Some(minute_key) {
                    continue;
                }
                guard.insert(key, minute_key.to_string());
            }
            let payload = serde_json::json!({
                "appId": app.id,
                "automationId": automation.id,
                "action": automation.action,
                "firedAt": now.to_rfc3339(),
            });
            if let Err(e) = app_handle.emit("runtime://automation", payload) {
                eprintln!("failed to emit automation event: {e}");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn parses_five_field_vixie_cron_from_examples() {
        // From src/app-schema/examples/bookkeeping.json.
        parse_cron("0 0 1 * *").unwrap();
        parse_cron("0 9 * * *").unwrap();
    }

    #[test]
    fn matches_the_expected_minute() {
        let schedule = parse_cron("30 9 * * *").unwrap();
        let matching = Utc.with_ymd_and_hms(2026, 1, 15, 9, 30, 0).unwrap();
        let not_matching = Utc.with_ymd_and_hms(2026, 1, 15, 9, 31, 0).unwrap();
        assert!(schedule.includes(matching));
        assert!(!schedule.includes(not_matching));
    }

    #[test]
    fn rejects_garbage_expression() {
        assert!(parse_cron("not a cron").is_err());
    }
}
