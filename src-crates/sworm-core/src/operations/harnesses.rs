use crate::{
    errors::ApiError,
    services::{
        env::EnvironmentService, extensions::ExtensionService, folders::resolve_folder, harnesses,
        settings_resolution::resolve_effective_settings_for_folder_path,
    },
    Host,
};
use sworm_protocol::harness::HarnessListing;

impl Host {
    /// List all harnesses with their detection status and the configured default preset.
    pub fn harness_list(&self) -> Result<HarnessListing, ApiError> {
        let effective =
            resolve_effective_settings_for_folder_path(None).map_err(ApiError::Internal)?;
        Ok(HarnessListing {
            harnesses: harnesses::detect_all(
                &ExtensionService::current(),
                &effective.settings,
                &EnvironmentService::current().base,
            ),
            default_preset: effective.settings.sessions.default_preset.clone(),
        })
    }

    pub fn harness_list_for_folder(&self, folder_path: String) -> Result<HarnessListing, ApiError> {
        let folder = resolve_folder(&folder_path)?;
        let env = self.folder_env(&folder);
        let effective = resolve_effective_settings_for_folder_path(Some(&folder))
            .map_err(ApiError::Internal)?;
        Ok(HarnessListing {
            harnesses: harnesses::detect_all(
                &ExtensionService::current(),
                &effective.settings,
                &env,
            ),
            default_preset: effective.settings.sessions.default_preset.clone(),
        })
    }
}
