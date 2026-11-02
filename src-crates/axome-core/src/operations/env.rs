use crate::{errors::ApiError, services::env::EnvironmentService, Host};

impl Host {
    pub fn environment_reload(&self) -> Result<(), ApiError> {
        EnvironmentService::reload();
        self.axo.reload_exec_env(None);
        Ok(())
    }
}
