//! Exact-launch pooling. Bind once to `Weak<ChatInner>`; unexpected exits call
//! `ChatInner::agent_exited` after removing the same process from the pool.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, OnceLock, Weak,
    },
    time::Duration,
};

use parking_lot::Mutex;
use tokio::sync::Mutex as AsyncMutex;

use super::{
    acp::{self, AgentKey, AgentProcess},
    ChatInner,
};

const IDLE_SHUTDOWN: Duration = Duration::from_secs(60);

pub(crate) struct AgentPool {
    inner: Arc<PoolInner>,
}

struct PoolInner {
    agents: Mutex<HashMap<AgentKey, Arc<AgentProcess>>>,
    idle: Mutex<HashMap<AgentKey, u64>>,
    spawn_locks: Mutex<HashMap<AgentKey, Arc<AsyncMutex<()>>>>,
    service: OnceLock<Weak<ChatInner>>,
    shutdown: AtomicBool,
}

impl AgentPool {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(PoolInner {
                agents: Mutex::new(HashMap::new()),
                idle: Mutex::new(HashMap::new()),
                spawn_locks: Mutex::new(HashMap::new()),
                service: OnceLock::new(),
                shutdown: AtomicBool::new(false),
            }),
        }
    }

    pub(crate) fn bind(&self, service: Weak<ChatInner>) {
        let _ = self.inner.service.set(service);
    }

    pub(crate) async fn acquire(
        &self,
        key: AgentKey,
        label: &str,
    ) -> Result<Arc<AgentProcess>, String> {
        let lock = self
            .inner
            .spawn_locks
            .lock()
            .entry(key.clone())
            .or_default()
            .clone();
        let _spawn = lock.lock().await;
        {
            let agents = self.inner.agents.lock();
            if self.inner.shutdown.load(Ordering::Acquire) {
                return Err("chat agent pool is shut down".into());
            }
            self.inner.bump_generation(&key);
            if let Some(agent) = agents.get(&key).filter(|agent| agent.is_alive()) {
                return Ok(agent.clone());
            }
        }

        let agent = acp::spawn(key.clone(), label).await?;
        {
            let mut agents = self.inner.agents.lock();
            if self.inner.shutdown.load(Ordering::Acquire) {
                agent.kill();
                return Err("chat agent pool is shut down".into());
            }
            agents.insert(key, agent.clone());
        }
        let pool = Arc::downgrade(&self.inner);
        agent.set_exit_hook(Box::new(move |agent| {
            if let Some(pool) = pool.upgrade() {
                pool.agent_exited(agent);
            }
        }));
        if !agent.is_alive() {
            let stderr = agent
                .last_stderr_line()
                .map(|line| format!(": {line}"))
                .unwrap_or_default();
            return Err(format!(
                "{label} chat agent failed to start: agent exited{stderr}"
            ));
        }
        Ok(agent)
    }

    pub(crate) fn release(&self, agent: &Arc<AgentProcess>) {
        let generation = {
            let agents = self.inner.agents.lock();
            if !agents
                .get(&agent.key)
                .is_some_and(|current| Arc::ptr_eq(current, agent))
                || !agent.sessions_is_empty()
            {
                return;
            }
            self.inner.bump_generation(&agent.key)
        };
        let pool = Arc::downgrade(&self.inner);
        let agent = agent.clone();
        tokio::spawn(async move {
            tokio::time::sleep(IDLE_SHUTDOWN).await;
            let Some(pool) = pool.upgrade() else { return };
            let removed = {
                // Acquire and the idle timer use the same map lock, so a lease
                // cannot be returned between checking its generation and removal.
                let mut agents = pool.agents.lock();
                let idle = pool.idle.lock();
                if idle.get(&agent.key) == Some(&generation)
                    && agents
                        .get(&agent.key)
                        .is_some_and(|current| Arc::ptr_eq(current, &agent))
                    && agent.sessions_is_empty()
                {
                    agents.remove(&agent.key);
                    true
                } else {
                    false
                }
            };
            if removed {
                agent.stop(Duration::from_secs(2)).await;
            }
        });
    }

    /// Kill-only: Host shutdown can run on a runtime worker or a blocking thread.
    pub(crate) fn shutdown(&self) {
        let agents = {
            let mut agents = self.inner.agents.lock();
            self.inner.shutdown.store(true, Ordering::Release);
            self.inner.idle.lock().clear();
            std::mem::take(&mut *agents)
        };
        for agent in agents.into_values() {
            agent.kill();
        }
    }
}

impl PoolInner {
    fn bump_generation(&self, key: &AgentKey) -> u64 {
        let mut idle = self.idle.lock();
        let generation = idle.entry(key.clone()).or_default();
        *generation = generation.wrapping_add(1);
        *generation
    }

    fn agent_exited(&self, agent: Arc<AgentProcess>) {
        {
            let mut agents = self.agents.lock();
            if agents
                .get(&agent.key)
                .is_some_and(|current| Arc::ptr_eq(current, &agent))
            {
                agents.remove(&agent.key);
            }
        }
        if let Some(service) = self.service.get().and_then(Weak::upgrade) {
            service.agent_exited(agent);
        }
    }
}
