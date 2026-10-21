use crate::errors::ApiError;
use crate::host::Host;
use crate::services::extensions::{missing_requirement, ExtensionService};
use crate::services::folders::resolve_folder;
use crate::services::harness_scanners::Scanner;
use crate::services::harnesses::{resolve_harness_command, terminal_args, SessionLaunch};
use crate::services::omp;
use crate::services::pty::{CompletedRunSink, PtySubscriber};
use crate::services::resume_discovery::PendingRun;
use crate::services::runs::RunKind;
use crate::services::settings_resolution::resolve_effective_settings_for_folder_path;
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::SystemTime;
use sworm_protocol::session::SessionStartInfo;
use sworm_protocol::settings::PresetSettings;
use tracing::{info, warn};

/// Keep the session id only when it exists; otherwise log that the harness's
/// conversation vanished and start fresh.
fn validated_session_id(
    token: Option<String>,
    label: &str,
    exists: impl FnOnce(&str) -> bool,
) -> Option<String> {
    token.filter(|token| {
        let exists = exists(token);
        if !exists {
            warn!("{label} {token} no longer exists, starting fresh");
        }
        exists
    })
}

impl Host {
    /// Start a run in a PTY inside `folder_path`.
    ///
    /// Resume semantics come from the harness's `terminal` contribution.
    pub fn session_start(
        &self,
        run_id: String,
        folder_path: String,
        harness_id: String,
        preset: Option<String>,
        session_id: Option<String>,
        cols: u16,
        rows: u16,
        subscriber: Option<PtySubscriber>,
        owner_id: Option<String>,
    ) -> Result<SessionStartInfo, ApiError> {
        let _owner = self.runs.owner_activity(owner_id.as_deref())?;
        let folder = resolve_folder(&folder_path)?;
        let kind = RunKind::Session {
            harness_id: harness_id.clone(),
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
                return Ok(SessionStartInfo {
                    resumed: true,
                    session_id: self.runs.session_id(&run_id),
                });
            }
            let (token, completed) = runs.reserve(&run_id, folder.clone(), kind);
            let token = token.expect("session reservation has token cell");
            let result = self.spawn_session(
                run_id.clone(),
                &folder,
                harness_id,
                preset,
                session_id,
                cols,
                rows,
                subscriber,
                owner_id,
                token.clone(),
                completed,
            );
            match result {
                Ok(info) => {
                    let mut current = token.lock();
                    if current.is_none() {
                        *current = info.session_id.clone();
                    }
                    Ok(info)
                }
                Err(error) => {
                    runs.abort(&run_id);
                    Err(error)
                }
            }
        })
    }

    fn spawn_session(
        &self,
        run_id: String,
        folder: &std::path::Path,
        harness_id: String,
        preset: Option<String>,
        session_id: Option<String>,
        cols: u16,
        rows: u16,
        subscriber: Option<PtySubscriber>,
        owner_id: Option<String>,
        token: Arc<Mutex<Option<String>>>,
        completed: Option<CompletedRunSink>,
    ) -> Result<SessionStartInfo, ApiError> {
        let effective_settings =
            resolve_effective_settings_for_folder_path(Some(folder)).map_err(ApiError::Internal)?;
        let settings = &effective_settings.settings;
        let mut child_env = self.folder_env(folder);
        let cwd = folder.to_string_lossy().into_owned();
        let id = harness_id;
        let catalog = ExtensionService::current();
        let harness = catalog
            .find_harness(&id)
            .ok_or_else(|| ApiError::InvalidArgument(format!("Unknown harness: {id}")))?;
        let config = settings.harnesses.get(&id).cloned().unwrap_or_default();
        if !config.enabled {
            return Err(ApiError::InvalidArgument(format!(
                "Harness disabled by settings: {id}"
            )));
        }
        let preset_settings =
            match preset {
                None => PresetSettings::default(),
                Some(name) => config.presets.get(&name).cloned().ok_or_else(|| {
                    ApiError::InvalidArgument(format!("Unknown preset {id}/{name}"))
                })?,
            };
        if let Some(message) = missing_requirement(&harness.requires, &child_env["PATH"]) {
            return Err(ApiError::InvalidArgument(message));
        }
        let cli_cmd = resolve_harness_command(
            harness,
            &child_env["PATH"],
            config.binary_path_override.as_deref(),
        )
        .map_err(ApiError::InvalidArgument)?;
        let terminal = &harness.definition.terminal;
        let scanner = Scanner::for_harness(&id);
        let session_id = if terminal.resume_args.is_empty() {
            None
        } else {
            validated_session_id(session_id, &harness.definition.label, |id| {
                scanner.map_or(true, |s| s.session_exists(&cwd, id))
            })
        };
        let minted = (session_id.is_none() && !terminal.new_session_args.is_empty())
            .then(|| uuid::Uuid::new_v4().to_string());
        let launch = match (session_id.as_deref(), minted.as_deref()) {
            (Some(id), _) => SessionLaunch::Resume(id),
            (_, Some(id)) => SessionLaunch::Mint(id),
            _ => SessionLaunch::Fresh,
        };
        let args = terminal_args(terminal, launch, &config.extra_args, &preset_settings.args);
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        child_env.extend(preset_settings.env);

        // Bridge values override inherited, Nix-provided and preset values.
        {
            match self.issue_bridge.ensure_running(folder) {
                Ok(info) => {
                    child_env.insert("SWORM_PROJECT_PATH".to_string(), info.project_path);
                    child_env.insert("SWORM_ISSUES_SOCKET".to_string(), info.socket_path);
                    child_env.insert("SWORM_ISSUES_TOKEN".to_string(), info.token);
                    child_env.insert(
                        "SWORM_ISSUES_PROTOCOL_VERSION".to_string(),
                        info.protocol_version.to_string(),
                    );
                }
                Err(error) => {
                    warn!("Issue bridge unavailable for run {run_id} in {cwd}: {error}");
                }
            }
        }

        let discovery = self.resume_discovery.clone();
        let discovering =
            session_id.is_none() && minted.is_none() && scanner.is_some_and(Scanner::discovers);
        let exited = discovering.then(|| Arc::new(AtomicBool::new(false)));
        let exit_flag = exited.clone();
        let on_exit: Box<dyn FnOnce(&str, Option<i32>) + Send> = Box::new(move |rid, code| {
            info!("Run {rid} exited with code {code:?}");
            if let Some(exit_flag) = exit_flag {
                exit_flag.store(true, Ordering::Release);
            }
            discovery.cancel(rid);
        });

        // Taken before spawn so nothing the process creates can predate it.
        let spawned_at = SystemTime::now();
        let event_sink = self
            .pty
            .spawn(
                run_id.clone(),
                folder,
                &cli_cmd,
                &arg_refs,
                Some(&cwd),
                Some(&child_env),
                cols,
                rows,
                subscriber,
                owner_id,
                Some(on_exit),
                completed,
            )
            .map_err(ApiError::Pty)?;

        match &session_id {
            Some(id) => self.resume_discovery.claim(id),
            None if discovering => {
                let tracked_run_id = run_id.clone();
                self.resume_discovery.track(PendingRun {
                    run_id,
                    scanner: scanner.expect("discoverable harness has scanner"),
                    cwd,
                    spawned_at,
                    event_sink,
                    on_bound: Box::new(move |bound| *token.lock() = Some(bound)),
                });
                // Exit can beat registration; its callback then had no pending
                // run to cancel. Check the same flag after registering.
                if exited
                    .expect("discoverable run has exit flag")
                    .load(Ordering::Acquire)
                {
                    self.resume_discovery.cancel(&tracked_run_id);
                }
            }
            None => {}
        }

        Ok(SessionStartInfo {
            resumed: session_id.is_some(),
            session_id: session_id.or(minted),
        })
    }

    /// Stop a session and release its retained record and transcript.
    pub fn session_stop(&self, run_id: String) -> Result<(), ApiError> {
        self.runs.with_run(&run_id, |runs| {
            // Stop checks only the domain discriminant.
            runs.stop(
                &run_id,
                RunKind::Session {
                    harness_id: String::new(),
                    preset: None,
                },
                &self.pty,
            )?;
            self.forget_run(&run_id);
            Ok(())
        })
    }

    pub fn omp_resolve_uri(
        &self,
        uri: String,
        cwd: Option<String>,
    ) -> Result<sworm_protocol::omp::OmpResolvedTarget, ApiError> {
        omp::resolve_omp_target(&uri, cwd.as_deref())
    }
}
