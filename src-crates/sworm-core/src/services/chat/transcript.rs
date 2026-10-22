//! Normalize lenient ACP updates into the chat transcript and incremental events.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;
use sworm_protocol::chat::*;

#[derive(Debug, Deserialize)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
pub(crate) enum UpdateWire {
    UserMessageChunk(ChunkWire),
    AgentMessageChunk(ChunkWire),
    AgentThoughtChunk(ChunkWire),
    ToolCall(ToolCallWire),
    ToolCallUpdate(ToolCallWire),
    Plan {
        entries: Vec<PlanEntryWire>,
    },
    #[serde(rename_all = "camelCase")]
    AvailableCommandsUpdate {
        available_commands: Vec<CommandWire>,
    },
    #[serde(rename_all = "camelCase")]
    ConfigOptionUpdate {
        config_options: Vec<Value>,
    },
    UsageUpdate {
        size: u64,
        used: u64,
        #[serde(default)]
        cost: Option<CostWire>,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChunkWire {
    pub(crate) content: ContentWire,
    pub(crate) message_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ContentWire {
    Text {
        text: String,
    },
    #[serde(rename_all = "camelCase")]
    Image {
        data: String,
        mime_type: String,
    },
    ResourceLink {
        uri: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolCallWire {
    pub(crate) tool_call_id: String,
    pub(crate) name: Option<String>,
    pub(crate) raw_input: Option<Value>,
    pub(crate) title: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) content: Option<Vec<ToolContentWire>>,
    pub(crate) locations: Option<Vec<LocationWire>>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ToolContentWire {
    Content {
        content: ContentWire,
    },
    #[serde(rename_all = "camelCase")]
    Diff {
        path: String,
        old_text: Option<String>,
        new_text: String,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocationWire {
    pub(crate) path: String,
    pub(crate) line: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanEntryWire {
    pub(crate) content: String,
    pub(crate) priority: String,
    pub(crate) status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandWire {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) input: Option<CommandInputWire>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandInputWire {
    pub(crate) hint: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CostWire {
    pub(crate) amount: f64,
    pub(crate) currency: String,
}

pub(crate) fn parse_update(value: Value) -> Option<UpdateWire> {
    match serde_json::from_value(value) {
        Ok(update) => Some(update),
        Err(error) => {
            tracing::warn!(%error, "skipping malformed ACP session update");
            None
        }
    }
}

pub(crate) fn parse_config_options(values: &[Value]) -> Vec<ChatConfigOption> {
    values
        .iter()
        .filter_map(|value| {
            let kind = match value.get("type")?.as_str()? {
                "select" => {
                    let current_value = value.get("currentValue")?.as_str()?.to_owned();
                    let mut choices = Vec::new();
                    flatten_choices(value.get("options")?.as_array()?, None, &mut choices);
                    ChatConfigKind::Select {
                        current_value,
                        choices,
                    }
                }
                "boolean" => ChatConfigKind::Boolean {
                    current_value: value.get("currentValue")?.as_bool()?,
                },
                _ => return None,
            };
            Some(ChatConfigOption {
                id: value.get("id")?.as_str()?.to_owned(),
                name: value.get("name")?.as_str()?.to_owned(),
                description: value
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                category: value
                    .get("category")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                kind,
            })
        })
        .collect()
}

fn flatten_choices(values: &[Value], group: Option<&str>, choices: &mut Vec<ChatConfigChoice>) {
    for value in values {
        let Some(name) = value.get("name").and_then(Value::as_str) else {
            continue;
        };
        if let Some(options) = value.get("options").and_then(Value::as_array) {
            flatten_choices(options, Some(name), choices);
        } else if let Some(choice) = value.get("value").and_then(Value::as_str) {
            choices.push(ChatConfigChoice {
                value: choice.to_owned(),
                name: name.to_owned(),
                description: value
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                group: group.map(str::to_owned),
            });
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    User,
    Message,
    Thought,
}

impl Role {
    fn prefix(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Message => "msg",
            Self::Thought => "thought",
        }
    }
}

#[derive(Default)]
pub(crate) struct Transcript {
    entries: Vec<ChatEntry>,
    index: HashMap<String, usize>,
    turn: u64,
    open: Option<(Role, String)>,
}

impl Transcript {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn entries(&self) -> &[ChatEntry] {
        &self.entries
    }

    pub(crate) fn apply(&mut self, update: UpdateWire) -> Vec<ChatEvent> {
        if !matches!(
            &update,
            UpdateWire::UserMessageChunk(_)
                | UpdateWire::AgentMessageChunk(_)
                | UpdateWire::AgentThoughtChunk(_)
        ) {
            self.open = None;
        }
        match update {
            UpdateWire::UserMessageChunk(chunk) => self.user_chunk(chunk),
            UpdateWire::AgentMessageChunk(chunk) => self.text_chunk(Role::Message, chunk),
            UpdateWire::AgentThoughtChunk(chunk) => self.text_chunk(Role::Thought, chunk),
            UpdateWire::ToolCall(tool) | UpdateWire::ToolCallUpdate(tool) => {
                vec![self.tool(tool)]
            }
            UpdateWire::Plan { entries } => {
                let id = format!("plan:{}", self.turn);
                let items = entries
                    .into_iter()
                    .map(|entry| ChatPlanItem {
                        content: entry.content,
                        priority: entry.priority,
                        status: entry.status,
                    })
                    .collect();
                vec![self.store(id.clone(), ChatEntry::Plan { id, items })]
            }
            UpdateWire::AvailableCommandsUpdate { available_commands } => {
                vec![ChatEvent::Commands {
                    commands: available_commands
                        .into_iter()
                        .map(|command| ChatCommand {
                            name: command.name,
                            description: command.description,
                            hint: command.input.and_then(|input| input.hint),
                        })
                        .collect(),
                }]
            }
            UpdateWire::ConfigOptionUpdate { config_options } => {
                vec![ChatEvent::ConfigOptions {
                    options: parse_config_options(&config_options),
                }]
            }
            UpdateWire::UsageUpdate { size, used, cost } => vec![ChatEvent::Usage {
                usage: ChatUsage {
                    used,
                    size,
                    cost: cost.map(|cost| ChatCost {
                        amount: cost.amount,
                        currency: cost.currency,
                    }),
                },
            }],
            UpdateWire::Other => Vec::new(),
        }
    }

    pub(crate) fn push_user(&mut self, content: Vec<ChatContent>) -> ChatEvent {
        let id = format!("user:{}", self.entries.len());
        self.turn += 1;
        self.open = None;
        self.store(id.clone(), ChatEntry::User { id, content })
    }

    pub(crate) fn end_turn(
        &mut self,
        stop_reason: ChatStopReason,
        usage: Option<ChatTokenUsage>,
        error: Option<String>,
    ) -> ChatEvent {
        self.open = None;
        let id = format!("end:{}", self.turn);
        self.store(
            id.clone(),
            ChatEntry::TurnEnd {
                id,
                stop_reason,
                usage,
                error,
            },
        )
    }

    fn chunk_id(&self, role: Role, message_id: Option<String>) -> String {
        if let Some(message_id) = message_id {
            return format!("{}:{message_id}", role.prefix());
        }
        if let Some((open_role, id)) = &self.open {
            if *open_role == role
                && self.index.get(id).copied() == self.entries.len().checked_sub(1)
            {
                return id.clone();
            }
        }
        format!("{}:{}", role.prefix(), self.entries.len())
    }

    fn text_chunk(&mut self, role: Role, chunk: ChunkWire) -> Vec<ChatEvent> {
        let ContentWire::Text { text } = chunk.content else {
            return Vec::new();
        };
        let id = self.chunk_id(role, chunk.message_id);
        self.open = Some((role, id.clone()));
        if let Some(&index) = self.index.get(&id) {
            match &mut self.entries[index] {
                ChatEntry::Message { text: existing, .. }
                | ChatEntry::Thought { text: existing, .. } => {
                    existing.push_str(&text);
                }
                _ => unreachable!("chunk ids are namespaced by role"),
            }
            vec![ChatEvent::TextDelta { id, text }]
        } else {
            let entry = match role {
                Role::Message => ChatEntry::Message {
                    id: id.clone(),
                    text,
                },
                Role::Thought => ChatEntry::Thought {
                    id: id.clone(),
                    text,
                },
                Role::User => unreachable!("user chunks carry content rather than text deltas"),
            };
            vec![self.store(id, entry)]
        }
    }

    fn user_chunk(&mut self, chunk: ChunkWire) -> Vec<ChatEvent> {
        let content = match chunk.content {
            ContentWire::Text { text } => ChatContent::Text { text },
            ContentWire::Image { data, mime_type } => ChatContent::Image { mime_type, data },
            _ => return Vec::new(),
        };
        let id = self.chunk_id(Role::User, chunk.message_id);
        self.open = Some((Role::User, id.clone()));
        if let Some(&index) = self.index.get(&id) {
            let ChatEntry::User {
                content: existing, ..
            } = &mut self.entries[index]
            else {
                unreachable!("user chunk ids are namespaced by role");
            };
            match (existing.last_mut(), content) {
                (Some(ChatContent::Text { text: previous }), ChatContent::Text { text }) => {
                    previous.push_str(&text);
                }
                (_, content) => existing.push(content),
            }
            vec![ChatEvent::Entry {
                entry: self.entries[index].clone(),
            }]
        } else {
            self.turn += 1;
            vec![self.store(
                id.clone(),
                ChatEntry::User {
                    id,
                    content: vec![content],
                },
            )]
        }
    }

    fn tool(&mut self, tool: ToolCallWire) -> ChatEvent {
        let id = format!("tool:{}", tool.tool_call_id);
        let index = *self.index.entry(id.clone()).or_insert_with(|| {
            let index = self.entries.len();
            self.entries.push(ChatEntry::Tool {
                id,
                name: None,
                input: None,
                title: tool.tool_call_id,
                tool_kind: "other".to_owned(),
                status: ChatToolStatus::Pending,
                content: Vec::new(),
                locations: Vec::new(),
            });
            index
        });
        let ChatEntry::Tool {
            name,
            input,
            title,
            tool_kind,
            status,
            content,
            locations,
            ..
        } = &mut self.entries[index]
        else {
            unreachable!("tool ids are namespaced by role");
        };
        if let Some(value) = tool.name {
            *name = Some(value);
        }
        if let Some(value) = tool.raw_input {
            *input = Some(value);
        }
        if let Some(value) = tool.title {
            *title = value;
        }
        if let Some(value) = tool.kind {
            *tool_kind = value;
        }
        if let Some(value) = tool.status {
            match value.as_str() {
                "pending" => *status = ChatToolStatus::Pending,
                "in_progress" => *status = ChatToolStatus::InProgress,
                "completed" => *status = ChatToolStatus::Completed,
                "failed" => *status = ChatToolStatus::Failed,
                _ => {}
            }
        }
        if let Some(value) = tool.content {
            *content = value
                .into_iter()
                .filter_map(|item| match item {
                    ToolContentWire::Content {
                        content: ContentWire::Text { text },
                    } => Some(ChatToolContent::Text { text }),
                    ToolContentWire::Content {
                        content: ContentWire::ResourceLink { uri, .. },
                    } => Some(ChatToolContent::Text { text: uri }),
                    ToolContentWire::Diff {
                        path,
                        old_text,
                        new_text,
                    } => Some(ChatToolContent::Diff {
                        path,
                        old_text,
                        new_text,
                    }),
                    _ => None,
                })
                .collect();
        }
        if let Some(value) = tool.locations {
            *locations = value
                .into_iter()
                .map(|location| ChatLocation {
                    path: location.path,
                    line: location.line,
                })
                .collect();
        }
        ChatEvent::Entry {
            entry: self.entries[index].clone(),
        }
    }

    fn store(&mut self, id: String, entry: ChatEntry) -> ChatEvent {
        if let Some(&index) = self.index.get(&id) {
            self.entries[index] = entry.clone();
        } else {
            self.index.insert(id, self.entries.len());
            self.entries.push(entry.clone());
        }
        ChatEvent::Entry { entry }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn apply(transcript: &mut Transcript, value: Value) -> Vec<ChatEvent> {
        transcript.apply(parse_update(value).expect("valid ACP fixture"))
    }

    #[test]
    fn message_chunks_append_and_tool_calls_break_implicit_continuation() {
        let mut transcript = Transcript::new();
        let first = ChatEntry::Message {
            id: "msg:answer".into(),
            text: "Hello".into(),
        };
        assert_eq!(
            apply(
                &mut transcript,
                json!({
                    "sessionUpdate": "agent_message_chunk", "messageId": "answer",
                    "content": {"type": "text", "text": "Hello"}
                })
            ),
            vec![ChatEvent::Entry { entry: first }]
        );
        assert_eq!(
            apply(
                &mut transcript,
                json!({
                    "sessionUpdate": "agent_message_chunk", "messageId": "answer",
                    "content": {"type": "text", "text": " world"}
                })
            ),
            vec![ChatEvent::TextDelta {
                id: "msg:answer".into(),
                text: " world".into()
            }]
        );
        assert_eq!(
            transcript.entries(),
            &[ChatEntry::Message {
                id: "msg:answer".into(),
                text: "Hello world".into()
            }]
        );
        apply(
            &mut transcript,
            json!({
                "sessionUpdate": "tool_call", "toolCallId": "read-1", "title": "Read",
                "kind": "read", "status": "pending", "rawInput": {"path": "README.md"}
            }),
        );
        let next = ChatEntry::Message {
            id: "msg:2".into(),
            text: "After tool".into(),
        };
        assert_eq!(
            apply(
                &mut transcript,
                json!({
                    "sessionUpdate": "agent_message_chunk",
                    "content": {"type": "text", "text": "After tool"}
                })
            ),
            vec![ChatEvent::Entry {
                entry: next.clone()
            }]
        );
        assert_eq!(transcript.entries().len(), 3);
        assert_eq!(transcript.entries().last(), Some(&next));
    }

    #[test]
    fn tool_updates_replace_content_and_preserve_omitted_fields() {
        let mut transcript = Transcript::new();
        apply(
            &mut transcript,
            json!({
                "sessionUpdate": "tool_call", "toolCallId": "read-1", "title": "Read README",
                "kind": "read", "status": "pending", "rawInput": {"path": "README.md"},
                "name": "read",
                "content": [{"type": "content", "content": {"type": "text", "text": "Reading"}}],
                "locations": [{"path": "README.md"}]
            }),
        );
        assert!(matches!(
            &transcript.entries()[0],
            ChatEntry::Tool { name: Some(name), input: Some(input), .. }
                if name == "read" && input == &json!({"path": "README.md"})
        ));
        let expected = ChatEntry::Tool {
            id: "tool:read-1".into(),
            name: Some("read_file".into()),
            input: Some(json!({"path": "CHANGELOG.md"})),
            title: "Read README".into(),
            tool_kind: "read".into(),
            status: ChatToolStatus::Completed,
            content: vec![ChatToolContent::Text {
                text: "Finished".into(),
            }],
            locations: vec![ChatLocation {
                path: "README.md".into(),
                line: None,
            }],
        };
        assert_eq!(
            apply(
                &mut transcript,
                json!({
                    "sessionUpdate": "tool_call_update", "toolCallId": "read-1", "status": "completed",
                    "name": "read_file", "rawInput": {"path": "CHANGELOG.md"},
                    "content": [{"type": "content", "content": {"type": "text", "text": "Finished"}}]
                })
            ),
            vec![ChatEvent::Entry {
                entry: expected.clone()
            }]
        );
        assert_eq!(transcript.entries(), &[expected.clone()]);
        assert_eq!(
            apply(
                &mut transcript,
                json!({"sessionUpdate": "tool_call_update", "toolCallId": "read-1"})
            ),
            vec![ChatEvent::Entry { entry: expected }]
        );
    }

    #[test]
    fn turn_end_stores_reported_usage_and_never_comes_from_replay() {
        let mut transcript = Transcript::new();
        apply(
            &mut transcript,
            json!({
                "sessionUpdate": "user_message_chunk",
                "content": {"type": "text", "text": "Replayed"}
            }),
        );
        assert!(!transcript
            .entries()
            .iter()
            .any(|entry| matches!(entry, ChatEntry::TurnEnd { .. })));
        for (turn, cache_read, cache_write) in [(2, None, None), (3, Some(30), Some(40))] {
            transcript.push_user(vec![ChatContent::Text {
                text: "Prompt".into(),
            }]);
            let expected = ChatEntry::TurnEnd {
                id: format!("end:{turn}"),
                stop_reason: ChatStopReason::EndTurn,
                usage: Some(ChatTokenUsage {
                    input: 100,
                    output: 20,
                    cache_read,
                    cache_write,
                }),
                error: None,
            };
            assert_eq!(
                transcript.end_turn(
                    ChatStopReason::EndTurn,
                    Some(ChatTokenUsage {
                        input: 100,
                        output: 20,
                        cache_read,
                        cache_write
                    }),
                    None,
                ),
                ChatEvent::Entry {
                    entry: expected.clone()
                }
            );
            assert_eq!(transcript.entries().last(), Some(&expected));
        }
    }

    #[test]
    fn config_selects_flatten_groups_with_innermost_names() {
        let options = parse_config_options(&[
            json!({
                "id": "thinking", "name": "Thinking", "category": "thought_level",
                "type": "select", "currentValue": "medium",
                "options": [{"value": "medium", "name": "Medium", "description": "Balanced"}]
            }),
            json!({
                "id": "model", "name": "Model", "category": "model", "type": "select",
                "currentValue": "nested", "options": [
                    {"value": "flat", "name": "Flat"},
                    {"group": "provider", "name": "Provider", "options": [
                        {"value": "outer", "name": "Outer"},
                        {"group": "family", "name": "Family", "options": [
                            {"value": "nested", "name": "Nested"}
                        ]}
                    ]}
                ]
            }),
        ]);
        assert_eq!(
            options,
            vec![
                ChatConfigOption {
                    id: "thinking".into(),
                    name: "Thinking".into(),
                    description: None,
                    category: Some("thought_level".into()),
                    kind: ChatConfigKind::Select {
                        current_value: "medium".into(),
                        choices: vec![ChatConfigChoice {
                            value: "medium".into(),
                            name: "Medium".into(),
                            description: Some("Balanced".into()),
                            group: None,
                        }],
                    },
                },
                ChatConfigOption {
                    id: "model".into(),
                    name: "Model".into(),
                    description: None,
                    category: Some("model".into()),
                    kind: ChatConfigKind::Select {
                        current_value: "nested".into(),
                        choices: vec![
                            ChatConfigChoice {
                                value: "flat".into(),
                                name: "Flat".into(),
                                description: None,
                                group: None
                            },
                            ChatConfigChoice {
                                value: "outer".into(),
                                name: "Outer".into(),
                                description: None,
                                group: Some("Provider".into())
                            },
                            ChatConfigChoice {
                                value: "nested".into(),
                                name: "Nested".into(),
                                description: None,
                                group: Some("Family".into())
                            },
                        ],
                    },
                },
            ]
        );
    }

    #[test]
    fn unknown_session_updates_produce_no_events() {
        let mut transcript = Transcript::new();
        assert!(apply(&mut transcript, json!({
            "sessionUpdate": "session_info_update", "title": "An omp session", "updatedAt": "2026-10-21T00:00:00Z"
        })).is_empty());
        assert!(transcript.entries().is_empty());
    }
}
