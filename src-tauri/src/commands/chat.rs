use crate::app_state::AppState;
use crate::host_events::channel_sink;
use sworm_core::errors::ApiError;
use sworm_protocol::chat::{ChatEvent, ChatStartInfo};

#[tauri::command]
pub async fn chat_start(
    run_id: String,
    folder_path: String,
    harness_id: String,
    preset: Option<String>,
    session_id: Option<String>,
    events: tauri::ipc::Channel<ChatEvent>,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<ChatStartInfo, ApiError> {
    let owner = window.label().to_string();
    let info = state
        .router
        .chat_start(
            run_id.clone(),
            folder_path,
            harness_id,
            preset,
            session_id,
            channel_sink(events),
            Some(owner.clone()),
        )
        .await?;
    if state.windows.owner_closed(&owner) {
        state.router.chat_stop(run_id).await?;
    }
    Ok(info)
}
