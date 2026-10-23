use crate::stream::{StreamReader, StreamWriter};
use std::sync::Arc;
use sworm_core::{errors::ApiError, events::EventSink, Host};
use sworm_protocol::{
    chat::ChatEvent,
    rpc::{ChatDown, WireError},
};
use tokio::sync::{mpsc, watch};
use uuid::Uuid;

pub(crate) async fn run(
    host: Arc<Host>,
    folder_path: String,
    session_id: String,
    mut send: StreamWriter,
    mut recv: StreamReader,
    mut shutdown: watch::Receiver<bool>,
) {
    let (sender, mut events) = mpsc::channel::<ChatEvent>(1024);
    let sink: EventSink<ChatEvent> = Arc::new(move |event| {
        sender.try_send(event).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => "chat stream lagging".to_owned(),
            mpsc::error::TrySendError::Closed(_) => "chat stream closed".to_owned(),
        })
    });
    let attach_id = Uuid::new_v4().to_string();
    let attach_host = Arc::clone(&host);
    let id = session_id.clone();
    let attachment = attach_id.clone();
    let attached = tokio::task::spawn_blocking(move || {
        attach_host.chat_attach(folder_path, id, attachment, sink)
    })
    .await
    .map_err(ApiError::from)
    .and_then(|result| result);
    if let Err(error) = attached {
        let _ = send
            .write_json(&ChatDown::Closed {
                error: WireError::from(error),
            })
            .await;
        let _ = send.finish().await;
        return;
    }

    // Keep the read future alive across events so a partial frame is never cancelled.
    let input = async { while recv.read_tagged::<serde::de::IgnoredAny>().await.is_ok() {} };
    tokio::pin!(input);
    loop {
        let event = tokio::select! {
            event = events.recv() => event,
            _ = &mut input => break,
            _ = shutdown.changed() => break,
        };
        let Some(event) = event else { break };
        let closed = matches!(&event, ChatEvent::Closed { .. });
        if send.write_json(&ChatDown::Event { event }).await.is_err() || closed {
            break;
        }
    }
    let _ = send.finish().await;
    if let Err(error) =
        tokio::task::spawn_blocking(move || host.chat_detach(&session_id, &attach_id)).await
    {
        tracing::warn!(%error, "chat stream detach task failed");
    }
}
