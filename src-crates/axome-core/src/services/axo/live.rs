use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, OnceLock, Weak},
};

use axo::store::{Entry, EntryId, SeqEntry, SubmissionKind};
use axo::{LiveSnapshot, MaintenanceKind, PromptRequest, SessionEvent};
use axo_agent::{AgentEvent, AgentMessage};
use axo_ai::{AssistantBlock, AssistantEvent, Message};
use parking_lot::Mutex;
use axome_protocol::chat::*;
use tokio::sync::{broadcast, oneshot};

use super::{axo_error, convert, AxoService};
use crate::{errors::ApiError, events::EventSink};

pub(crate) struct LiveSession {
    pub id: String,
    pub folder: PathBuf,
    pub session: Arc<axo::Session>,
    service: Weak<AxoService>,
    state: Mutex<RelayState>,
    relay: OnceLock<tokio::task::AbortHandle>,
}

struct RelayState {
    entries: Vec<SeqEntry>,
    path: Vec<usize>,
    streaming: Option<Vec<ChatBlock>>,
    live: ChatLive,
    queue_ids: Vec<String>,
    commands: Vec<ChatCommand>,
    subscribers: Vec<(String, EventSink<ChatEvent>)>,
    waiters: HashMap<String, oneshot::Sender<()>>,
    submitted: bool,
}

fn maintenance(kind: MaintenanceKind) -> ChatMaintenance {
    match kind {
        MaintenanceKind::Compact => ChatMaintenance::Compact,
        MaintenanceKind::Shake => ChatMaintenance::Shake,
        MaintenanceKind::BranchSummary => ChatMaintenance::BranchSummary,
    }
}

impl RelayState {
    fn new(
        entries: Vec<SeqEntry>,
        snapshot: LiveSnapshot,
        commands: Vec<ChatCommand>,
        service: &Weak<AxoService>,
    ) -> Self {
        let path = convert::branch(&entries);
        let (model, thinking) = convert::selection(&entries, &path);
        let context_window = service
            .upgrade()
            .map_or(0, |s| s.context_window(model.as_deref()));
        let queued = convert::queue_at_attach(&entries, &snapshot.agent);
        let (queue_ids, queue) = queued.into_iter().unzip();
        let live = ChatLive {
            title: convert::title(&entries),
            running: snapshot.agent.running,
            maintenance: snapshot.maintenance.map(maintenance),
            model,
            thinking: convert::thinking_to_wire(thinking),
            tools: snapshot
                .agent
                .pending_tool_calls
                .iter()
                .map(|call| ChatToolRun {
                    call_id: call.id.clone(),
                    name: call.name.clone(),
                    arguments: serde_json::Value::Object(call.arguments.clone()),
                    output: Vec::new(),
                })
                .collect(),
            queue,
            questions: snapshot
                .pending_questions
                .into_iter()
                .map(|(call_id, questions)| ChatQuestions {
                    call_id,
                    questions: questions.iter().map(convert::question).collect(),
                })
                .collect(),
            retry: None,
            stats: convert::stats(&entries, &path, context_window),
        };
        let submitted = entries
            .iter()
            .any(|seq| matches!(seq.entry, Entry::Submission { .. }));
        Self {
            entries,
            path,
            streaming: snapshot
                .agent
                .streaming
                .as_ref()
                .map(|m| m.content.iter().map(convert::block).collect()),
            live,
            queue_ids,
            commands,
            subscribers: Vec::new(),
            waiters: HashMap::new(),
            submitted,
        }
    }

    fn snapshot(&self, id: &str) -> ChatState {
        ChatState {
            session_id: id.to_owned(),
            entries: self
                .path
                .iter()
                .filter_map(|at| convert::entry(&self.entries, *at))
                .collect(),
            streaming: self.streaming.clone(),
            live: self.live.clone(),
            commands: self.commands.clone(),
        }
    }

    fn broadcast(&mut self, event: ChatEvent) {
        self.subscribers
            .retain(|(_, sink)| sink(event.clone()).is_ok());
    }

    fn broadcast_snapshot(&mut self, id: &str) {
        let state = self.snapshot(id);
        self.broadcast(ChatEvent::Snapshot { state });
    }

    fn broadcast_live(&mut self) {
        self.broadcast(ChatEvent::Live {
            live: self.live.clone(),
        });
    }

    fn set_block(&mut self, index: usize, block: ChatBlock) {
        let blocks = self.streaming.get_or_insert_with(Vec::new);
        if index == blocks.len() {
            blocks.push(block);
        } else if let Some(slot) = blocks.get_mut(index) {
            *slot = block;
        }
    }

    fn streaming_event(&mut self, event: AssistantEvent) {
        match event {
            AssistantEvent::Start { .. } => {
                self.streaming = Some(Vec::new());
                let retry = self.live.retry.take().is_some();
                self.broadcast(ChatEvent::Delta {
                    delta: ChatDelta::Start,
                });
                if retry {
                    self.broadcast_live();
                }
            }
            AssistantEvent::TextStart { content_index } => self.set_block(
                content_index,
                ChatBlock::Text {
                    text: String::new(),
                },
            ),
            AssistantEvent::ThinkingStart { content_index } => self.set_block(
                content_index,
                ChatBlock::Thinking {
                    text: String::new(),
                    redacted: false,
                },
            ),
            AssistantEvent::TextDelta {
                content_index,
                delta,
            } => {
                let blocks = self.streaming.get_or_insert_with(Vec::new);
                if content_index == blocks.len() {
                    blocks.push(ChatBlock::Text {
                        text: String::new(),
                    });
                }
                if let Some(ChatBlock::Text { text }) = blocks.get_mut(content_index) {
                    text.push_str(&delta);
                }
                self.broadcast(ChatEvent::Delta {
                    delta: ChatDelta::Text {
                        index: content_index,
                        text: delta,
                    },
                });
            }
            AssistantEvent::ThinkingDelta {
                content_index,
                delta,
            } => {
                let blocks = self.streaming.get_or_insert_with(Vec::new);
                if content_index == blocks.len() {
                    blocks.push(ChatBlock::Thinking {
                        text: String::new(),
                        redacted: false,
                    });
                }
                if let Some(ChatBlock::Thinking { text, .. }) = blocks.get_mut(content_index) {
                    text.push_str(&delta);
                }
                self.broadcast(ChatEvent::Delta {
                    delta: ChatDelta::Thinking {
                        index: content_index,
                        text: delta,
                    },
                });
            }
            AssistantEvent::ToolCallStart {
                content_index,
                id,
                name,
            } => {
                self.set_block(
                    content_index,
                    ChatBlock::ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: serde_json::Value::Null,
                    },
                );
                self.broadcast(ChatEvent::Delta {
                    delta: ChatDelta::ToolCall {
                        index: content_index,
                        id,
                        name,
                        arguments: None,
                    },
                });
            }
            AssistantEvent::ToolCallEnd {
                content_index,
                block: AssistantBlock::ToolCall(call),
            } => {
                self.set_block(
                    content_index,
                    convert::block(&AssistantBlock::ToolCall(call.clone())),
                );
                self.broadcast(ChatEvent::Delta {
                    delta: ChatDelta::ToolCall {
                        index: content_index,
                        id: call.id,
                        name: call.name,
                        arguments: Some(serde_json::Value::Object(call.arguments)),
                    },
                });
            }
            _ => {}
        }
    }

    // Returns refresh/unload flags; registry work must happen after releasing this lock.
    fn apply(&mut self, event: SessionEvent, id: &str, service: &Weak<AxoService>) -> (bool, bool) {
        let mut refresh = false;
        let mut unload = false;
        let mut changed = false;
        match event {
            SessionEvent::Entry(seq) => {
                let previous = self.live.clone();
                let linear = seq.entry.parent_id()
                    == self.path.last().and_then(|at| self.entries[*at].entry.id())
                    && !matches!(
                        seq.entry,
                        Entry::Leaf { .. } | Entry::ContextEdit { .. } | Entry::Compaction { .. }
                    );
                let at = self.entries.len();
                self.entries.push(seq);
                if linear {
                    self.path.push(at);
                } else {
                    self.path = convert::branch(&self.entries);
                }
                let entry = &self.entries[at].entry;
                match entry {
                    Entry::Submission {
                        submission_id,
                        kind,
                        content,
                        request_id,
                        ..
                    } => {
                        if let Some(request_id) = request_id {
                            if let Some(waiter) = self.waiters.remove(request_id) {
                                let _ = waiter.send(());
                            }
                        }
                        let kind = match kind {
                            SubmissionKind::Prompt => None,
                            SubmissionKind::Steer => Some(ChatQueueKind::Steer),
                            SubmissionKind::FollowUp => Some(ChatQueueKind::FollowUp),
                        };
                        if let Some(kind) = kind {
                            self.queue_ids.push(submission_id.clone());
                            self.live.queue.push(ChatQueued {
                                kind,
                                text: convert::text(content),
                            });
                        }
                        if !self.submitted {
                            self.submitted = true;
                            self.live.title = convert::title(&self.entries);
                            refresh = true;
                        }
                    }
                    Entry::Message {
                        message,
                        submission_id,
                        ..
                    } => {
                        if let Some(at) = submission_id
                            .as_ref()
                            .and_then(|id| self.queue_ids.iter().position(|queued| queued == id))
                        {
                            self.queue_ids.remove(at);
                            self.live.queue.remove(at);
                        }
                        match message {
                            AgentMessage::Llm(Message::Assistant(_)) => {
                                self.streaming = None;
                                self.live.stats = convert::stats(
                                    &self.entries,
                                    &self.path,
                                    self.live.stats.context_window,
                                );
                            }
                            AgentMessage::Llm(Message::ToolResult(result)) => self
                                .live
                                .tools
                                .retain(|run| run.call_id != result.tool_call_id),
                            _ => {}
                        }
                    }
                    Entry::ModelChange { .. } | Entry::ThinkingLevelChange { .. }
                        if self.path.contains(&at) =>
                    {
                        let (model, thinking) = convert::selection(&self.entries, &self.path);
                        self.live.stats.context_window = service
                            .upgrade()
                            .map_or(0, |s| s.context_window(model.as_deref()));
                        self.live.model = model;
                        self.live.thinking = convert::thinking_to_wire(thinking);
                    }
                    Entry::RunStart { .. } | Entry::RunResumed { .. } => self.live.running = true,
                    Entry::RunEnd { .. } => {
                        self.live.running = false;
                        self.live.tools.clear();
                        self.live.retry = None;
                        self.streaming = None;
                        refresh = true;
                        unload = true;
                    }
                    Entry::SessionInfo { .. } => {
                        self.live.title = convert::title(&self.entries);
                        refresh = true;
                    }
                    _ => {}
                }
                if !linear {
                    let (model, thinking) = convert::selection(&self.entries, &self.path);
                    self.live.model = model;
                    self.live.thinking = convert::thinking_to_wire(thinking);
                    let window = service
                        .upgrade()
                        .map_or(0, |s| s.context_window(self.live.model.as_deref()));
                    self.live.stats = convert::stats(&self.entries, &self.path, window);
                    self.broadcast_snapshot(id);
                } else if let Some(entry) = convert::entry(&self.entries, at) {
                    self.broadcast(ChatEvent::Entry { entry });
                }
                changed = self.live != previous;
            }
            SessionEvent::Agent(AgentEvent::AgentStart) => {
                changed = !self.live.running;
                self.live.running = true;
            }
            SessionEvent::Agent(AgentEvent::ToolExecutionStart {
                call_id,
                name,
                args,
            }) => {
                if !self.live.tools.iter().any(|run| run.call_id == call_id) {
                    self.live.tools.push(ChatToolRun {
                        call_id,
                        name,
                        arguments: serde_json::Value::Object(args),
                        output: Vec::new(),
                    });
                    changed = true;
                }
            }
            SessionEvent::Agent(AgentEvent::ToolExecutionUpdate { call_id, update }) => {
                let content = convert::content(&update.content);
                if let Some(run) = self
                    .live
                    .tools
                    .iter_mut()
                    .find(|run| run.call_id == call_id)
                {
                    run.output = content.clone();
                }
                self.broadcast(ChatEvent::Delta {
                    delta: ChatDelta::ToolOutput { call_id, content },
                });
            }
            SessionEvent::Agent(AgentEvent::ToolExecutionEnd { call_id, .. }) => {
                let before = self.live.tools.len();
                self.live.tools.retain(|run| run.call_id != call_id);
                changed = before != self.live.tools.len();
            }
            SessionEvent::Agent(AgentEvent::MessageUpdate { event }) => self.streaming_event(event),
            SessionEvent::QuestionPending { call_id, questions } => {
                if !self.live.questions.iter().any(|q| q.call_id == call_id) {
                    self.live.questions.push(ChatQuestions {
                        call_id,
                        questions: questions.iter().map(convert::question).collect(),
                    });
                    changed = true;
                }
            }
            SessionEvent::QuestionResolved { call_id } => {
                let before = self.live.questions.len();
                self.live.questions.retain(|q| q.call_id != call_id);
                changed = before != self.live.questions.len();
                unload = true;
            }
            SessionEvent::MaintenanceStart { kind } => {
                let next = Some(maintenance(kind));
                changed = self.live.maintenance != next;
                self.live.maintenance = next;
            }
            SessionEvent::MaintenanceEnd { error, aborted, .. } => {
                changed = self.live.maintenance.take().is_some();
                if let Some(message) = error.filter(|_| !aborted) {
                    self.broadcast(ChatEvent::Notice { message });
                }
                unload = true;
            }
            SessionEvent::Retry { attempt, delay_ms } => {
                self.live.retry = Some(ChatRetry { attempt, delay_ms });
                changed = true;
            }
            _ => {}
        }
        if changed {
            self.broadcast_live();
        }
        (refresh, unload)
    }
}

impl LiveSession {
    pub(super) async fn start(
        service: Weak<AxoService>,
        folder: PathBuf,
        session: axo::Session,
    ) -> Arc<Self> {
        let id = session.id().to_owned();
        let attach = session.attach(None).await;
        let mut commands = vec![ChatCommand {
            name: "compact".into(),
            description: "Summarize older history to free context".into(),
        }];
        commands.extend(session.skills().iter().map(|skill| ChatCommand {
            name: skill.name.clone(),
            description: skill.description.clone(),
        }));
        let live = Arc::new(Self {
            id,
            folder,
            session: Arc::new(session),
            service: service.clone(),
            state: Mutex::new(RelayState::new(
                attach.missed,
                attach.live,
                commands,
                &service,
            )),
            relay: OnceLock::new(),
        });
        let weak = Arc::downgrade(&live);
        let session = live.session.clone();
        let relay = tokio::spawn(async move {
            let mut events = attach.events;
            loop {
                match events.recv().await {
                    Ok(event) => {
                        let Some(live) = weak.upgrade() else {
                            break;
                        };
                        let (refresh, unload) =
                            live.state.lock().apply(event, &live.id, &live.service);
                        if let Some(service) = live.service.upgrade() {
                            if refresh {
                                service.schedule_refresh(&live.folder);
                            }
                            if unload {
                                service.maybe_unload(&live.id);
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        let Some(live) = weak.upgrade() else {
                            break;
                        };
                        let last_seq = live.state.lock().entries.last().map(|seq| seq.seq);
                        let attach = session.attach(last_seq).await;
                        {
                            let mut state = live.state.lock();
                            state.entries.extend(attach.missed);
                            let entries = std::mem::take(&mut state.entries);
                            let mut rebuilt = RelayState::new(
                                entries,
                                attach.live,
                                std::mem::take(&mut state.commands),
                                &live.service,
                            );
                            rebuilt.subscribers = std::mem::take(&mut state.subscribers);
                            rebuilt.waiters = std::mem::take(&mut state.waiters);
                            // The missed durable entries may include the submission a caller is awaiting.
                            for seq in &rebuilt.entries {
                                if let Entry::Submission {
                                    request_id: Some(request_id),
                                    ..
                                } = &seq.entry
                                {
                                    if let Some(waiter) = rebuilt.waiters.remove(request_id) {
                                        let _ = waiter.send(());
                                    }
                                }
                            }
                            *state = rebuilt;
                            state.broadcast_snapshot(&live.id);
                        }
                        events = attach.events;
                        if let Some(service) = live.service.upgrade() {
                            service.schedule_refresh(&live.folder);
                            service.maybe_unload(&live.id);
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        let _ = live.relay.set(relay.abort_handle());
        live
    }

    pub(super) fn abort_relay(&self) {
        if let Some(relay) = self.relay.get() {
            relay.abort();
        }
    }

    pub(super) fn subscribe(
        &self,
        attach_id: String,
        sink: EventSink<ChatEvent>,
    ) -> Result<(), ApiError> {
        let mut state = self.state.lock();
        sink(ChatEvent::Snapshot {
            state: state.snapshot(&self.id),
        })
        .map_err(ApiError::Internal)?;
        state.subscribers.retain(|(id, _)| id != &attach_id);
        state.subscribers.push((attach_id, sink));
        Ok(())
    }

    pub(super) fn unsubscribe(&self, attach_id: &str) {
        self.state
            .lock()
            .subscribers
            .retain(|(id, _)| id != attach_id);
    }
    pub(super) fn idle_and_unattached(&self) -> bool {
        let state = self.state.lock();
        state.subscribers.is_empty()
            && !state.live.running
            && state.live.maintenance.is_none()
            && state.live.questions.is_empty()
    }
    pub(super) fn busy(&self) -> bool {
        let state = self.state.lock();
        state.live.running || state.live.maintenance.is_some()
    }
    pub(super) fn broadcast_closed(&self, message: String) {
        self.state.lock().broadcast(ChatEvent::Closed {
            message,
            missing: false,
        });
    }

    pub(super) async fn prompt(
        self: &Arc<Self>,
        request_id: String,
        text: String,
    ) -> Result<(), ApiError> {
        let (submitted, mut submission) = oneshot::channel();
        self.state
            .lock()
            .waiters
            .insert(request_id.clone(), submitted);
        let (mut done, mut settled) = oneshot::channel();
        let live = self.clone();
        let driver_request_id = request_id.clone();
        tokio::spawn(async move {
            let session = live.session.clone();
            let submitted_request_id = driver_request_id.clone();
            let mut driver = tokio::spawn(async move {
                session
                    .prompt_with(PromptRequest {
                        text,
                        request_id: Some(submitted_request_id),
                    })
                    .await
            });
            let result = tokio::select! {
                result = &mut driver => result,
                _ = done.closed() => {
                    live.state.lock().waiters.remove(&driver_request_id);
                    driver.await
                }
            };
            live.state.lock().waiters.remove(&driver_request_id);
            let result = match result {
                Ok(result) => result.map(|_| ()).map_err(|error| {
                    let message = error.to_string();
                    (axo_error(error), Some(message))
                }),
                Err(error) => {
                    let message = error.to_string();
                    if let Some(service) = live.service.upgrade() {
                        service.crashed(&live.id, message.clone());
                    }
                    Err((
                        ApiError::Internal(format!("axo session stopped unexpectedly: {message}")),
                        None,
                    ))
                }
            };
            if let Err(Err((_, Some(message)))) = done.send(result) {
                live.state.lock().broadcast(ChatEvent::Notice { message });
            }
        });
        tokio::select! {
            biased;
            result = &mut settled => {
                self.state.lock().waiters.remove(&request_id);
                result.map_err(|_| ApiError::Internal("axo prompt driver stopped".into()))?.map_err(|(error, _)| error)
            }
            result = &mut submission => {
                if result.is_err() {
                    return settled.await.map_err(|_| ApiError::Internal("axo prompt driver stopped".into()))?.map_err(|(error, _)| error);
                }
                // Close before checking a result already sent, so no later driver error is lost.
                settled.close();
                if let Ok(Err((_, Some(message)))) = settled.try_recv() {
                    self.state.lock().broadcast(ChatEvent::Notice { message });
                }
                Ok(())
            }
        }
    }

    pub(super) fn select_branch_target(&self, entry_id: &str) -> Result<EntryId, ApiError> {
        let state = self.state.lock();
        if !state
            .entries
            .iter()
            .any(|seq| seq.entry.id() == Some(entry_id))
        {
            return Err(ApiError::NotFound(format!("axo entry {entry_id}")));
        }
        let mut descendants = HashSet::from([entry_id.to_owned()]);
        let mut target = None;
        for seq in &state.entries {
            if seq.entry.id().is_some_and(|id| descendants.contains(id))
                || seq
                    .entry
                    .parent_id()
                    .is_some_and(|id| descendants.contains(id))
            {
                if let Some(id) = seq.entry.id() {
                    descendants.insert(id.to_owned());
                    if !matches!(seq.entry, Entry::Leaf { .. })
                        && target.as_ref().is_none_or(|(last, _)| *last < seq.seq)
                    {
                        target = Some((seq.seq, id.to_owned()));
                    }
                }
            }
        }
        target
            .map(|(_, id)| EntryId::from(id))
            .ok_or_else(|| ApiError::NotFound(format!("axo branch {entry_id}")))
    }
}
