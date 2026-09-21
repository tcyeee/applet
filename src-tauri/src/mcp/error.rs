//! Maps this crate's `String` errors (the convention already used by
//! `commands.rs`) onto `rmcp`'s `ErrorData` so every `#[tool]` method can use
//! `.map_err(McpToolError)?` the same way `commands.rs` uses
//! `.map_err(|e| e.to_string())`. Messages are passed through verbatim —
//! `runtime::*` and `validator` already write them to be self-correcting for
//! an AI caller (see AGENTS.md).

use rmcp::ErrorData;

pub struct McpToolError(pub String);

impl From<String> for McpToolError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

impl From<McpToolError> for ErrorData {
    fn from(err: McpToolError) -> Self {
        ErrorData::internal_error(err.0, None)
    }
}
