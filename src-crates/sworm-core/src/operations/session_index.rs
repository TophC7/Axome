use crate::{errors::ApiError, services::folders::resolve_folder, Host};
use sworm_protocol::session::SessionIndexSnapshot;

impl Host {
    pub fn session_index_get(&self, folder_path: String) -> Result<SessionIndexSnapshot, ApiError> {
        self.session_index.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.session_index.folder_snapshot(&folder)
    }

    pub fn session_index_refresh(
        &self,
        folder_path: String,
    ) -> Result<SessionIndexSnapshot, ApiError> {
        self.session_index.bind_runtime();
        let folder = resolve_folder(&folder_path)?;
        self.session_index.refresh_folder(&folder)
    }
}
