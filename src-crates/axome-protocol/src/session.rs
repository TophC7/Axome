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
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SessionSource {
    Harness {
        harness_id: String,
        /// Unnamed (`None`) first, then by name. Never empty.
        presets: Vec<Option<String>>,
    },
    Axo,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum SessionSourceKey {
    Harness {
        harness_id: String,
        preset: Option<String>,
    },
    Axo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Agent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LastMessage {
    pub role: MessageRole,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexedSession {
    pub source: SessionSource,
    pub session_id: String,
    pub title: Option<String>,
    pub last_message: Option<LastMessage>,
    pub message_count: Option<u32>,
    pub updated_at: Option<String>,
    /// Model used by the latest assistant turn, not the configured default.
    pub model: Option<String>,
    pub git_branch: Option<String>,
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
    pub source: SessionSourceKey,
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
