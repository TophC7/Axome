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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexedSession {
    pub harness_id: String,
    pub session_id: String,
    pub title: Option<String>,
    pub updated_at: Option<String>,
    pub model: Option<String>,
    pub git_branch: Option<String>,
    /// Presets whose source listed it: unnamed (`None`) first, then by name. Never empty.
    pub presets: Vec<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionIndexStatus {
    Pending,
    Ready,
    Missing,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexSource {
    pub harness_id: String,
    pub preset: Option<String>,
    pub status: SessionIndexStatus,
    pub refreshing: bool,
    pub refreshed_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionIndexSnapshot {
    pub sessions: Vec<IndexedSession>,
    pub sources: Vec<SessionIndexSource>,
}
