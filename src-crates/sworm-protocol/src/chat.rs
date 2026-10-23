//! Chat wire model: axo session entries, live state and events as clients render them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ChatThinking {
    Off,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatModel {
    /// "provider/id" selector.
    pub selector: String,
    pub provider: String,
    pub name: String,
    /// Supported levels, lowest first; `[Off]` without reasoning.
    pub thinking: Vec<ChatThinking>,
    pub context_window: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatModelCatalog {
    /// Models of providers whose credentials resolve on the host, sorted by provider then name.
    pub models: Vec<ChatModel>,
    /// axo `defaultModel` when it is in `models`.
    pub default_model: Option<String>,
    pub default_thinking: ChatThinking,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ChatBlock {
    Text {
        text: String,
    },
    /// Redacted thinking has empty text.
    Thinking {
        text: String,
        redacted: bool,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub total_tokens: u64,
    /// USD.
    pub cost: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChatStopReason {
    Stop,
    Length,
    ToolUse,
    Error,
    Aborted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChatRunOutcome {
    Done,
    Aborted,
    Failed,
    Interrupted,
}

/// One displayable entry on the session's active branch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatEntry {
    pub id: String,
    pub timestamp: String,
    pub body: ChatEntryBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ChatEntryBody {
    /// `anchor`: where an edit of this message branches from (`None` = session start).
    /// `siblings`: ids of every user message with the same anchor (itself included), oldest first.
    User {
        text: String,
        anchor: Option<String>,
        siblings: Vec<String>,
    },
    Assistant {
        blocks: Vec<ChatBlock>,
        /// "provider/model" selector.
        model: String,
        usage: ChatUsage,
        stop_reason: ChatStopReason,
        error: Option<String>,
        duration_ms: Option<u64>,
    },
    ToolResult {
        call_id: String,
        tool_name: String,
        content: Vec<ChatContent>,
        details: Option<serde_json::Value>,
        is_error: bool,
        duration_ms: Option<u64>,
    },
    /// A host message axo marks for display.
    Notice {
        text: String,
    },
    ModelChange {
        model: String,
    },
    ThinkingChange {
        thinking: ChatThinking,
    },
    Compaction {
        summary: String,
        tokens_before: u64,
    },
    BranchSummary {
        summary: String,
    },
    RunEnd {
        outcome: ChatRunOutcome,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChatMaintenance {
    Compact,
    Shake,
    BranchSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatToolRun {
    pub call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    /// Latest partial output.
    pub output: Vec<ChatContent>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ChatQueueKind {
    Steer,
    FollowUp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatQueued {
    pub kind: ChatQueueKind,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatQuestionOption {
    pub label: String,
    pub description: Option<String>,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatQuestion {
    pub id: String,
    pub question: String,
    pub header: Option<String>,
    pub options: Vec<ChatQuestionOption>,
    pub multi: bool,
    pub recommended: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatQuestions {
    pub call_id: String,
    pub questions: Vec<ChatQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatAnswer {
    pub id: String,
    pub selected: Vec<String>,
    pub custom: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatRetry {
    pub attempt: u32,
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatStats {
    /// total_tokens of the last response on the active branch.
    pub context_tokens: u64,
    /// Current model's window; 0 when unknown.
    pub context_window: u64,
    /// Sums over every response in the session file.
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatLive {
    pub title: Option<String>,
    pub running: bool,
    pub maintenance: Option<ChatMaintenance>,
    pub model: Option<String>,
    pub thinking: ChatThinking,
    pub tools: Vec<ChatToolRun>,
    pub queue: Vec<ChatQueued>,
    pub questions: Vec<ChatQuestions>,
    pub retry: Option<ChatRetry>,
    pub stats: ChatStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatCommand {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatState {
    pub session_id: String,
    pub entries: Vec<ChatEntry>,
    /// Blocks of the response being streamed; `None` when none is.
    pub streaming: Option<Vec<ChatBlock>>,
    pub live: ChatLive,
    pub commands: Vec<ChatCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ChatDelta {
    /// A response started: `streaming` becomes `Some(vec![])`.
    Start,
    /// Appends to block `index` (creating a text block when `index == blocks.len()`).
    Text { index: usize, text: String },
    /// Appends to block `index` (creating a thinking block when `index == blocks.len()`).
    Thinking { index: usize, text: String },
    /// Sets tool-call block `index`; `arguments` is `None` until the call is complete.
    ToolCall {
        index: usize,
        id: String,
        name: String,
        arguments: Option<serde_json::Value>,
    },
    /// Replaces running tool `call_id`'s output.
    ToolOutput {
        call_id: String,
        content: Vec<ChatContent>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ChatEvent {
    /// First event of every attach, and again whenever the active branch changes non-linearly.
    Snapshot {
        state: ChatState,
    },
    /// Appended to the active branch. An `Assistant` entry also ends `streaming`.
    Entry {
        entry: ChatEntry,
    },
    Live {
        live: ChatLive,
    },
    Delta {
        delta: ChatDelta,
    },
    Notice {
        message: String,
    },
    /// The stream ended for good; `missing` = the session does not exist.
    Closed {
        message: String,
        missing: bool,
    },
    /// Emitted by transports (never the Host) while a remote stream reconnects; a `Snapshot` follows.
    Reconnecting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChatForkInfo {
    pub session_id: String,
    pub editable_text: Option<String>,
}
