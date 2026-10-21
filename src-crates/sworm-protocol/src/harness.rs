use crate::session::SessionView;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessStatus {
    pub id: String,
    pub label: String,
    pub status: HarnessConnectionStatus,
    pub enabled: bool,
    pub presets: Vec<HarnessPresetEntry>,
    pub version: Option<String>,
    pub resolved_path: Option<String>,
    pub message: Option<String>,
    pub install_hint: String,
}

/// A named harness preset with its resolved default view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessPresetEntry {
    pub name: String,
    /// Resolved default view: the preset's view, or terminal when unset or the harness has no chat agent.
    pub view: SessionView,
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
