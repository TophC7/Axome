use super::RouterInner;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Weak,
    },
    time::Duration,
};
use axome_core::{errors::ApiError, events::EventSink, Host};
use axome_protocol::{
    chat::ChatEvent,
    rpc::{ChatDown, Open, WireError},
};
use axome_remote::wire::read_frame;
use tokio::task::JoinHandle;

pub(crate) struct ChatAttachments {
    attachments: Mutex<HashMap<String, Attachment>>,
    next_generation: AtomicU64,
}

struct Attachment {
    owner: String,
    session_id: String,
    generation: u64,
    target: AttachTarget,
}

enum AttachTarget {
    Local,
    Remote { task: JoinHandle<()> },
}

impl ChatAttachments {
    pub(crate) fn new() -> Self {
        Self {
            attachments: Mutex::new(HashMap::new()),
            next_generation: AtomicU64::new(1),
        }
    }

    pub(crate) fn attach_local(&self, attach_id: String, owner: String, session_id: String) {
        self.attachments.lock().insert(
            attach_id,
            Attachment {
                owner,
                session_id,
                generation: self.next_generation.fetch_add(1, Ordering::Relaxed),
                target: AttachTarget::Local,
            },
        );
    }

    pub(crate) fn attach_remote(
        &self,
        router: Weak<RouterInner>,
        attach_id: String,
        owner: String,
        server: String,
        path: String,
        session_id: String,
        events: EventSink<ChatEvent>,
    ) {
        let mut attachments = self.attachments.lock();
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        // Register under the lock so a fast pump cannot finish before insertion.
        let task = tokio::spawn(pump(
            router,
            attach_id.clone(),
            server,
            path,
            session_id.clone(),
            generation,
            events,
        ));
        attachments.insert(
            attach_id,
            Attachment {
                owner,
                session_id,
                generation,
                target: AttachTarget::Remote { task },
            },
        );
    }

    pub(crate) fn detach(&self, attach_id: &str, host: &Arc<Host>) {
        let attachment = self.attachments.lock().remove(attach_id);
        if let Some(attachment) = attachment {
            attachment.detach(attach_id, host);
        }
    }

    pub(crate) fn release_owner(&self, owner: &str, host: &Arc<Host>) {
        let released: Vec<_> = {
            let mut attachments = self.attachments.lock();
            let ids: Vec<_> = attachments
                .iter()
                .filter(|(_, attachment)| attachment.owner == owner)
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter()
                .filter_map(|id| attachments.remove(&id).map(|attachment| (id, attachment)))
                .collect()
        };
        for (id, attachment) in released {
            attachment.detach(&id, host);
        }
    }

    fn current(&self, attach_id: &str, generation: u64) -> bool {
        self.attachments
            .lock()
            .get(attach_id)
            .is_some_and(|attachment| attachment.generation == generation)
    }

    fn finish(&self, attach_id: &str, generation: u64) {
        let mut attachments = self.attachments.lock();
        if attachments
            .get(attach_id)
            .is_some_and(|attachment| attachment.generation == generation)
        {
            attachments.remove(attach_id);
        }
    }
}

impl Attachment {
    fn detach(self, attach_id: &str, host: &Arc<Host>) {
        match self.target {
            AttachTarget::Local => host.chat_detach(&self.session_id, attach_id),
            AttachTarget::Remote { task } => task.abort(),
        }
    }
}

fn current(router: &Weak<RouterInner>, attach_id: &str, generation: u64) -> bool {
    router
        .upgrade()
        .is_some_and(|router| router.chats.current(attach_id, generation))
}

async fn pump(
    router: Weak<RouterInner>,
    attach_id: String,
    server: String,
    path: String,
    session_id: String,
    generation: u64,
    events: EventSink<ChatEvent>,
) {
    let mut attempt = 0;
    'reconnect: while current(&router, &attach_id, generation) {
        let Some(router_now) = router.upgrade() else {
            break;
        };
        let client = router_now.client(&server).await;
        drop(router_now);
        if let Ok(client) = client {
            if let Ok((send, mut recv)) = client
                .open_stream(Open::Chat {
                    folder_path: path.clone(),
                    session_id: session_id.clone(),
                })
                .await
            {
                while current(&router, &attach_id, generation) {
                    let frame = read_frame::<ChatDown>(&mut recv).await;
                    if !current(&router, &attach_id, generation) {
                        break 'reconnect;
                    }
                    match frame {
                        Ok(ChatDown::Event { event }) => {
                            if matches!(&event, ChatEvent::Snapshot { .. }) {
                                attempt = 0;
                            }
                            let closed = matches!(&event, ChatEvent::Closed { .. });
                            if events(event).is_err() || closed {
                                break 'reconnect;
                            }
                        }
                        Ok(ChatDown::Closed { error }) => {
                            let missing = matches!(&error, WireError::NotFound { .. });
                            let _ = events(ChatEvent::Closed {
                                message: ApiError::from(error).to_string(),
                                missing,
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
        if !current(&router, &attach_id, generation) {
            break;
        }
        if events(ChatEvent::Reconnecting).is_err() {
            break;
        }
        tokio::time::sleep(Duration::from_secs((1_u64 << attempt).min(30))).await;
        attempt = (attempt + 1).min(5);
    }
    if let Some(router) = router.upgrade() {
        router.chats.finish(&attach_id, generation);
    }
}
