use serde::{Deserialize, Serialize};

/// One host's shell launch options for a folder (or its global settings).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellListing {
    /// Program name of the bare shell, e.g. "fish".
    pub shell: String,
    pub presets: Vec<ShellPresetEntry>,
    pub default_preset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellPresetEntry {
    pub name: String,
    /// Program name the preset runs.
    pub shell: String,
}
