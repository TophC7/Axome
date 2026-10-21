use crate::app_state::AppState;
use crate::host_events::channel_sink;
use sworm_core::errors::ApiError;
use sworm_protocol::pty::PtyEvent;
use sworm_protocol::session::SessionStartInfo;

#[tauri::command]
pub async fn session_start(
    run_id: String,
    folder_path: String,
    harness_id: String,
    preset: Option<String>,
    session_id: Option<String>,
    cols: u16,
    rows: u16,
    output: tauri::ipc::Channel<Vec<u8>>,
    events: tauri::ipc::Channel<PtyEvent>,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<SessionStartInfo, ApiError> {
    let owner = window.label().to_string();
    let info = state
        .router
        .session_start(
            run_id.clone(),
            folder_path,
            harness_id,
            preset,
            session_id,
            cols,
            rows,
            channel_sink(output),
            channel_sink(events),
            Some(owner.clone()),
        )
        .await?;
    if state.windows.owner_closed(&owner) {
        state.router.session_stop(run_id).await?;
    }
    Ok(info)
}
