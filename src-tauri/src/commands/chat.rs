use crate::app_state::AppState;
use crate::host_events::channel_sink;
use axome_core::errors::ApiError;
use axome_protocol::chat::ChatEvent;

#[tauri::command]
pub async fn chat_attach(
    attach_id: String,
    folder_path: String,
    session_id: String,
    events: tauri::ipc::Channel<ChatEvent>,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<(), ApiError> {
    let owner = window.label().to_string();
    state
        .router
        .chat_attach(
            attach_id.clone(),
            folder_path,
            session_id,
            channel_sink(events),
            owner.clone(),
        )
        .await?;
    if state.windows.owner_closed(&owner) {
        state.router.chat_detach(attach_id).await;
    }
    Ok(())
}

#[tauri::command]
pub async fn chat_detach(
    attach_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), ApiError> {
    state.router.chat_detach(attach_id).await;
    Ok(())
}
