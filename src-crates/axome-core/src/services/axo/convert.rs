use std::collections::{HashMap, HashSet};

use axo::store::{Entry, RunOutcome, SeqEntry, SubmissionKind};
use axo_agent::{AgentMessage, AgentSnapshot};
use axo_ai::{AssistantBlock, Message, StopReason, ThinkingLevel, Usage, UserBlock};
use axome_protocol::chat::*;

use crate::services::history::normalize_title;

pub(super) fn thinking_to_wire(level: ThinkingLevel) -> ChatThinking {
    match level {
        ThinkingLevel::Off => ChatThinking::Off,
        ThinkingLevel::Minimal => ChatThinking::Minimal,
        ThinkingLevel::Low => ChatThinking::Low,
        ThinkingLevel::Medium => ChatThinking::Medium,
        ThinkingLevel::High => ChatThinking::High,
        ThinkingLevel::XHigh => ChatThinking::Xhigh,
        ThinkingLevel::Max => ChatThinking::Max,
    }
}

pub(super) fn thinking_from_wire(level: ChatThinking) -> ThinkingLevel {
    match level {
        ChatThinking::Off => ThinkingLevel::Off,
        ChatThinking::Minimal => ThinkingLevel::Minimal,
        ChatThinking::Low => ThinkingLevel::Low,
        ChatThinking::Medium => ThinkingLevel::Medium,
        ChatThinking::High => ThinkingLevel::High,
        ChatThinking::Xhigh => ThinkingLevel::XHigh,
        ChatThinking::Max => ThinkingLevel::Max,
    }
}

pub(super) fn usage(value: &Usage) -> ChatUsage {
    ChatUsage {
        input: value.input,
        output: value.output,
        cache_read: value.cache_read,
        cache_write: value.cache_write,
        total_tokens: value.total_tokens,
        cost: value.cost.total,
    }
}

pub(super) fn content(blocks: &[UserBlock]) -> Vec<ChatContent> {
    blocks
        .iter()
        .map(|block| match block {
            UserBlock::Text { text } => ChatContent::Text { text: text.clone() },
            UserBlock::Image { mime_type, data } => ChatContent::Image {
                mime_type: mime_type.clone(),
                data: data.to_string(),
            },
        })
        .collect()
}

pub(super) fn text(blocks: &[UserBlock]) -> String {
    let mut result = String::new();
    let mut first = true;
    for block in blocks {
        if let UserBlock::Text { text } = block {
            if !first {
                result.push('\n');
            }
            result.push_str(text);
            first = false;
        }
    }
    result
}

pub(super) fn block(value: &AssistantBlock) -> ChatBlock {
    match value {
        AssistantBlock::Text { text, .. } => ChatBlock::Text { text: text.clone() },
        AssistantBlock::Thinking {
            thinking, redacted, ..
        } => ChatBlock::Thinking {
            text: if *redacted {
                String::new()
            } else {
                thinking.clone()
            },
            redacted: *redacted,
        },
        AssistantBlock::ToolCall(call) => ChatBlock::ToolCall {
            id: call.id.clone(),
            name: call.name.clone(),
            arguments: serde_json::Value::Object(call.arguments.clone()),
        },
    }
}

pub(super) fn is_user(value: &Entry) -> bool {
    matches!(
        value,
        Entry::Message {
            message: AgentMessage::Llm(Message::User(_)),
            ..
        }
    )
}

fn anchor<'a>(value: &'a Entry, by_id: &HashMap<&'a str, &'a Entry>) -> Option<&'a str> {
    let mut parent = value.parent_id();
    // A corrupt cyclic parent chain must not hang a transcript conversion.
    for _ in 0..=by_id.len() {
        let id = parent?;
        let Some(ancestor) = by_id.get(id) else {
            return Some(id);
        };
        match ancestor {
            // Navigation leaves preserve their parent's position, not a new edit anchor.
            Entry::Leaf { .. }
            | Entry::Submission { .. }
            | Entry::RunStart { .. }
            | Entry::RunResumed { .. }
            | Entry::Message {
                message: AgentMessage::Llm(Message::System(_)),
                ..
            } => {
                parent = ancestor.parent_id();
            }
            _ => return Some(id),
        }
    }
    None
}

pub(super) fn entry(entries: &[SeqEntry], at: usize) -> Option<ChatEntry> {
    let value = &entries.get(at)?.entry;
    let base = value.base()?;
    let body = match value {
        Entry::Message {
            message: AgentMessage::Llm(Message::User(user)),
            ..
        } => {
            let by_id: HashMap<_, _> = entries
                .iter()
                .filter_map(|seq| Some((seq.entry.id()?, &seq.entry)))
                .collect();
            let user_anchor = anchor(value, &by_id);
            let mut siblings: Vec<_> = entries
                .iter()
                .filter(|seq| is_user(&seq.entry) && anchor(&seq.entry, &by_id) == user_anchor)
                .collect();
            siblings.sort_by_key(|seq| seq.seq);
            ChatEntryBody::User {
                text: text(&user.content),
                anchor: user_anchor.map(str::to_owned),
                siblings: siblings
                    .into_iter()
                    .filter_map(|seq| seq.entry.id().map(str::to_owned))
                    .collect(),
            }
        }
        Entry::Message {
            message: AgentMessage::Llm(Message::Assistant(assistant)),
            ..
        } => ChatEntryBody::Assistant {
            blocks: assistant.content.iter().map(block).collect(),
            model: format!("{}/{}", assistant.provider, assistant.model),
            usage: usage(&assistant.usage),
            stop_reason: match assistant.stop_reason {
                StopReason::Stop => ChatStopReason::Stop,
                StopReason::Length => ChatStopReason::Length,
                StopReason::ToolUse => ChatStopReason::ToolUse,
                StopReason::Error => ChatStopReason::Error,
                StopReason::Aborted => ChatStopReason::Aborted,
            },
            error: assistant.error_message.clone(),
            duration_ms: assistant.duration_ms,
        },
        Entry::Message {
            message: AgentMessage::Llm(Message::ToolResult(result)),
            ..
        } => ChatEntryBody::ToolResult {
            call_id: result.tool_call_id.clone(),
            tool_name: result.tool_name.clone(),
            content: content(&result.content),
            details: result.details.clone(),
            is_error: result.is_error,
            duration_ms: result.duration_ms,
        },
        Entry::Message {
            message: AgentMessage::Custom(custom),
            ..
        } if custom.display => ChatEntryBody::Notice {
            text: text(&custom.content),
        },
        Entry::CustomMessage {
            content,
            display: true,
            ..
        } => ChatEntryBody::Notice {
            text: text(content),
        },
        Entry::ModelChange {
            provider, model_id, ..
        } => ChatEntryBody::ModelChange {
            model: format!("{provider}/{model_id}"),
        },
        Entry::ThinkingLevelChange { thinking_level, .. } => ChatEntryBody::ThinkingChange {
            thinking: thinking_to_wire(*thinking_level),
        },
        Entry::Compaction {
            summary,
            tokens_before,
            ..
        } => ChatEntryBody::Compaction {
            summary: summary.clone(),
            tokens_before: *tokens_before,
        },
        Entry::BranchSummary { summary, .. } => ChatEntryBody::BranchSummary {
            summary: summary.clone(),
        },
        Entry::RunEnd {
            outcome, reason, ..
        } if reason.as_deref() != Some("fork") => ChatEntryBody::RunEnd {
            outcome: match outcome {
                RunOutcome::Done => ChatRunOutcome::Done,
                RunOutcome::Aborted => ChatRunOutcome::Aborted,
                RunOutcome::Failed => ChatRunOutcome::Failed,
                RunOutcome::Interrupted => ChatRunOutcome::Interrupted,
            },
            reason: reason.clone(),
        },
        _ => return None,
    };
    Some(ChatEntry {
        id: base.id.to_string(),
        timestamp: base.timestamp.clone(),
        body,
    })
}

pub(super) fn branch(entries: &[SeqEntry]) -> Vec<usize> {
    axo::store::project(entries, None).path().to_vec()
}

pub(super) fn title(entries: &[SeqEntry]) -> Option<String> {
    title_of(entries.iter().map(|seq| &seq.entry))
}

pub(super) fn title_of<'a>(entries: impl IntoIterator<Item = &'a Entry>) -> Option<String> {
    let mut name = None;
    let mut prompt = None;
    for entry in entries {
        match entry {
            Entry::SessionInfo {
                name: Some(value), ..
            } => name = Some(value.as_str()),
            Entry::Submission {
                kind: SubmissionKind::Prompt,
                content,
                ..
            } if prompt.is_none() => {
                prompt = Some(text(content));
            }
            _ => {}
        }
    }
    name.or(prompt.as_deref()).and_then(normalize_title)
}

pub(super) fn selection(entries: &[SeqEntry], path: &[usize]) -> (Option<String>, ThinkingLevel) {
    let mut model = None;
    let mut thinking = ThinkingLevel::Off;
    for &at in path {
        match &entries[at].entry {
            Entry::ModelChange {
                provider, model_id, ..
            } => model = Some(format!("{provider}/{model_id}")),
            Entry::ThinkingLevelChange { thinking_level, .. } => thinking = *thinking_level,
            _ => {}
        }
    }
    (model, thinking)
}

pub(super) fn stats(entries: &[SeqEntry], path: &[usize], context_window: u64) -> ChatStats {
    let context_tokens = path
        .iter()
        .rev()
        .find_map(|&at| match &entries[at].entry {
            Entry::Message {
                message: AgentMessage::Llm(Message::Assistant(value)),
                ..
            } => Some(value.usage.total_tokens),
            _ => None,
        })
        .unwrap_or(0);
    let mut result = ChatStats {
        context_tokens,
        context_window,
        input: 0,
        output: 0,
        cache_read: 0,
        cache_write: 0,
        cost: 0.0,
    };
    for seq in entries {
        let usage = match &seq.entry {
            Entry::Message {
                message: AgentMessage::Llm(Message::Assistant(value)),
                ..
            } => &value.usage,
            Entry::Usage { usage, .. } => usage,
            Entry::Compaction {
                usage: Some(usage), ..
            }
            | Entry::BranchSummary {
                usage: Some(usage), ..
            } => usage,
            _ => continue,
        };
        result.input += usage.input;
        result.output += usage.output;
        result.cache_read += usage.cache_read;
        result.cache_write += usage.cache_write;
        result.cost += usage.cost.total;
    }
    result
}

pub(super) fn queue_at_attach(
    entries: &[SeqEntry],
    snapshot: &AgentSnapshot,
) -> Vec<(String, ChatQueued)> {
    let matched: HashSet<_> = entries
        .iter()
        .filter_map(|seq| match &seq.entry {
            Entry::Message {
                submission_id: Some(id),
                ..
            } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let mut steering = Vec::new();
    let mut follow_up = Vec::new();
    for seq in entries {
        if let Entry::Submission {
            submission_id,
            kind,
            content,
            ..
        } = &seq.entry
        {
            if matched.contains(submission_id.as_str()) {
                continue;
            }
            let (queue, kind) = match kind {
                SubmissionKind::Steer => (&mut steering, ChatQueueKind::Steer),
                SubmissionKind::FollowUp => (&mut follow_up, ChatQueueKind::FollowUp),
                SubmissionKind::Prompt => continue,
            };
            queue.push((seq.seq, submission_id, kind, content));
        }
    }
    steering.sort_by_key(|item| item.0);
    follow_up.sort_by_key(|item| item.0);
    let steering_skip = steering
        .len()
        .saturating_sub(snapshot.queued_steering.len());
    let follow_up_skip = follow_up
        .len()
        .saturating_sub(snapshot.queued_follow_up.len());
    let mut queued: Vec<_> = steering
        .into_iter()
        .skip(steering_skip)
        .chain(follow_up.into_iter().skip(follow_up_skip))
        .collect();
    queued.sort_by_key(|item| item.0);
    queued
        .into_iter()
        .map(|(_, id, kind, content)| {
            (
                id.clone(),
                ChatQueued {
                    kind,
                    text: text(content),
                },
            )
        })
        .collect()
}

pub(super) fn question(value: &axo::Question) -> ChatQuestion {
    ChatQuestion {
        id: value.id.clone(),
        question: value.question.clone(),
        header: value.header.clone(),
        options: value
            .options
            .iter()
            .map(|option| ChatQuestionOption {
                label: option.label.clone(),
                description: option.description.clone(),
                preview: option.preview.clone(),
            })
            .collect(),
        multi: value.multi,
        recommended: value.recommended,
    }
}

pub(super) fn answer(value: ChatAnswer) -> axo::Answer {
    axo::Answer {
        id: value.id,
        selected: value.selected,
        custom: value.custom,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axo::store::{EntryBase, EntryId};
    use axo_ai::{AssistantMessage, SystemMessage, UserMessage};

    fn base(id: &str, parent: Option<&str>) -> EntryBase {
        EntryBase {
            id: EntryId(id.into()),
            parent_id: parent.map(|id| EntryId(id.into())),
            timestamp: "2026-10-23T00:00:00.000Z".into(),
        }
    }

    fn log(entries: Vec<Entry>) -> Vec<SeqEntry> {
        entries
            .into_iter()
            .enumerate()
            .map(|(seq, entry)| SeqEntry {
                seq: seq as u64,
                entry,
            })
            .collect()
    }

    fn submission(id: &str, parent: Option<&str>, kind: SubmissionKind) -> Entry {
        Entry::Submission {
            base: base(id, parent),
            submission_id: id.into(),
            kind,
            content: vec![UserBlock::Text { text: id.into() }],
            request_id: None,
        }
    }

    fn user(id: &str, parent: Option<&str>, submission: &str) -> Entry {
        Entry::Message {
            base: base(id, parent),
            submission_id: Some(submission.into()),
            message: AgentMessage::Llm(Message::User(UserMessage {
                content: vec![UserBlock::Text { text: id.into() }],
                timestamp: 0,
            })),
        }
    }

    fn system(id: &str, parent: Option<&str>) -> Entry {
        Entry::Message {
            base: base(id, parent),
            submission_id: None,
            message: AgentMessage::Llm(Message::System(SystemMessage {
                content: vec![],
                sections: None,
                tools_added: vec![],
                tools_removed: vec![],
                timestamp: 0,
            })),
        }
    }

    #[test]
    fn edits_share_anchor_and_siblings_past_run_bookkeeping() {
        let entries = log(vec![
            Entry::RunEnd {
                base: base("a", None),
                run_id: "old".into(),
                outcome: RunOutcome::Done,
                reason: None,
            },
            submission("s1", Some("a"), SubmissionKind::Prompt),
            Entry::RunStart {
                base: base("r1", Some("s1")),
                run_id: "one".into(),
            },
            system("system", Some("r1")),
            user("u1", Some("system"), "s1"),
            Entry::Leaf {
                base: base("leaf", Some("a")),
            },
            submission("s2", Some("leaf"), SubmissionKind::Prompt),
            Entry::RunStart {
                base: base("r2", Some("s2")),
                run_id: "two".into(),
            },
            Entry::RunResumed {
                base: base("resume", Some("r2")),
                run_id: "two".into(),
                attempt: 1,
            },
            user("u2", Some("resume"), "s2"),
        ]);
        for at in [4, 9] {
            let ChatEntryBody::User {
                anchor, siblings, ..
            } = entry(&entries, at).unwrap().body
            else {
                panic!("expected user");
            };
            assert_eq!(anchor.as_deref(), Some("a"));
            assert_eq!(siblings, ["u1", "u2"]);
        }
    }

    #[test]
    fn path_hides_system_bookkeeping_custom_and_fork_end() {
        let assistant = AssistantMessage {
            content: vec![AssistantBlock::Text {
                text: "reply".into(),
                text_signature: None,
            }],
            api: axo_ai::ApiKind::OpenAiCompletions,
            provider: "test".into(),
            model: "m".into(),
            response_model: None,
            response_id: None,
            provider_thinking_level: None,
            thinking_level: None,
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            error_message: None,
            failure: None,
            raw_stop_reason: None,
            end_turn: None,
            timestamp: 0,
            duration_ms: None,
        };
        let entries = log(vec![
            system("system", None),
            submission("s", Some("system"), SubmissionKind::Prompt),
            Entry::RunStart {
                base: base("r", Some("s")),
                run_id: "run".into(),
            },
            Entry::CustomMessage {
                base: base("hidden", Some("r")),
                custom_type: "hidden".into(),
                content: vec![],
                details: None,
                display: false,
            },
            Entry::ModelChange {
                base: base("model", Some("hidden")),
                provider: "test".into(),
                model_id: "m".into(),
            },
            Entry::Message {
                base: base("reply", Some("model")),
                message: AgentMessage::Llm(Message::Assistant(assistant)),
                submission_id: None,
            },
            Entry::RunEnd {
                base: base("end", Some("reply")),
                run_id: "run".into(),
                outcome: RunOutcome::Done,
                reason: Some("fork".into()),
            },
        ]);
        let converted: Vec<_> = branch(&entries)
            .into_iter()
            .filter_map(|at| entry(&entries, at))
            .collect();
        assert_eq!(converted.len(), 2);
        assert!(matches!(
            converted[0].body,
            ChatEntryBody::ModelChange { .. }
        ));
        assert!(matches!(converted[1].body, ChatEntryBody::Assistant { .. }));
    }

    #[test]
    fn queue_attach_keeps_unmatched_tail_in_sequence_order() {
        let entries = log(vec![
            submission("old-steer", None, SubmissionKind::Steer),
            submission("consumed", None, SubmissionKind::Steer),
            submission("old-follow", None, SubmissionKind::FollowUp),
            submission("follow", None, SubmissionKind::FollowUp),
            submission("steer", None, SubmissionKind::Steer),
            user("delivered", None, "consumed"),
            submission("prompt", None, SubmissionKind::Prompt),
        ]);
        let queued_message = AgentMessage::Llm(Message::User(UserMessage {
            content: vec![],
            timestamp: 0,
        }));
        let snapshot = AgentSnapshot {
            queued_steering: vec![queued_message.clone()],
            queued_follow_up: vec![queued_message],
            ..AgentSnapshot::default()
        };
        assert_eq!(
            queue_at_attach(&entries, &snapshot),
            vec![
                (
                    "follow".into(),
                    ChatQueued {
                        kind: ChatQueueKind::FollowUp,
                        text: "follow".into()
                    }
                ),
                (
                    "steer".into(),
                    ChatQueued {
                        kind: ChatQueueKind::Steer,
                        text: "steer".into()
                    }
                ),
            ]
        );
        assert!(queue_at_attach(&entries, &AgentSnapshot::default()).is_empty());
    }
}
