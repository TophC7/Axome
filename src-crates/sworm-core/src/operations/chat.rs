use std::{collections::BTreeMap, sync::Arc, time::Duration};

use parking_lot::Mutex;
use serde_json::{json, Value};
use sworm_protocol::chat::*;

use crate::{
    errors::ApiError,
    events::EventSink,
    host::Host,
    services::{
        chat::{acp::AgentKey, request_id, ChatRun, RunState},
        extensions::{missing_requirement, resolve_contribution_command},
        folders::resolve_folder,
        harnesses::{chat_args, resolve_chat_command},
    },
};

fn mcp_entry(
    name: &str,
    command: String,
    args: &[String],
    env: &BTreeMap<String, String>,
) -> Value {
    let env: Vec<_> = env
        .iter()
        .map(|(name, value)| json!({"name":name,"value":value}))
        .collect();
    json!({"name":name,"command":command,"args":args,"env":env})
}

impl Host {
    pub fn chat_start(
        &self,
        run_id: String,
        folder_path: String,
        harness_id: String,
        preset: Option<String>,
        session_id: Option<String>,
        subscriber: Option<EventSink<ChatEvent>>,
        owner_id: Option<String>,
    ) -> Result<ChatStartInfo, ApiError> {
        let _owner = self.runs.owner_activity(owner_id.as_deref())?;
        let folder = resolve_folder(&folder_path)?;
        self.runs.with_start(&run_id, |_| {
            if let Ok(run) = self.chat.get(&run_id) {
                let mut owner = run.owner.lock();
                let mut state = run.state.lock();
                if state.ended || state.stopping {
                    return Err(ApiError::NotFound(format!("Chat run not found: {run_id}")));
                }
                if run.folder != folder || run.harness_id != harness_id || run.preset != preset {
                    return Err(ApiError::InvalidArgument(format!(
                        "run id is already bound to different metadata: {run_id}"
                    )));
                }
                *owner = owner_id;
                drop(owner);
                if let Some(subscriber) = subscriber {
                    run.attach(&mut state, subscriber);
                }
                return Ok(ChatStartInfo {
                    session_id: state.bound.then(|| state.acp_session_id.clone()).flatten(),
                    resumed: true,
                });
            }
            let launch = self.resolve_harness_launch(&folder, &harness_id, preset.as_deref())?;
            let harness = launch.harness();
            let label = harness.definition.label.clone();
            let chat =
                harness.definition.chat.as_ref().ok_or_else(|| {
                    ApiError::InvalidArgument(format!("{label} has no chat agent"))
                })?;
            let path = launch.env.get("PATH").map(String::as_str).unwrap_or("");
            let command = resolve_chat_command(
                harness,
                chat,
                path,
                launch.config.binary_path_override.as_deref(),
            )
            .map_err(ApiError::InvalidArgument)?;
            let args = chat_args(chat, &launch.config.extra_args, &launch.preset.args);
            let mut mcp_servers = Vec::new();
            let mut notices = Vec::new();
            let mut add_server = |name: &str,
                                  result: Result<String, String>,
                                  args: &[String],
                                  env: &BTreeMap<String, String>| {
                match result {
                    Ok(command) => mcp_servers.push(mcp_entry(name, command, args, env)),
                    Err(error) => notices.push(ChatEvent::Notice {
                        level: ChatNoticeLevel::Warning,
                        message: format!("MCP server {name} unavailable: {error}"),
                    }),
                }
            };
            for server in &launch.catalog().mcp_servers {
                let definition = &server.definition;
                if launch.mcp_servers.contains_key(&definition.id) {
                    continue;
                }
                let command = match missing_requirement(&server.requires, path) {
                    Some(error) => Err(error),
                    None => resolve_contribution_command(
                        &definition.command,
                        server.extension_dir.as_deref(),
                        path,
                        None,
                    ),
                };
                add_server(&definition.id, command, &definition.args, &definition.env);
            }
            for (name, server) in &launch.mcp_servers {
                add_server(
                    name,
                    resolve_contribution_command(&server.command, None, path, None),
                    &server.args,
                    &server.env,
                );
            }
            let key = AgentKey {
                command,
                args,
                env: launch.env.into_iter().collect(),
            };
            let run = Arc::new(ChatRun {
                run_id: run_id.clone(),
                folder,
                harness_id,
                preset,
                label,
                key,
                mcp_servers,
                owner: Mutex::new(owner_id),
                agent: Mutex::new(None),
                state: Mutex::new(RunState::new(session_id.clone())),
            });
            let handle = self.chat.runtime();
            {
                let _claim = self.session_claims.lock();
                if let Some(id) = &session_id {
                    self.ensure_session_free(&run.harness_id, id, &run_id)?;
                }
                self.chat
                    .inner
                    .runs
                    .lock()
                    .insert(run_id.clone(), run.clone());
                let mut state = run.state.lock();
                if let Some(subscriber) = subscriber {
                    run.attach(&mut state, subscriber);
                }
            }
            let result = handle.block_on(async {
                let agent = self
                    .chat
                    .inner
                    .pool
                    .acquire(run.key.clone(), &run.label)
                    .await?;
                *run.agent.lock() = Some(agent.clone());
                let load = session_id.filter(|_| agent.capabilities().load_session);
                let resumed = run.open_session(&agent, load, true).await?;
                let mut state = run.state.lock();
                if state.ended || state.stopping {
                    return Err("Chat run stopped".to_owned());
                }
                self.resume_discovery
                    .claim(state.acp_session_id.as_deref().expect("session opened"));
                state.capabilities = ChatCapabilities {
                    images: agent.capabilities().images,
                    fork: agent.capabilities().fork,
                };
                state.turn = ChatTurn::Idle;
                run.emit_snapshot(&mut state);
                for notice in notices {
                    run.emit(&mut state, notice);
                }
                Ok(ChatStartInfo {
                    session_id: state.bound.then(|| state.acp_session_id.clone()).flatten(),
                    resumed,
                })
            });
            if let Err(error) = &result {
                self.chat.inner.finish(&run, Some(error.clone()));
            }
            result.map_err(ApiError::InvalidArgument)
        })
    }

    pub fn chat_prompt(&self, run_id: String, prompt: Vec<ChatContent>) -> Result<(), ApiError> {
        let run = self.chat.get(&run_id)?;
        let current = run.agent.lock();
        let mut state = run.state.lock();
        if state.ended || state.stopping {
            return Err(ApiError::NotFound(format!("Chat run not found: {run_id}")));
        }
        if state.turn != ChatTurn::Idle {
            return Err(ApiError::InvalidArgument(
                "A turn is already running".into(),
            ));
        }
        if !state.capabilities.images
            && prompt
                .iter()
                .any(|item| matches!(item, ChatContent::Image { .. }))
        {
            return Err(ApiError::InvalidArgument(
                "This agent does not accept images".into(),
            ));
        }
        let agent = current
            .clone()
            .ok_or_else(|| ApiError::Internal("Chat agent unavailable".into()))?;
        let session_id = state.acp_session_id.clone().expect("idle chat has session");
        if !state.bound {
            state.bound = true;
            run.emit(
                &mut state,
                ChatEvent::SessionBound {
                    session_id: session_id.clone(),
                },
            );
            self.resume_discovery.claim(&session_id);
        }
        let wire_prompt: Vec<_> = prompt
            .iter()
            .map(|item| match item {
                ChatContent::Text { text } => json!({"type":"text","text":text}),
                ChatContent::Image { mime_type, data } => {
                    json!({"type":"image","mimeType":mime_type,"data":data})
                }
            })
            .collect();
        let entry = state.transcript.push_user(prompt);
        run.emit(&mut state, entry);
        state.turn = ChatTurn::Running;
        run.emit(
            &mut state,
            ChatEvent::TurnState {
                turn: ChatTurn::Running,
            },
        );
        drop(state);
        drop(current);
        self.chat.runtime().spawn(async move {
            let result = agent
                .request(
                    "session/prompt",
                    json!({"sessionId":session_id,"prompt":wire_prompt}),
                    None,
                )
                .await;
            let current = run.agent.lock();
            if !current
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &agent))
            {
                return;
            }
            let mut state = run.state.lock();
            if state.ended || state.stopping || state.turn != ChatTurn::Running {
                return;
            }
            let (stop_reason, usage, error) = match result {
                Ok(result) => {
                    let stop_reason = result
                        .get("stopReason")
                        .cloned()
                        .ok_or_else(|| "Agent returned no stopReason".to_owned())
                        .and_then(|value| {
                            serde_json::from_value::<ChatStopReason>(value)
                                .map_err(|error| format!("Invalid agent stopReason: {error}"))
                        });
                    let usage = result.get("usage").and_then(|usage| {
                        Some(ChatTokenUsage {
                            input: usage.get("inputTokens")?.as_u64()?,
                            output: usage.get("outputTokens")?.as_u64()?,
                            cache_read: usage.get("cachedReadTokens").and_then(Value::as_u64),
                            cache_write: usage.get("cachedWriteTokens").and_then(Value::as_u64),
                        })
                    });
                    match stop_reason {
                        Ok(stop_reason) => (stop_reason, usage, None),
                        Err(error) => (ChatStopReason::Error, usage, Some(error)),
                    }
                }
                Err(error) => (ChatStopReason::Error, None, Some(error.message)),
            };
            let end = state.transcript.end_turn(stop_reason, usage, error);
            run.emit(&mut state, end);
            state.turn = ChatTurn::Idle;
            run.emit(
                &mut state,
                ChatEvent::TurnState {
                    turn: ChatTurn::Idle,
                },
            );
            run.cancel_requests(&mut state);
        });
        Ok(())
    }

    pub fn chat_cancel(&self, run_id: String) -> Result<(), ApiError> {
        let run = self.chat.get(&run_id)?;
        let agent = run.agent.lock();
        let mut state = run.state.lock();
        if state.ended || state.stopping {
            return Err(ApiError::NotFound(format!("Chat run not found: {run_id}")));
        }
        let result = if state.turn == ChatTurn::Running {
            agent
                .as_ref()
                .map(|agent| {
                    agent.notify("session/cancel", json!({"sessionId":state.acp_session_id}))
                })
                .transpose()
                .map_err(|error| ApiError::Internal(error.message))
        } else {
            Ok(None)
        };
        run.cancel_requests(&mut state);
        result.map(|_| ())
    }

    pub fn chat_set_config(
        &self,
        run_id: String,
        config_id: String,
        value: ChatConfigValue,
    ) -> Result<(), ApiError> {
        let run = self.chat.get(&run_id)?;
        let (agent, session_id) = {
            let current = run.agent.lock();
            let state = run.state.lock();
            if state.ended || state.stopping {
                return Err(ApiError::NotFound(format!("Chat run not found: {run_id}")));
            }
            (
                current
                    .clone()
                    .ok_or_else(|| ApiError::Internal("Chat agent unavailable".into()))?,
                state.acp_session_id.clone(),
            )
        };
        let mut params = json!({"sessionId":session_id,"configId":config_id});
        match value {
            ChatConfigValue::Select { value } => params["value"] = json!(value),
            ChatConfigValue::Boolean { value } => {
                params["type"] = json!("boolean");
                params["value"] = json!(value);
            }
        }
        let result = self
            .chat
            .runtime()
            .block_on(agent.request(
                "session/set_config_option",
                params,
                Some(Duration::from_secs(30)),
            ))
            .map_err(|error| ApiError::Internal(error.message))?;
        let current = run.agent.lock();
        let mut state = run.state.lock();
        if !state.ended
            && !state.stopping
            && current
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &agent))
        {
            run.apply_config(&mut state, &result);
            let options = state.config_options.clone();
            run.emit(&mut state, ChatEvent::ConfigOptions { options });
        }
        Ok(())
    }

    pub fn chat_fork(&self, run_id: String) -> Result<ChatForkInfo, ApiError> {
        let run = self.chat.get(&run_id)?;
        let (agent, session_id) = {
            let current = run.agent.lock();
            let state = run.state.lock();
            if state.ended || state.stopping {
                return Err(ApiError::NotFound(format!("Chat run not found: {run_id}")));
            }
            if !state.bound {
                return Err(ApiError::InvalidArgument(
                    "Chat session is not bound".into(),
                ));
            }
            if state.turn != ChatTurn::Idle {
                return Err(ApiError::InvalidArgument(
                    "Chat must be idle to fork".into(),
                ));
            }
            if !state.capabilities.fork {
                return Err(ApiError::InvalidArgument(
                    "This agent does not support session forks".into(),
                ));
            }
            (
                current
                    .clone()
                    .ok_or_else(|| ApiError::Internal("Chat agent unavailable".into()))?,
                state
                    .acp_session_id
                    .clone()
                    .ok_or_else(|| ApiError::Internal("Chat session unavailable".into()))?,
            )
        };
        self.chat.runtime().block_on(async {
            let result = agent
                .request(
                    "session/fork",
                    json!({
                        "sessionId": session_id, "cwd": run.folder, "mcpServers": run.mcp_servers,
                    }),
                    Some(Duration::from_secs(120)),
                )
                .await
                .map_err(|error| ApiError::Internal(error.message))?;
            let session_id = result["sessionId"]
                .as_str()
                .ok_or_else(|| ApiError::Internal("Agent returned no fork sessionId".into()))?
                .to_owned();
            // omp keeps forks registered; load can reuse them and replay history.
            // Close when supported so an unopened fork retains no live resources.
            if agent.capabilities().close_session {
                if let Err(error) = agent
                    .request(
                        "session/close",
                        json!({"sessionId": session_id}),
                        Some(Duration::from_secs(5)),
                    )
                    .await
                {
                    tracing::warn!(%run_id, %session_id, %error, "Failed to close forked ACP session");
                }
            }
            agent.unregister_session(&session_id);
            Ok(ChatForkInfo { session_id })
        })
    }

    pub fn chat_respond(
        &self,
        run_id: String,
        request_id_value: String,
        response: ChatResponse,
    ) -> Result<(), ApiError> {
        let run = self.chat.get(&run_id)?;
        let mut state = run.state.lock();
        if state.ended || state.stopping {
            return Err(ApiError::NotFound(format!("Chat run not found: {run_id}")));
        }
        let index = state
            .requests
            .iter()
            .position(|(request, _, _)| request_id(request) == request_id_value)
            .ok_or_else(|| {
                ApiError::NotFound(format!("Chat request not found: {request_id_value}"))
            })?;
        let (request, id, agent) = &state.requests[index];
        let result = match (request, response) {
            (
                ChatRequest::Permission { .. },
                ChatResponse::Permission {
                    option_id: Some(id),
                },
            ) => json!({"outcome":{"outcome":"selected","optionId":id}}),
            (ChatRequest::Permission { .. }, ChatResponse::Permission { option_id: None }) => {
                json!({"outcome":{"outcome":"cancelled"}})
            }
            (ChatRequest::Elicitation { .. }, ChatResponse::Elicitation { action, content }) => {
                let mut result = json!({"action":action});
                if let Some(content) = content {
                    result["content"] = json!(content);
                }
                result
            }
            _ => {
                return Err(ApiError::InvalidArgument(
                    "Chat response does not match request".into(),
                ))
            }
        };
        agent
            .respond(id.clone(), result)
            .map_err(|error| ApiError::Internal(error.message))?;
        state.requests.remove(index);
        run.emit(
            &mut state,
            ChatEvent::RequestResolved {
                request_id: request_id_value,
            },
        );
        Ok(())
    }

    pub fn chat_stop(&self, run_id: String) -> Result<(), ApiError> {
        self.runs.with_run(&run_id, |_| {
            let Ok(run) = self.chat.get(&run_id) else {
                return Ok(());
            };
            let (agent, session_id) = {
                let mut current = run.agent.lock();
                let mut state = run.state.lock();
                if state.ended || state.stopping {
                    return Ok(());
                }
                // Keep the one-writer claim until close and unregister finish.
                state.stopping = true;
                let agent = current.take();
                run.cancel_requests(&mut state);
                if state.turn == ChatTurn::Running {
                    if let Some(agent) = &agent {
                        let _ = agent
                            .notify("session/cancel", json!({"sessionId":state.acp_session_id}));
                    }
                }
                (agent, state.acp_session_id.clone())
            };
            if let Some(agent) = &agent {
                if let Some(id) = &session_id {
                    if agent.capabilities().close_session {
                        if let Err(error) = self.chat.runtime().block_on(agent.request(
                            "session/close",
                            json!({"sessionId":id}),
                            Some(Duration::from_secs(5)),
                        )) {
                            tracing::warn!(%run_id, %error, "Failed to close ACP session");
                        }
                    }
                    agent.unregister_session(id);
                }
                self.chat.inner.pool.release(agent);
            }
            self.chat.inner.remove(&run);
            let mut state = run.state.lock();
            state.ended = true;
            run.emit(&mut state, ChatEvent::Exit { message: None });
            state.subscriber = None;
            Ok(())
        })
    }

    pub fn chat_attach(
        &self,
        run_id: String,
        subscriber: EventSink<ChatEvent>,
    ) -> Result<u64, ApiError> {
        let run = self.chat.get(&run_id)?;
        let mut state = run.state.lock();
        if state.ended || state.stopping {
            return Err(ApiError::NotFound(format!("Chat run not found: {run_id}")));
        }
        Ok(run.attach(&mut state, subscriber))
    }

    pub fn chat_detach(&self, run_id: &str, token: u64) {
        if let Ok(run) = self.chat.get(run_id) {
            let mut state = run.state.lock();
            if state
                .subscriber
                .as_ref()
                .is_some_and(|(current, _)| *current == token)
            {
                state.subscriber = None;
            }
        }
    }

    pub fn chat_owner(&self, run_id: &str) -> Option<Option<String>> {
        self.chat.owner(run_id)
    }
}
