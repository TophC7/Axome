use super::RouterInner;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Weak,
    },
    time::Duration,
};
use sworm_core::{errors::ApiError, events::EventSink};
use sworm_protocol::{
    chat::{ChatEvent, ChatTurn},
    rpc::{ChatDown, Open},
};
use sworm_remote::wire::read_frame;
use tokio::task::JoinHandle;

pub(crate) struct RemoteChatService {
    chats: Mutex<HashMap<String, RemoteChat>>,
    next_generation: AtomicU64,
}

struct RemoteChat {
    server: String,
    workbench: String,
    owner_id: Option<String>,
    generation: u64,
    task: JoinHandle<()>,
}

impl RemoteChatService {
    pub(crate) fn new() -> Self {
        Self {
            chats: Mutex::new(HashMap::new()),
            next_generation: AtomicU64::new(1),
        }
    }

    pub(crate) fn attach(
        &self,
        router: Weak<RouterInner>,
        run_id: String,
        server: String,
        workbench: String,
        owner_id: Option<String>,
        events: EventSink<ChatEvent>,
    ) {
        let mut chats = self.chats.lock();
        if let Some(chat) = chats.remove(&run_id) {
            chat.task.abort();
        }
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        // Register under the lock so a fast pump cannot finish before insertion.
        let task = tokio::spawn(pump(
            router,
            run_id.clone(),
            server.clone(),
            generation,
            events,
        ));
        chats.insert(
            run_id,
            RemoteChat {
                server,
                workbench,
                owner_id,
                generation,
                task,
            },
        );
    }

    pub(crate) fn target(&self, run_id: &str) -> Option<(String, String)> {
        self.chats
            .lock()
            .get(run_id)
            .map(|chat| (chat.server.clone(), chat.workbench.clone()))
    }

    pub(crate) fn cancel(&self, run_id: &str) {
        if let Some(chat) = self.chats.lock().remove(run_id) {
            chat.task.abort();
        }
    }

    pub(crate) fn release(&self, owner: &str, run_ids: &[String]) {
        let mut chats = self.chats.lock();
        for run_id in run_ids {
            if chats
                .get(run_id)
                .is_some_and(|chat| chat.owner_id.as_deref() == Some(owner))
            {
                if let Some(chat) = chats.remove(run_id) {
                    chat.task.abort();
                }
            }
        }
    }

    pub(crate) fn release_owner(&self, owner: &str) -> Vec<(String, String, String)> {
        let mut chats = self.chats.lock();
        let ids: Vec<String> = chats
            .iter()
            .filter(|(_, chat)| chat.owner_id.as_deref() == Some(owner))
            .map(|(run_id, _)| run_id.clone())
            .collect();
        ids.into_iter()
            .filter_map(|run_id| {
                let chat = chats.remove(&run_id)?;
                chat.task.abort();
                Some((run_id, chat.server, chat.workbench))
            })
            .collect()
    }

    fn current(&self, run_id: &str, generation: u64) -> bool {
        self.chats
            .lock()
            .get(run_id)
            .is_some_and(|chat| chat.generation == generation)
    }

    fn finish(&self, run_id: &str, generation: u64) {
        let mut chats = self.chats.lock();
        if chats
            .get(run_id)
            .is_some_and(|chat| chat.generation == generation)
        {
            chats.remove(run_id);
        }
    }
}

fn current(router: &Weak<RouterInner>, run_id: &str, generation: u64) -> bool {
    router
        .upgrade()
        .is_some_and(|router| router.remote_chat.current(run_id, generation))
}

async fn pump(
    router: Weak<RouterInner>,
    run_id: String,
    server: String,
    generation: u64,
    events: EventSink<ChatEvent>,
) {
    let mut attempt = 0;
    'reconnect: while current(&router, &run_id, generation) {
        let Some(router_now) = router.upgrade() else {
            break;
        };
        let client = router_now.client(&server).await;
        drop(router_now);
        if let Ok(client) = client {
            if let Ok((send, mut recv)) = client
                .open_stream(Open::Chat {
                    run_id: run_id.clone(),
                })
                .await
            {
                while current(&router, &run_id, generation) {
                    let frame = read_frame::<ChatDown>(&mut recv).await;
                    if !current(&router, &run_id, generation) {
                        break 'reconnect;
                    }
                    match frame {
                        Ok(ChatDown::Event { event }) => {
                            if matches!(&event, ChatEvent::Snapshot { .. }) {
                                attempt = 0;
                            }
                            let exited = matches!(&event, ChatEvent::Exit { .. });
                            if events(event).is_err() || exited {
                                break 'reconnect;
                            }
                        }
                        Ok(ChatDown::Closed { error }) => {
                            let _ = events(ChatEvent::Exit {
                                message: Some(ApiError::from(error).to_string()),
                            });
                            break 'reconnect;
                        }
                        Err(_) => break,
                    }
                }
                // The server watches this half for detach; keep it open while reading.
                drop(send);
            }
            if let Some(router_now) = router.upgrade() {
                router_now.evict(&server, &client).await;
            }
        }
        if !current(&router, &run_id, generation) {
            break;
        }
        if events(ChatEvent::TurnState {
            turn: ChatTurn::Reconnecting,
        })
        .is_err()
        {
            break;
        }
        tokio::time::sleep(Duration::from_secs((1_u64 << attempt).min(30))).await;
        attempt = (attempt + 1).min(5);
    }
    if let Some(router) = router.upgrade() {
        router.remote_chat.finish(&run_id, generation);
    }
}
