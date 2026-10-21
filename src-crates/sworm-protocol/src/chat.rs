use serde::{Deserialize, Serialize};

/// Result of `chat_start`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatStartInfo {
    /// Session id bound to the tab: loaded, or prompted at least once. None for a fresh, unprompted session.
    pub session_id: Option<String>,
    /// The requested session was loaded, or an existing run was reattached.
    pub resumed: bool,
}

/// ACP-independent updates for one chat run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChatEvent {
    /// Replaces everything the subscriber holds; first event of every attachment.
    Snapshot {
        state: ChatState,
    },
    TurnState {
        turn: ChatTurn,
    },
    /// Insert, or replace the entry with the same id.
    Entry {
        entry: ChatEntry,
    },
    /// Append to the text of a `message` or `thought` entry.
    TextDelta {
        id: String,
        text: String,
    },
    ConfigOptions {
        options: Vec<ChatConfigOption>,
    },
    Commands {
        commands: Vec<ChatCommand>,
    },
    Usage {
        usage: ChatUsage,
    },
    Request {
        request: ChatRequest,
    },
    RequestResolved {
        request_id: String,
    },
    SessionBound {
        session_id: String,
    },
    Notice {
        level: ChatNoticeLevel,
        message: String,
    },
    /// The run ended; nothing follows.
    Exit {
        message: Option<String>,
    },
}

/// Complete state sent when a subscriber attaches.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatState {
    pub session_id: Option<String>,
    pub turn: ChatTurn,
    pub entries: Vec<ChatEntry>,
    pub config_options: Vec<ChatConfigOption>,
    pub commands: Vec<ChatCommand>,
    pub usage: Option<ChatUsage>,
    pub requests: Vec<ChatRequest>,
    pub accepts_images: bool,
}

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
        title: String,
        tool_kind: String,
        status: ChatToolStatus,
        content: Vec<ChatToolContent>,
        locations: Vec<ChatLocation>,
    },
    Plan {
        id: String,
        items: Vec<ChatPlanItem>,
    },
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

/// New value submitted for a configuration option.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatConfigValue {
    Select { value: String },
    Boolean { value: bool },
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

/// Pending user interaction requested by the agent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatRequest {
    Permission {
        request_id: String,
        tool_call_id: String,
        title: String,
        options: Vec<ChatPermissionOption>,
    },
    Elicitation {
        request_id: String,
        message: String,
        schema: serde_json::Value,
    },
}

/// One agent-provided permission decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatPermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: String,
}

/// User response to a pending interaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ChatResponse {
    /// None = cancelled.
    Permission { option_id: Option<String> },
    Elicitation {
        action: ChatElicitationAction,
        content: Option<serde_json::Map<String, serde_json::Value>>,
    },
}

/// Disposition of an elicitation request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatElicitationAction {
    Accept,
    Decline,
    Cancel,
}

/// Severity of a chat notice.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatNoticeLevel {
    Info,
    Warning,
    Error,
}
