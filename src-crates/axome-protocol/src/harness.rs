use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessStatus {
    pub id: String,
    pub label: String,
    pub status: HarnessConnectionStatus,
    pub enabled: bool,
    pub presets: Vec<String>,
    pub version: Option<String>,
    pub resolved_path: Option<String>,
    pub message: Option<String>,
    pub install_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HarnessConnectionStatus {
    Connected,
    Missing,
    Error,
}

/// One host's harnesses for a folder (or its global settings), plus that scope's `sessions.default_preset`.
/// The default travels here because `settings_get_effective` ignores the folder layer of remote folders.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessListing {
    pub harnesses: Vec<HarnessStatus>,
    pub default_preset: Option<String>,
}
