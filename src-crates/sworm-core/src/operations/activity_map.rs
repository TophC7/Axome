use crate::{errors::ApiError, Host};
use sworm_protocol::activity_map::DiscoveredProject;

impl Host {
    pub fn activity_map_get(&self) -> Result<Vec<DiscoveredProject>, ApiError> {
        self.session_index.bind_runtime();
        if !self.session_index.has_global_rows()? {
            self.session_index.refresh_global()?;
        }
        self.session_index.activity_map()
    }

    pub fn activity_map_refresh(&self) -> Result<Vec<DiscoveredProject>, ApiError> {
        self.session_index.bind_runtime();
        self.session_index.refresh_global()?;
        self.session_index.activity_map()
    }
}
