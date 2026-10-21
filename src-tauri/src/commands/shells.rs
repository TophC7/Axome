use crate::app_state::AppState;
use crate::host_events::channel_sink;
use sworm_core::errors::ApiError;
use sworm_protocol::pty::PtyEvent;

#[tauri::command]
pub async fn shell_start(
    run_id: String,
    folder_path: String,
    preset: Option<String>,
    cols: u16,
    rows: u16,
    output: tauri::ipc::Channel<Vec<u8>>,
    events: tauri::ipc::Channel<PtyEvent>,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<(), ApiError> {
    let owner = window.label().to_string();
    state
        .router
        .shell_start(
            run_id.clone(),
            folder_path,
            preset,
            cols,
            rows,
            channel_sink(output),
            channel_sink(events),
            Some(owner.clone()),
        )
        .await?;
    if state.windows.owner_closed(&owner) {
        state.router.shell_stop(run_id).await?;
    }
    Ok(())
}
