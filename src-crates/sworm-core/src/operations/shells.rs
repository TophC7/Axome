use crate::errors::ApiError;
use crate::host::Host;
use crate::services::env::EnvironmentService;
use crate::services::folders::resolve_folder;
use crate::services::pty::PtySubscriber;
use crate::services::runs::RunKind;
use crate::services::settings_resolution::resolve_effective_settings_for_folder_path;
use crate::services::shells;
use sworm_protocol::shell::ShellListing;

impl Host {
    pub fn shell_start(
        &self,
        run_id: String,
        folder_path: String,
        preset: Option<String>,
        cols: u16,
        rows: u16,
        subscriber: Option<PtySubscriber>,
        owner_id: Option<String>,
    ) -> Result<(), ApiError> {
        let _owner = self.runs.owner_activity(owner_id.as_deref())?;
        let folder = resolve_folder(&folder_path)?;
        let kind = RunKind::Shell {
            preset: preset.clone(),
        };
        self.runs.with_start(&run_id, |runs| {
            if runs.reuse(
                &run_id,
                &folder,
                &kind,
                &self.pty,
                subscriber.as_ref(),
                owner_id.as_deref(),
                cols,
                rows,
            )? {
                return Ok(());
            }
            let (_, completed) = runs.reserve(&run_id, folder.clone(), kind);
            let result = (|| {
                let effective = resolve_effective_settings_for_folder_path(Some(&folder))
                    .map_err(ApiError::Internal)?;
                let launch = shells::resolve(
                    &effective.settings.shell,
                    preset.as_deref(),
                    self.folder_env(&folder),
                    &EnvironmentService::current().detected_shell,
                )
                .map_err(ApiError::InvalidArgument)?;
                let args: Vec<&str> = launch.args.iter().map(String::as_str).collect();
                self.pty
                    .spawn(
                        run_id.clone(),
                        &folder,
                        &launch.cmd,
                        &args,
                        Some(&folder.to_string_lossy()),
                        Some(&launch.env),
                        cols,
                        rows,
                        subscriber,
                        owner_id,
                        None,
                        completed,
                    )
                    .map_err(ApiError::Pty)?;
                Ok(launch.startup)
            })();
            let startup = match result {
                Ok(startup) => startup,
                Err(error) => {
                    runs.abort(&run_id);
                    return Err(error);
                }
            };
            if let Some(startup) = startup {
                if let Err(error) = self.pty.write(&run_id, format!("{startup}\n").as_bytes()) {
                    // Retain the record if cleanup fails, so the live shell remains stoppable.
                    runs.stop(&run_id, RunKind::Shell { preset: None }, &self.pty)?;
                    return Err(ApiError::Pty(error));
                }
            }
            Ok(())
        })
    }

    pub fn shell_stop(&self, run_id: String) -> Result<(), ApiError> {
        self.runs.with_run(&run_id, |runs| {
            runs.stop(&run_id, RunKind::Shell { preset: None }, &self.pty)?;
            self.forget_run(&run_id);
            Ok(())
        })
    }

    pub fn shell_list(&self) -> Result<ShellListing, ApiError> {
        let effective =
            resolve_effective_settings_for_folder_path(None).map_err(ApiError::Internal)?;
        let environment = EnvironmentService::current();
        Ok(shells::listing(
            &effective.settings.shell,
            environment.path(),
            &environment.detected_shell,
        ))
    }

    pub fn shell_list_for_folder(&self, folder_path: String) -> Result<ShellListing, ApiError> {
        let folder = resolve_folder(&folder_path)?;
        let env = self.folder_env(&folder);
        let effective = resolve_effective_settings_for_folder_path(Some(&folder))
            .map_err(ApiError::Internal)?;
        Ok(shells::listing(
            &effective.settings.shell,
            env.get("PATH").map(String::as_str).unwrap_or_default(),
            &EnvironmentService::current().detected_shell,
        ))
    }
}
