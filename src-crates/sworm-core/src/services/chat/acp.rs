//! ACP stdio transport. `ChatRun` owns update/request handling; the pool installs
//! an exit hook and hands attached runs to `ChatInner::agent_exited`.

use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    fmt,
    process::Stdio,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, OnceLock, Weak,
    },
    time::{Duration, Instant},
};

use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::{mpsc, oneshot},
};

use super::ChatRun;

const MAX_LINE: usize = 64 * 1024 * 1024;
const STDERR_TAIL: usize = 4096;
const EXIT_POLL: Duration = Duration::from_millis(50);
type Pending = oneshot::Sender<Result<Value, RpcError>>;
type ExitHook = Box<dyn Fn(Arc<AgentProcess>) + Send + Sync>;

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub(crate) struct AgentKey {
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug)]
pub(crate) struct AgentCapabilities {
    pub load_session: bool,
    pub close_session: bool,
    pub images: bool,
    pub fork: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct RpcError {
    pub message: String,
}

impl RpcError {
    fn exited() -> Self {
        Self {
            message: "agent exited".into(),
        }
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}

pub(crate) struct AgentProcess {
    pub key: AgentKey,
    pub pid: u32,
    pub spawned_at: Instant,
    capabilities: OnceLock<AgentCapabilities>,
    writer: mpsc::UnboundedSender<String>,
    pending: Mutex<HashMap<u64, Pending>>,
    next_id: AtomicU64,
    sessions: Mutex<HashMap<String, Weak<ChatRun>>>,
    unrouted: Mutex<HashMap<String, Vec<Value>>>,
    stderr_tail: Mutex<VecDeque<u8>>,
    child: Mutex<Option<Child>>,
    stopping: AtomicBool,
    exited: AtomicBool,
    exit_status: Mutex<Option<String>>,
    exit_hook: OnceLock<ExitHook>,
    exit_notified: AtomicBool,
}

impl AgentProcess {
    pub(crate) fn capabilities(&self) -> &AgentCapabilities {
        self.capabilities.get().expect("agent handshake completed")
    }

    pub(crate) fn is_alive(&self) -> bool {
        !self.exited.load(Ordering::Acquire) && !self.stopping.load(Ordering::Acquire)
    }

    pub(crate) fn last_stderr_line(&self) -> Option<String> {
        let bytes: Vec<_> = self.stderr_tail.lock().iter().copied().collect();
        String::from_utf8_lossy(&bytes)
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(str::to_owned)
    }

    pub(crate) fn exit_status(&self) -> Option<String> {
        self.exit_status.lock().clone()
    }

    pub(crate) fn attached_runs(&self) -> Vec<Arc<ChatRun>> {
        self.sessions
            .lock()
            .values()
            .filter_map(Weak::upgrade)
            .collect()
    }

    pub(crate) fn sessions_is_empty(&self) -> bool {
        self.sessions.lock().is_empty()
    }

    pub(crate) fn register_session(&self, id: String, run: Weak<ChatRun>) {
        // Serialize the buffered replay with live delivery so chunks cannot overtake it.
        let mut sessions = self.sessions.lock();
        sessions.insert(id.clone(), run.clone());
        let buffered = self.unrouted.lock().remove(&id).unwrap_or_default();
        if let Some(run) = run.upgrade() {
            for update in buffered {
                run.apply_update(update);
            }
        }
    }

    pub(crate) fn unregister_session(&self, id: &str) -> usize {
        let mut sessions = self.sessions.lock();
        sessions.remove(id);
        self.unrouted.lock().remove(id);
        sessions.len()
    }

    pub(crate) async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Option<Duration>,
    ) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock();
            if !self.is_alive() {
                return Err(RpcError::exited());
            }
            pending.insert(id, tx);
        }
        // Also remove requests when their awaiting task is cancelled.
        let _pending = PendingRequest { agent: self, id };
        self.send(json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params}))?;
        match timeout {
            Some(timeout) => tokio::time::timeout(timeout, rx)
                .await
                .map_err(|_| RpcError {
                    message: format!("{method} timed out"),
                })?
                .unwrap_or_else(|_| Err(RpcError::exited())),
            None => rx.await.unwrap_or_else(|_| Err(RpcError::exited())),
        }
    }

    pub(crate) fn notify(&self, method: &str, params: Value) -> Result<(), RpcError> {
        self.send(json!({"jsonrpc":"2.0", "method":method, "params":params}))
    }

    pub(crate) fn respond(&self, id: Value, result: Value) -> Result<(), RpcError> {
        self.send(json!({"jsonrpc":"2.0", "id":id, "result":result}))
    }

    fn send(&self, message: Value) -> Result<(), RpcError> {
        if !self.is_alive() {
            return Err(RpcError::exited());
        }
        self.writer
            .send(message.to_string())
            .map_err(|_| RpcError::exited())
    }

    fn route(self: &Arc<Self>, message: Value) {
        let id = message.get("id");
        let method = message.get("method").and_then(Value::as_str);
        if let Some(method) = method {
            let params = message.get("params").cloned().unwrap_or(Value::Null);
            if let Some(id) = id {
                self.incoming_request(id.clone(), method, params);
            } else if method == "session/update" {
                let (Some(id), Some(update)) = (
                    params.get("sessionId").and_then(Value::as_str),
                    params.get("update"),
                ) else {
                    tracing::warn!("malformed ACP session/update");
                    return;
                };
                let sessions = self.sessions.lock();
                if let Some(run) = sessions.get(id).and_then(Weak::upgrade) {
                    run.apply_update(update.clone());
                } else {
                    let mut unrouted = self.unrouted.lock();
                    let updates = unrouted.entry(id.to_owned()).or_default();
                    if updates.len() < 512 {
                        updates.push(update.clone());
                    }
                }
            }
        } else if let Some(id) = id.and_then(Value::as_u64) {
            let result = if let Some(error) = message.get("error") {
                Err(RpcError {
                    message: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("agent error")
                        .into(),
                })
            } else if let Some(result) = message.get("result") {
                Ok(result.clone())
            } else {
                tracing::warn!("malformed ACP response");
                return;
            };
            if let Some(pending) = self.pending.lock().remove(&id) {
                let _ = pending.send(result);
            }
        } else {
            tracing::warn!("malformed ACP message");
        }
    }

    fn incoming_request(self: &Arc<Self>, id: Value, method: &str, params: Value) {
        match method {
            "elicitation/create" if params.get("mode").and_then(Value::as_str) != Some("form") => {
                let _ = self.respond(id, json!({"action":"decline"}));
            }
            "session/request_permission" | "elicitation/create" => {
                let run = params
                    .get("sessionId")
                    .and_then(Value::as_str)
                    .and_then(|id| self.sessions.lock().get(id).and_then(Weak::upgrade));
                if let Some(run) = run {
                    run.add_request(self, id, method, params);
                } else {
                    let result = if method == "session/request_permission" {
                        json!({"outcome":{"outcome":"cancelled"}})
                    } else {
                        json!({"action":"cancel"})
                    };
                    let _ = self.respond(id, result);
                }
            }
            _ => {
                let _ = self.send(json!({
                    "jsonrpc":"2.0", "id":id,
                    "error":{"code":-32601,"message":"Method not found"}
                }));
            }
        }
    }

    pub(crate) fn set_exit_hook(self: &Arc<Self>, hook: ExitHook) {
        let _ = self.exit_hook.set(hook);
        self.notify_exit();
    }

    fn notify_exit(self: &Arc<Self>) {
        if self.exited.load(Ordering::Acquire) && !self.stopping.load(Ordering::Acquire) {
            if let Some(hook) = self.exit_hook.get() {
                if !self.exit_notified.swap(true, Ordering::AcqRel) {
                    hook(self.clone());
                }
            }
        }
    }

    fn mark_exited(self: &Arc<Self>) {
        if !self.exited.swap(true, Ordering::AcqRel) {
            for (_, pending) in self.pending.lock().drain() {
                let _ = pending.send(Err(RpcError::exited()));
            }
        }
        self.notify_exit();
    }

    fn poll_exit(&self) -> bool {
        let mut child = self.child.lock();
        let Some(process) = child.as_mut() else {
            return true;
        };
        match process.try_wait() {
            Ok(Some(status)) => {
                *self.exit_status.lock() = Some(status.to_string());
                child.take();
                true
            }
            Ok(None) => false,
            Err(error) => {
                tracing::warn!(%error, pid = self.pid, "could not inspect ACP process");
                false
            }
        }
    }

    fn signal(&self, signal: i32) {
        // The child is the leader of its own process group, including its MCP children.
        unsafe {
            libc::kill(-(self.pid as i32), signal);
        }
    }

    fn force_kill(&self) {
        if let Some(child) = self.child.lock().as_mut() {
            self.signal(libc::SIGKILL);
            let _ = child.start_kill();
        }
    }

    pub(crate) async fn stop(&self, grace: Duration) {
        self.stopping.store(true, Ordering::Release);
        {
            let child = self.child.lock();
            if child.is_none() {
                return;
            }
            self.signal(libc::SIGTERM);
        }
        let deadline = tokio::time::Instant::now() + grace;
        loop {
            if self.poll_exit() {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                self.force_kill();
                return;
            }
            tokio::time::sleep_until((tokio::time::Instant::now() + EXIT_POLL).min(deadline)).await;
        }
    }

    /// Shutdown must also work on runtime workers and after the runtime has stopped.
    pub(crate) fn kill(&self) {
        self.stopping.store(true, Ordering::Release);
        self.force_kill();
        for (_, pending) in self.pending.lock().drain() {
            let _ = pending.send(Err(RpcError::exited()));
        }
    }
}

impl Drop for AgentProcess {
    fn drop(&mut self) {
        // A cancelled spawn must not leave the agent's MCP process group behind.
        if self.child.get_mut().is_some() {
            self.signal(libc::SIGKILL);
        }
    }
}

struct PendingRequest<'a> {
    agent: &'a AgentProcess,
    id: u64,
}

impl Drop for PendingRequest<'_> {
    fn drop(&mut self) {
        self.agent.pending.lock().remove(&self.id);
    }
}

pub(crate) async fn spawn(key: AgentKey, label: &str) -> Result<Arc<AgentProcess>, String> {
    let mut child = Command::new(&key.command)
        .args(&key.args)
        .env_clear()
        .envs(&key.env)
        .current_dir(key.env.get("HOME").map(String::as_str).unwrap_or("/"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("{label} chat agent failed to start: {error}"))?;
    let pid = child.id().expect("newly spawned child has a pid");
    let mut stdin = child.stdin.take().expect("piped stdin");
    let stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let (writer, mut outgoing) = mpsc::unbounded_channel::<String>();
    let agent = Arc::new(AgentProcess {
        key,
        pid,
        spawned_at: Instant::now(),
        capabilities: OnceLock::new(),
        writer,
        pending: Mutex::new(HashMap::new()),
        next_id: AtomicU64::new(1),
        sessions: Mutex::new(HashMap::new()),
        unrouted: Mutex::new(HashMap::new()),
        stderr_tail: Mutex::new(VecDeque::with_capacity(STDERR_TAIL)),
        child: Mutex::new(Some(child)),
        stopping: AtomicBool::new(false),
        exited: AtomicBool::new(false),
        exit_status: Mutex::new(None),
        exit_hook: OnceLock::new(),
        exit_notified: AtomicBool::new(false),
    });

    let weak = Arc::downgrade(&agent);
    tokio::spawn(async move {
        while let Some(mut line) = outgoing.recv().await {
            line.push('\n');
            if let Err(error) = stdin.write_all(line.as_bytes()).await {
                tracing::warn!(%error, "ACP stdin write failed");
                if let Some(agent) = weak.upgrade() {
                    agent.force_kill();
                    agent.mark_exited();
                }
                break;
            }
        }
    });

    let weak = Arc::downgrade(&agent);
    tokio::spawn(async move {
        let mut stdout = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            // Cap the read itself, not just the resulting allocation after a newline.
            let mut bounded = (&mut stdout).take((MAX_LINE + 1) as u64);
            let read = bounded.read_line(&mut line).await;
            let oversized = bounded.limit() == 0;
            let Some(agent) = weak.upgrade() else { break };
            if oversized {
                tracing::warn!("agent sent an oversized message");
                agent.force_kill();
                agent.mark_exited();
                break;
            }
            match read {
                Ok(0) => {
                    if !agent.poll_exit() {
                        agent.force_kill();
                    }
                    agent.mark_exited();
                    break;
                }
                Ok(_) => match serde_json::from_str::<Value>(&line) {
                    Ok(message) => agent.route(message),
                    Err(error) => tracing::warn!(%error, "malformed ACP JSON line"),
                },
                Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
                    tracing::warn!(%error, "malformed ACP UTF-8 line");
                }
                Err(error) => {
                    tracing::warn!(%error, "ACP stdout read failed");
                    agent.force_kill();
                    agent.mark_exited();
                    break;
                }
            }
        }
    });

    let weak = Arc::downgrade(&agent);
    tokio::spawn(async move {
        let mut bytes = [0; STDERR_TAIL];
        let mut line = Vec::new();
        loop {
            let count = match stderr.read(&mut bytes).await {
                Ok(0) => break,
                Ok(count) => count,
                Err(error) => {
                    tracing::debug!(%error, "ACP stderr read failed");
                    break;
                }
            };
            let Some(agent) = weak.upgrade() else { break };
            {
                let mut tail = agent.stderr_tail.lock();
                let remove = (tail.len() + count).saturating_sub(STDERR_TAIL);
                tail.drain(..remove);
                tail.extend(&bytes[..count]);
            }
            for byte in &bytes[..count] {
                if *byte == b'\n' {
                    tracing::debug!(pid, stderr = %String::from_utf8_lossy(&line), "ACP stderr");
                    line.clear();
                } else {
                    line.push(*byte);
                    if line.len() == STDERR_TAIL {
                        tracing::debug!(pid, stderr = %String::from_utf8_lossy(&line), "ACP stderr");
                        line.clear();
                    }
                }
            }
        }
        if !line.is_empty() {
            tracing::debug!(pid, stderr = %String::from_utf8_lossy(&line), "ACP stderr");
        }
    });

    let weak = Arc::downgrade(&agent);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(EXIT_POLL).await;
            let Some(agent) = weak.upgrade() else { break };
            if agent.poll_exit() {
                agent.mark_exited();
                break;
            }
        }
    });

    let initialized = agent
        .request(
            "initialize",
            json!({
                "protocolVersion":1,
                "clientCapabilities":{
                    "fs":{"readTextFile":false,"writeTextFile":false},
                    "terminal":false,"elicitation":{"form":{}}
                },
                "clientInfo":{"name":"sworm","title":"Sworm","version":env!("CARGO_PKG_VERSION")}
            }),
            Some(Duration::from_secs(30)),
        )
        .await;
    let response = match initialized {
        Ok(response) => response,
        Err(error) => {
            agent.stop(Duration::from_secs(2)).await;
            let stderr = agent
                .last_stderr_line()
                .map(|line| format!(": {line}"))
                .unwrap_or_default();
            return Err(format!(
                "{label} chat agent failed to start: {error}{stderr}"
            ));
        }
    };
    if response.get("protocolVersion").and_then(Value::as_u64) != Some(1) {
        agent.stop(Duration::from_secs(2)).await;
        let version = response.get("protocolVersion").unwrap_or(&Value::Null);
        return Err(format!("{label} speaks ACP v{version}; Sworm supports v1"));
    }
    let capabilities = &response["agentCapabilities"];
    let _ = agent.capabilities.set(AgentCapabilities {
        load_session: capabilities["loadSession"] == true,
        close_session: capabilities["sessionCapabilities"].get("close").is_some(),
        images: capabilities["promptCapabilities"]["image"] == true,
        fork: capabilities["sessionCapabilities"]["fork"].is_object(),
    });
    Ok(agent)
}
