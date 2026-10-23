use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use axo::{EnvMap, ExecEnv, Resolved};
use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;

use crate::services::db::DatabaseService;
use crate::services::env::{folder_nix, EnvironmentService};

pub(crate) struct FolderExecEnv {
    db: Arc<DatabaseService>,
    folder: PathBuf,
    cached: Mutex<Option<Resolved>>,
}

impl FolderExecEnv {
    pub(crate) fn new(db: Arc<DatabaseService>, folder: PathBuf) -> Self {
        Self {
            db,
            folder,
            cached: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ExecEnv for FolderExecEnv {
    async fn resolve(
        &self,
        _cwd: &Path,
        _cancel: &CancellationToken,
        _progress: &(dyn for<'a> Fn(&'a str) + Send + Sync),
    ) -> Resolved {
        let mut cached = self.cached.lock();
        if let Some(resolved) = cached.as_ref() {
            return resolved.clone();
        }
        let nix = folder_nix(&self.db, &self.folder);
        let env: EnvMap = EnvironmentService::current()
            .with_nix(nix.as_ref())
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();
        let resolved = Resolved {
            env: Arc::new(env),
            notice: None,
            prompt_note: nix.is_some().then(|| {
                format!(
                    "Commands run inside the Nix dev shell of {}; its tools are already on PATH.",
                    self.folder.display()
                )
            }),
        };
        *cached = Some(resolved.clone());
        resolved
    }

    fn invalidate(&self) {
        *self.cached.lock() = None;
    }
}
