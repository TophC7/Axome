//! ACP chat sessions and transcript normalization.

pub(crate) mod transcript;

pub(crate) mod acp;
mod pool;

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use serde_json::{json, Value};
use sworm_protocol::chat::*;
use tokio::runtime::Handle;

use crate::{errors::ApiError, events::EventSink};
use acp::{AgentKey, AgentProcess};
use pool::AgentPool;
use transcript::{parse_config_options, parse_update, Transcript};

pub(crate) struct ChatService {
    pub(crate) inner: Arc<ChatInner>,
}

pub(crate) struct ChatInner {
    pub(crate) runs: Mutex<HashMap<String, Arc<ChatRun>>>,
    pub(crate) pool: AgentPool,
    pub(crate) runtime: OnceLock<Handle>,
}

pub(crate) struct ChatRun {
    pub(crate) run_id: String,
    pub(crate) folder: PathBuf,
    pub(crate) harness_id: String,
    pub(crate) preset: Option<String>,
    pub(crate) label: String,
    pub(crate) key: AgentKey,
    pub(crate) mcp_servers: Vec<Value>,
    pub(crate) owner: Mutex<Option<String>>,
    pub(crate) agent: Mutex<Option<Arc<AgentProcess>>>,
    pub(crate) state: Mutex<RunState>,
}

pub(crate) struct RunState {
    pub(crate) acp_session_id: Option<String>,
    pub(crate) bound: bool,
    pub(crate) turn: ChatTurn,
    pub(crate) transcript: Transcript,
    pub(crate) config_options: Vec<ChatConfigOption>,
    pub(crate) commands: Vec<ChatCommand>,
    pub(crate) usage: Option<ChatUsage>,
    pub(crate) requests: Vec<(ChatRequest, Value, Arc<AgentProcess>)>,
    pub(crate) accepts_images: bool,
    pub(crate) subscriber: Option<(u64, EventSink<ChatEvent>)>,
    pub(crate) next_attachment: u64,
    notices: VecDeque<ChatEvent>,
    last_recovery: Option<Instant>,
    pub(crate) stopping: bool,
    pub(crate) ended: bool,
}

impl RunState {
    pub(crate) fn new(session_id: Option<String>) -> Self {
        Self {
            acp_session_id: session_id,
            bound: false,
            turn: ChatTurn::Starting,
            transcript: Transcript::new(),
            config_options: Vec::new(),
            commands: Vec::new(),
            usage: None,
            requests: Vec::new(),
            accepts_images: false,
            subscriber: None,
            next_attachment: 0,
            notices: VecDeque::new(),
            last_recovery: None,
            stopping: false,
            ended: false,
        }
    }
}

impl ChatService {
    pub(crate) fn new() -> Self {
        let inner = Arc::new(ChatInner {
            runs: Mutex::new(HashMap::new()),
            pool: AgentPool::new(),
            runtime: OnceLock::new(),
        });
        inner.pool.bind(Arc::downgrade(&inner));
        Self { inner }
    }

    pub(crate) fn get(&self, run_id: &str) -> Result<Arc<ChatRun>, ApiError> {
        self.inner
            .runs
            .lock()
            .get(run_id)
            .cloned()
            .ok_or_else(|| ApiError::NotFound(format!("Chat run not found: {run_id}")))
    }

    pub(crate) fn runtime(&self) -> &Handle {
        self.inner.runtime.get_or_init(Handle::current)
    }

    pub(crate) fn holder(&self, harness_id: &str, session_id: &str) -> Option<String> {
        self.inner.runs.lock().values().find_map(|run| {
            if run.harness_id != harness_id {
                return None;
            }
            let state = run.state.lock();
            (!state.ended && state.acp_session_id.as_deref() == Some(session_id))
                .then(|| run.run_id.clone())
        })
    }

    pub(crate) fn owner_run_ids(&self, owner: &str) -> Vec<String> {
        self.inner
            .runs
            .lock()
            .values()
            .filter(|run| run.owner.lock().as_deref() == Some(owner))
            .filter(|run| {
                let state = run.state.lock();
                !state.ended && !state.stopping
            })
            .map(|run| run.run_id.clone())
            .collect()
    }

    pub(crate) fn owner(&self, run_id: &str) -> Option<Option<String>> {
        self.inner
            .runs
            .lock()
            .get(run_id)
            .map(|run| run.owner.lock().clone())
    }

    pub(crate) fn run_ids_in_roots(&self, roots: &[PathBuf]) -> Vec<String> {
        self.inner
            .runs
            .lock()
            .values()
            .filter(|run| roots.iter().any(|root| run.folder.starts_with(root)))
            .map(|run| run.run_id.clone())
            .collect()
    }

    pub(crate) fn live_runs(&self) -> Vec<(Option<String>, PathBuf, String, Option<String>)> {
        self.inner
            .runs
            .lock()
            .values()
            .filter(|run| {
                let state = run.state.lock();
                !state.ended && !state.stopping
            })
            .map(|run| {
                (
                    run.owner.lock().clone(),
                    run.folder.clone(),
                    run.harness_id.clone(),
                    run.preset.clone(),
                )
            })
            .collect()
    }

    pub(crate) fn shutdown(&self) {
        self.inner.pool.shutdown();
    }
}

pub(crate) fn request_id(request: &ChatRequest) -> &str {
    match request {
        ChatRequest::Permission { request_id, .. }
        | ChatRequest::Elicitation { request_id, .. } => request_id,
    }
}

pub(crate) fn cancelled(request: &ChatRequest) -> Value {
    match request {
        ChatRequest::Permission { .. } => json!({"outcome":{"outcome":"cancelled"}}),
        ChatRequest::Elicitation { .. } => json!({"action":"cancel"}),
    }
}

impl ChatRun {
    pub(crate) fn emit(&self, state: &mut RunState, event: ChatEvent) {
        // Web subscribers attach after startup; notices are not part of a snapshot.
        if state.subscriber.is_none() && matches!(event, ChatEvent::Notice { .. }) {
            if state.notices.len() == 32 {
                state.notices.pop_front();
            }
            state.notices.push_back(event);
            return;
        }
        if state
            .subscriber
            .as_ref()
            .is_some_and(|(_, sink)| sink(event).is_err())
        {
            state.subscriber = None;
        }
    }

    pub(crate) fn snapshot(state: &RunState) -> ChatState {
        ChatState {
            session_id: state.bound.then(|| state.acp_session_id.clone()).flatten(),
            turn: state.turn.clone(),
            entries: state.transcript.entries().to_vec(),
            config_options: state.config_options.clone(),
            commands: state.commands.clone(),
            usage: state.usage.clone(),
            requests: state
                .requests
                .iter()
                .map(|(request, _, _)| request.clone())
                .collect(),
            accepts_images: state.accepts_images,
        }
    }

    pub(crate) fn emit_snapshot(&self, state: &mut RunState) {
        let snapshot = Self::snapshot(state);
        self.emit(state, ChatEvent::Snapshot { state: snapshot });
    }

    pub(crate) fn attach(&self, state: &mut RunState, subscriber: EventSink<ChatEvent>) -> u64 {
        state.next_attachment += 1;
        let token = state.next_attachment;
        state.subscriber = Some((token, subscriber));
        self.emit_snapshot(state);
        while state.subscriber.is_some() {
            let Some(notice) = state.notices.pop_front() else {
                break;
            };
            self.emit(state, notice);
        }
        token
    }

    pub(crate) fn apply_update(&self, update: Value) {
        let Some(update) = parse_update(update) else {
            return;
        };
        let mut state = self.state.lock();
        if state.ended || state.stopping {
            return;
        }
        for event in state.transcript.apply(update) {
            match &event {
                ChatEvent::ConfigOptions { options } => state.config_options = options.clone(),
                ChatEvent::Commands { commands } => state.commands = commands.clone(),
                ChatEvent::Usage { usage } => state.usage = Some(usage.clone()),
                _ => {}
            }
            self.emit(&mut state, event);
        }
    }

    pub(crate) fn add_request(
        &self,
        agent: &Arc<AgentProcess>,
        jsonrpc_id: Value,
        method: &str,
        params: Value,
    ) {
        let request_id = uuid::Uuid::new_v4().to_string();
        let request = if method == "session/request_permission" {
            let tool = &params["toolCall"];
            ChatRequest::Permission {
                request_id,
                tool_call_id: tool["toolCallId"].as_str().unwrap_or("").into(),
                title: tool["title"]
                    .as_str()
                    .unwrap_or("Permission requested")
                    .into(),
                options: params["options"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|option| {
                        Some(ChatPermissionOption {
                            option_id: option["optionId"].as_str()?.into(),
                            name: option["name"].as_str()?.into(),
                            kind: option["kind"].as_str()?.into(),
                        })
                    })
                    .collect(),
            }
        } else {
            ChatRequest::Elicitation {
                request_id,
                message: params["message"].as_str().unwrap_or("").into(),
                schema: params["requestedSchema"].clone(),
            }
        };
        let mut state = self.state.lock();
        if state.ended || state.stopping {
            let _ = agent.respond(jsonrpc_id, cancelled(&request));
            return;
        }
        state
            .requests
            .push((request.clone(), jsonrpc_id, agent.clone()));
        self.emit(&mut state, ChatEvent::Request { request });
    }

    pub(crate) fn cancel_requests(&self, state: &mut RunState) {
        for (request, id, agent) in std::mem::take(&mut state.requests) {
            let _ = agent.respond(id, cancelled(&request));
            self.emit(
                state,
                ChatEvent::RequestResolved {
                    request_id: request_id(&request).into(),
                },
            );
        }
    }

    pub(crate) fn apply_config(&self, state: &mut RunState, result: &Value) {
        state.config_options = parse_config_options(
            result
                .get("configOptions")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        );
    }

    /// Never register while holding state: ACP drains buffered updates synchronously.
    pub(crate) async fn open_session(
        self: &Arc<Self>,
        agent: &Arc<AgentProcess>,
        load: Option<String>,
        fallback: bool,
    ) -> Result<bool, String> {
        if let Some(id) = load {
            agent.register_session(id.clone(), Arc::downgrade(self));
            let result = agent
                .request(
                    "session/load",
                    json!({
                        "sessionId": id, "cwd": self.folder, "mcpServers": self.mcp_servers,
                    }),
                    Some(Duration::from_secs(120)),
                )
                .await;
            match result {
                Ok(result) => {
                    {
                        let mut state = self.state.lock();
                        if !state.ended && !state.stopping {
                            state.bound = true;
                            self.apply_config(&mut state, &result);
                            return Ok(true);
                        }
                    }
                    self.close_abandoned_session(agent, &id).await;
                    return Err("Chat run stopped".into());
                }
                Err(error) => {
                    agent.unregister_session(&id);
                    if !fallback {
                        return Err(error.message);
                    }
                    let mut state = self.state.lock();
                    if state.ended || state.stopping {
                        return Err("Chat run stopped".into());
                    }
                    self.cancel_requests(&mut state);
                    state.transcript = Transcript::new();
                    state.commands.clear();
                    state.usage = None;
                }
            }
        }
        let result = agent
            .request(
                "session/new",
                json!({
                    "cwd": self.folder, "mcpServers": self.mcp_servers,
                }),
                Some(Duration::from_secs(60)),
            )
            .await
            .map_err(|error| error.message)?;
        let id = result["sessionId"]
            .as_str()
            .ok_or("Agent returned no sessionId")?
            .to_owned();
        let ended = {
            let mut state = self.state.lock();
            if !state.ended && !state.stopping {
                state.acp_session_id = Some(id.clone());
                state.bound = false;
                self.apply_config(&mut state, &result);
            }
            state.ended || state.stopping
        };
        if !ended {
            agent.register_session(id.clone(), Arc::downgrade(self));
        }
        let ended = {
            let state = self.state.lock();
            state.ended || state.stopping
        };
        if ended {
            self.close_abandoned_session(agent, &id).await;
            return Err("Chat run stopped".into());
        }
        Ok(false)
    }

    async fn close_abandoned_session(&self, agent: &Arc<AgentProcess>, id: &str) {
        agent.unregister_session(id);
        {
            let state = self.state.lock();
            // Stop already owns closing this session, including a late load reply.
            if state.stopping && state.acp_session_id.as_deref() == Some(id) {
                return;
            }
        }
        if agent.capabilities().close_session {
            if let Err(error) = agent
                .request(
                    "session/close",
                    json!({"sessionId":id}),
                    Some(Duration::from_secs(5)),
                )
                .await
            {
                tracing::warn!(%error, "Failed to close stopped ACP session");
            }
        }
    }
}

impl ChatInner {
    pub(crate) fn remove(&self, run: &Arc<ChatRun>) {
        let mut runs = self.runs.lock();
        if runs
            .get(&run.run_id)
            .is_some_and(|current| Arc::ptr_eq(current, run))
        {
            runs.remove(&run.run_id);
        }
    }

    pub(crate) fn finish(&self, run: &Arc<ChatRun>, message: Option<String>) {
        let (agent, session_id) = {
            let mut current = run.agent.lock();
            let mut state = run.state.lock();
            // Explicit stop owns cleanup until session/close has completed.
            if state.ended || state.stopping {
                return;
            }
            run.cancel_requests(&mut state);
            state.ended = true;
            run.emit(&mut state, ChatEvent::Exit { message });
            state.subscriber = None;
            (current.take(), state.acp_session_id.clone())
        };
        if let Some(agent) = agent {
            if let Some(id) = session_id {
                agent.unregister_session(&id);
            }
            self.pool.release(&agent);
        }
        self.remove(run);
    }

    pub(crate) fn agent_exited(self: &Arc<Self>, agent: Arc<AgentProcess>) {
        let runs = agent.attached_runs();
        for run in runs {
            let give_up = {
                let current = run.agent.lock();
                if !current
                    .as_ref()
                    .is_some_and(|current| Arc::ptr_eq(current, &agent))
                {
                    continue;
                }
                let mut state = run.state.lock();
                if state.ended || state.stopping {
                    continue;
                }
                let give_up = agent.spawned_at.elapsed() < Duration::from_secs(10)
                    || state
                        .last_recovery
                        .is_some_and(|last| last.elapsed() < Duration::from_secs(30));
                if !give_up {
                    state.last_recovery = Some(Instant::now());
                    state.turn = ChatTurn::Reconnecting;
                    run.emit(
                        &mut state,
                        ChatEvent::TurnState {
                            turn: ChatTurn::Reconnecting,
                        },
                    );
                    run.cancel_requests(&mut state);
                    run.emit(
                        &mut state,
                        ChatEvent::Notice {
                            level: ChatNoticeLevel::Warning,
                            message: "Agent process exited; reconnecting.".into(),
                        },
                    );
                }
                give_up
            };
            if give_up {
                let detail = agent
                    .last_stderr_line()
                    .or_else(|| agent.exit_status())
                    .unwrap_or_else(|| "unknown status".into());
                self.finish(&run, Some(format!("Agent process exited: {detail}")));
                continue;
            }
            let inner = self.clone();
            let old_agent = agent.clone();
            self.runtime
                .get()
                .expect("chat runtime initialized")
                .spawn(async move {
                    if let Err(error) = inner.recover(&run, &old_agent).await {
                        inner.finish(&run, Some(error));
                    }
                });
        }
    }

    async fn recover(
        &self,
        run: &Arc<ChatRun>,
        old_agent: &Arc<AgentProcess>,
    ) -> Result<(), String> {
        let old_id = run.state.lock().acp_session_id.clone();
        if let Some(id) = old_id {
            old_agent.unregister_session(&id);
        }
        let agent = self.pool.acquire(run.key.clone(), &run.label).await?;
        let load = {
            let mut current = run.agent.lock();
            let mut state = run.state.lock();
            if state.ended || state.stopping {
                drop(state);
                drop(current);
                self.pool.release(&agent);
                return Ok(());
            }
            *current = Some(agent.clone());
            let load = (state.bound && agent.capabilities().load_session)
                .then(|| state.acp_session_id.clone())
                .flatten();
            state.transcript = Transcript::new();
            state.config_options.clear();
            state.commands.clear();
            state.usage = None;
            if state.bound && load.is_none() {
                run.emit(
                    &mut state,
                    ChatEvent::Notice {
                        level: ChatNoticeLevel::Warning,
                        message: "The agent cannot reload sessions; started a new conversation."
                            .into(),
                    },
                );
            }
            if load.is_none() {
                state.bound = false;
            }
            run.emit_snapshot(&mut state);
            load
        };
        // Once installed in run.agent, finish (or explicit stop) owns release.
        run.open_session(&agent, load, false).await?;
        let mut state = run.state.lock();
        if !state.ended && !state.stopping {
            state.accepts_images = agent.capabilities().images;
            state.turn = ChatTurn::Idle;
            run.emit_snapshot(&mut state);
        }
        Ok(())
    }
}
