//! Bridge to the *only* sanctioned App Schema validator, `validateAppDefinition`
//! (`src/app-schema/validate.ts`, Zod-based). Per AGENTS.md, App Schema
//! business-rule validation must not be reimplemented in Rust — so instead of
//! porting Zod rules here, `create_app`/`update_app` spawn a Node subprocess
//! running the bundled `dist-cli/validate-app.mjs` (built by
//! `pnpm build:mcp-validator` from `src/app-schema/validate-cli.ts`) and trust
//! its verdict, exactly like the React frontend trusts `validateAppDefinition`
//! before ever calling a Tauri command.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ValidatorOutput {
    success: bool,
    #[serde(default)]
    data: Option<serde_json::Value>,
    #[serde(default)]
    errors: Option<Vec<String>>,
}

pub enum ValidationOutcome {
    Valid(serde_json::Value),
    Invalid(Vec<String>),
}

/// Resolves the validator bundle path: `APPLET_VALIDATOR_PATH` env var if
/// set, otherwise `<repo root>/dist-cli/validate-app.mjs` (dev layout, one
/// level up from `src-tauri/`).
fn validator_path() -> PathBuf {
    if let Ok(path) = std::env::var("APPLET_VALIDATOR_PATH") {
        return PathBuf::from(path);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("dist-cli")
        .join("validate-app.mjs")
}

pub fn validate_app_definition(
    definition: &serde_json::Value,
) -> Result<ValidationOutcome, String> {
    let script = validator_path();
    if !script.exists() {
        return Err(format!(
            "App Schema validator bundle not found at \"{}\". Run `pnpm build:mcp-validator` \
             (or set APPLET_VALIDATOR_PATH) before using create_app/update_app.",
            script.display()
        ));
    }

    let mut child = Command::new("node")
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn \"node {}\": {e}", script.display()))?;

    let input = serde_json::to_vec(definition).map_err(|e| e.to_string())?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(&input)
        .map_err(|e| format!("failed to write to validator stdin: {e}"))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to read validator output: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "App Schema validator exited with an error: {stderr}"
        ));
    }

    let parsed: ValidatorOutput = serde_json::from_slice(&output.stdout).map_err(|e| {
        format!(
            "App Schema validator produced unparseable output: {e} (stdout: {})",
            String::from_utf8_lossy(&output.stdout)
        )
    })?;

    Ok(if parsed.success {
        ValidationOutcome::Valid(parsed.data.unwrap_or(serde_json::Value::Null))
    } else {
        ValidationOutcome::Invalid(parsed.errors.unwrap_or_default())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle_available() -> bool {
        validator_path().exists()
    }

    #[test]
    fn validates_the_bookkeeping_example() {
        if !bundle_available() {
            eprintln!(
                "skipping: dist-cli/validate-app.mjs not built, run `pnpm build:mcp-validator`"
            );
            return;
        }
        let json = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("src/app-schema/examples/bookkeeping.json"),
        )
        .unwrap();
        let definition: serde_json::Value = serde_json::from_str(&json).unwrap();
        match validate_app_definition(&definition).unwrap() {
            ValidationOutcome::Valid(_) => {}
            ValidationOutcome::Invalid(errors) => panic!("expected valid, got errors: {errors:?}"),
        }
    }

    #[test]
    fn rejects_an_incomplete_definition() {
        if !bundle_available() {
            eprintln!(
                "skipping: dist-cli/validate-app.mjs not built, run `pnpm build:mcp-validator`"
            );
            return;
        }
        let definition = serde_json::json!({ "id": "not-enough" });
        match validate_app_definition(&definition).unwrap() {
            ValidationOutcome::Valid(_) => panic!("expected validation to fail"),
            ValidationOutcome::Invalid(errors) => assert!(!errors.is_empty()),
        }
    }
}
