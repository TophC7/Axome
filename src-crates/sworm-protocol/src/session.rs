use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Result of `session_start`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartInfo {
    /// Whether the harness resumed an existing conversation.
    pub resumed: bool,
    /// Session id the process is known to own at spawn time; null until discovery binds one.
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionView {
    Chat,
    Terminal,
}
