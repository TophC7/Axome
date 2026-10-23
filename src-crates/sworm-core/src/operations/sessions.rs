use crate::errors::ApiError;
use crate::host::Host;
use crate::services::extensions::missing_requirement;
use crate::services::folders::resolve_folder;
use crate::services::harnesses::{
    resolve_harness_command, resolve_harness_launch, terminal_args, SessionLaunch,
};
use crate::services::history::{binding::PendingRun, HistorySource};
use crate::services::omp;
use crate::services::pty::{CompletedRunSink, PtyRunState, PtySubscriber};
use crate::services::runs::RunKind;
use parking_lot::Mutex;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::SystemTime;
use sworm_protocol::session::SessionStartInfo;
use tracing::{info, warn};

impl Host {
    /// Caller holds `session_claims`. Errors when another live terminal run on this host holds the session.
    pub(crate) fn ensure_session_free(
        &self,
        harness_id: &str,
        session_id: &str,
        run_id: &str,
    ) -> Result<(), ApiError> {
        if self
            .runs
            .session_runs(harness_id, session_id)
            .iter()
            .any(|id| id != run_id && self.pty.run_state(id) == Some(PtyRunState::Live))
        {
            return Err(ApiError::InvalidArgument(format!(
                "Session {session_id} is already open in another tab"
            )));
        }
        Ok(())
    }

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
        self.session_index.bind_runtime();
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
                token,
                completed,
            );
            match result {
                Ok(info) => Ok(info),
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
        let launch =
            resolve_harness_launch(&self.db, Some(folder), &harness_id, preset.as_deref())?;
        if let Some(message) = missing_requirement(
            &launch.harness().requires,
            launch.env.get("PATH").map(String::as_str).unwrap_or(""),
        ) {
            return Err(ApiError::InvalidArgument(message));
        }
        let harness = launch.harness();
        let label = &harness.definition.label;
        let history = HistorySource::from_launch(&launch)
            .unwrap_or_else(|error| {
                warn!("{label} history unavailable: {error}");
                None
            })
            .map(Arc::new);
        let config = &launch.config;
        let cwd = folder.to_string_lossy().into_owned();
        let id = harness_id;
        let path = launch.env.get("PATH").map(String::as_str).unwrap_or("");
        let cli_cmd =
            resolve_harness_command(harness, path, config.binary_path_override.as_deref())
                .map_err(ApiError::InvalidArgument)?;
        let terminal = &harness.definition.terminal;
        let session_id = if terminal.resume_args.is_empty() {
            None
        } else {
            session_id.filter(|session_id| {
                let Some(history) = &history else {
                    return true;
                };
                match history.contains(folder, session_id) {
                    Ok(false) => {
                        warn!("{label} {session_id} no longer exists, starting fresh");
                        false
                    }
                    Ok(true) => true,
                    Err(error) => {
                        warn!("{label} {session_id} history validation failed, keeping session id: {error}");
                        true
                    }
                }
            })
        };
        let minted = (session_id.is_none() && !terminal.new_session_args.is_empty())
            .then(|| uuid::Uuid::new_v4().to_string());
        let binding = session_id.is_none()
            && minted.is_none()
            && !terminal.resume_args.is_empty()
            && history.is_some();
        let session_launch = match (session_id.as_deref(), minted.as_deref()) {
            (Some(id), _) => SessionLaunch::Resume(id),
            (_, Some(id)) => SessionLaunch::Mint(id),
            _ => SessionLaunch::Fresh,
        };
        let args = terminal_args(
            terminal,
            session_launch,
            &config.extra_args,
            &launch.preset.args,
        );
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let mut child_env = launch.env;

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

        let binder = self.history_binder.clone();
        // Only harnesses with a history source are indexed.
        let index = history.is_some().then(|| Arc::clone(&self.session_index));
        let exit_folder = folder.to_path_buf();
        let exit_harness_id = id.clone();
        let exit_preset = preset.clone();
        let exited = binding.then(|| Arc::new(AtomicBool::new(false)));
        let exit_flag = exited.clone();
        let on_exit: Box<dyn FnOnce(&str, Option<i32>) + Send> = Box::new(move |rid, code| {
            info!("Run {rid} exited with code {code:?}");
            if let Some(exit_flag) = exit_flag {
                exit_flag.store(true, Ordering::Release);
            }
            binder.cancel(rid);
            if let Some(index) = index {
                index.schedule(&exit_folder, &exit_harness_id, exit_preset.as_deref());
            }
        });

        let _claim = self.session_claims.lock();
        if let Some(session_id) = session_id.as_deref() {
            self.ensure_session_free(&id, session_id, &run_id)?;
        }
        *token.lock() = session_id.clone().or_else(|| minted.clone());

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
        drop(_claim);

        match &session_id {
            Some(id) => self.history_binder.claim(id),
            None if binding => {
                let tracked_run_id = run_id.clone();
                let index = Arc::clone(&self.session_index);
                let folder = folder.to_path_buf();
                let bound_folder = folder.clone();
                let bound_preset = preset.clone();
                self.history_binder.track(PendingRun {
                    run_id,
                    source: history.expect("binding run has a history source"),
                    preset,
                    folder,
                    spawned_at,
                    event_sink,
                    on_bound: Box::new(move |bound| {
                        *token.lock() = Some(bound);
                        index.schedule(&bound_folder, &id, bound_preset.as_deref());
                    }),
                });
                // Exit can beat registration; its callback then had no pending
                // run to cancel. Check the same flag after registering.
                if exited
                    .expect("binding run has exit flag")
                    .load(Ordering::Acquire)
                {
                    self.history_binder.cancel(&tracked_run_id);
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
