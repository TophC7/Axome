//! Chat transcript model rendered by the dormant chat components; Phase 5 reshapes it around axo.

use serde::{Deserialize, Serialize};

/// Current chat run activity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatTurn {
    Starting,
    Idle,
    Running,
    Reconnecting,
}

/// One ordered transcript entry, identified within a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatEntry {
    User {
        id: String,
        content: Vec<ChatContent>,
    },
    Message {
        id: String,
        text: String,
    },
    Thought {
        id: String,
        text: String,
    },
    Tool {
        id: String,
        /// Programmatic tool name, when the agent reports one.
        name: Option<String>,
        title: String,
        tool_kind: String,
        status: ChatToolStatus,
        /// Raw tool arguments.
        input: Option<serde_json::Value>,
        content: Vec<ChatToolContent>,
        locations: Vec<ChatLocation>,
    },
    Plan {
        id: String,
        items: Vec<ChatPlanItem>,
    },
    /// One per prompted turn. Absent for replayed turns.
    TurnEnd {
        id: String,
        stop_reason: ChatStopReason,
        usage: Option<ChatTokenUsage>,
        error: Option<String>,
    },
}

/// Why a prompted turn ended.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatStopReason {
    EndTurn,
    Cancelled,
    Refusal,
    MaxTokens,
    MaxTurnRequests,
    /// The prompt request failed; `error` carries the message.
    Error,
}

/// Token usage the agent reported for one prompt turn.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatTokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
}

/// Execution status of a tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatToolStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

/// Displayable output from a tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatToolContent {
    Text {
        text: String,
    },
    Diff {
        path: String,
        old_text: Option<String>,
        new_text: String,
    },
}

/// File location associated with a tool call.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatLocation {
    pub path: String,
    pub line: Option<u32>,
}

/// One item in the agent's plan.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatPlanItem {
    pub content: String,
    pub priority: String,
    pub status: String,
}

/// User prompt content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatContent {
    Text {
        text: String,
    },
    Image {
        mime_type: String,
        /// Base64-encoded image bytes.
        data: String,
    },
}

/// Agent-provided session configuration option.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatConfigOption {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub kind: ChatConfigKind,
}

/// Supported configuration controls and their current values.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatConfigKind {
    Select {
        current_value: String,
        choices: Vec<ChatConfigChoice>,
    },
    Boolean {
        current_value: bool,
    },
}

/// One selectable configuration value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatConfigChoice {
    pub value: String,
    pub name: String,
    pub description: Option<String>,
    pub group: Option<String>,
}

/// Slash command advertised by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCommand {
    pub name: String,
    pub description: String,
    pub hint: Option<String>,
}

/// Context usage and optional accumulated cost.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatUsage {
    pub used: u64,
    pub size: u64,
    pub cost: Option<ChatCost>,
}

/// Agent-reported monetary cost.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatCost {
    pub amount: f64,
    pub currency: String,
}

/// One agent-provided permission decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatPermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}
