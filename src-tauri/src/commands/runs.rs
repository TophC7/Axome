use crate::app_state::AppState;
use sworm_core::errors::ApiError;

#[tauri::command]
pub async fn run_write(
    run_id: String,
    data: Vec<u8>,
    state: tauri::State<'_, AppState>,
) -> Result<(), ApiError> {
    state.router.run_write(run_id, data).await
}

#[tauri::command]
pub async fn run_resize(
    run_id: String,
    cols: u16,
    rows: u16,
    state: tauri::State<'_, AppState>,
) -> Result<(), ApiError> {
    state.router.run_resize(run_id, cols, rows).await
}
