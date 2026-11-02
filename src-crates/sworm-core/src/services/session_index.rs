//! Complete history listings replace one source atomically; failed refreshes retain cached rows.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    path::Path,
    sync::{Arc, OnceLock, Weak},
};

use chrono::{SecondsFormat, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use sworm_protocol::{
    activity_map::DiscoveredProject,
    session::{
        IndexedSession, LastMessage, MessageRole, SessionIndexSnapshot, SessionIndexSource,
        SessionIndexStatus, SessionSource, SessionSourceKey,
    },
};
use tokio::{
    runtime::Handle,
    sync::{watch, Semaphore},
    task::AbortHandle,
};

use crate::{
    errors::ApiError,
    events::{deliver, EventSink, HostEvent},
    services::{
        activity_map,
        axo::AxoService,
        db::DatabaseService,
        extensions::{ExtensionService, LoadedExtensionCatalog},
        harnesses::resolve_harness_launch,
        history::{HistoryRow, HistorySource},
        session_watcher::SessionWatcher,
        settings_resolution::SettingsResolver,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum SourceId {
    Harness(String),
    Axo,
}

impl SourceId {
    fn db(&self) -> String {
        match self {
            Self::Harness(id) => format!("harness:{id}"),
            Self::Axo => "axo".into(),
        }
    }

    fn from_db(value: &str) -> Option<Self> {
        match value {
            "axo" => Some(Self::Axo),
            _ => value
                .strip_prefix("harness:")
                .map(|id| Self::Harness(id.into())),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct SourceKey {
    pub scope: String,
    pub source: SourceId,
    pub preset: String,
}

impl SourceKey {
    pub(crate) fn new(folder: Option<&Path>, source: SourceId, preset: Option<&str>) -> Self {
        Self {
            scope: folder.map_or_else(String::new, |folder| folder.to_string_lossy().into_owned()),
            source,
            preset: preset.unwrap_or_default().to_owned(),
        }
    }

    fn folder(&self) -> Option<&Path> {
        (!self.scope.is_empty()).then(|| Path::new(&self.scope))
    }

    fn preset(&self) -> Option<&str> {
        (!self.preset.is_empty()).then_some(self.preset.as_str())
    }
}

pub(crate) struct SourceRow {
    pub key: SourceKey,
    pub refreshed_at: Option<String>,
    pub status: SessionIndexStatus,
    pub message: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct Observation {
    pub key: SourceKey,
    pub session_id: String,
    pub folder_path: String,
    pub title: Option<String>,
    pub last_message: Option<LastMessage>,
    pub message_count: Option<u32>,
    pub updated_at: Option<String>,
    pub model: Option<String>,
    pub git_branch: Option<String>,
}

fn status_literal(status: SessionIndexStatus) -> &'static str {
    match status {
        SessionIndexStatus::Pending => "pending",
        SessionIndexStatus::Ready => "ready",
        SessionIndexStatus::Missing => "missing",
        SessionIndexStatus::Error => "error",
    }
}

fn parse_status(literal: &str) -> SessionIndexStatus {
    match literal {
        "ready" => SessionIndexStatus::Ready,
        "missing" => SessionIndexStatus::Missing,
        _ => SessionIndexStatus::Error,
    }
}

pub(crate) fn commit_listing(
    conn: &Connection,
    key: &SourceKey,
    refreshed_at: &str,
    sessions: &[HistoryRow],
    message: Option<&str>,
) -> rusqlite::Result<()> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO session_index_sources
           (scope_folder, source_id, preset, refreshed_at, status, message)
         VALUES (?1, ?2, ?3, ?4, 'ready', ?5)
         ON CONFLICT (scope_folder, source_id, preset) DO UPDATE SET
           refreshed_at = excluded.refreshed_at, status = 'ready', message = excluded.message",
        params![
            key.scope,
            key.source.db(),
            key.preset,
            refreshed_at,
            message
        ],
    )?;
    tx.execute(
        "DELETE FROM indexed_sessions WHERE scope_folder = ?1 AND source_id = ?2 AND preset = ?3",
        params![key.scope, key.source.db(), key.preset],
    )?;
    {
        let mut insert = tx.prepare(
            "INSERT INTO indexed_sessions
               (scope_folder, source_id, preset, session_id, folder_path, title, updated_at, model, git_branch, last_message, message_count, last_message_role)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )?;
        for session in sessions {
            insert.execute(params![
                key.scope,
                key.source.db(),
                key.preset,
                session.id,
                session.cwd,
                session.title,
                session.updated_at,
                session.model,
                session.git_branch,
                session
                    .last_message
                    .as_ref()
                    .map(|message| message.text.as_str()),
                session.message_count,
                session
                    .last_message
                    .as_ref()
                    .map(|message| match message.role {
                        MessageRole::User => "user",
                        MessageRole::Agent => "agent",
                    }),
            ])?;
        }
    }
    tx.commit()
}

pub(crate) fn record_failure(
    conn: &Connection,
    key: &SourceKey,
    status: SessionIndexStatus,
    message: &str,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO session_index_sources (scope_folder, source_id, preset, status, message)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (scope_folder, source_id, preset) DO UPDATE SET
           status = excluded.status, message = excluded.message",
        params![
            key.scope,
            key.source.db(),
            key.preset,
            status_literal(status),
            message
        ],
    )?;
    Ok(())
}

pub(crate) fn load_sources(
    conn: &Connection,
    scope: Option<&str>,
) -> rusqlite::Result<Vec<SourceRow>> {
    let mut statement = conn.prepare(
        "SELECT scope_folder, source_id, preset, refreshed_at, status, message
         FROM session_index_sources WHERE ?1 IS NULL OR scope_folder = ?1",
    )?;
    let rows = statement.query_map(params![scope], |row| {
        let Some(source) = SourceId::from_db(&row.get::<_, String>(1)?) else {
            return Ok(None);
        };
        Ok(Some(SourceRow {
            key: SourceKey {
                scope: row.get(0)?,
                source,
                preset: row.get(2)?,
            },
            refreshed_at: row.get(3)?,
            status: parse_status(&row.get::<_, String>(4)?),
            message: row.get(5)?,
        }))
    })?;
    rows.filter_map(|row| row.transpose()).collect()
}

pub(crate) fn load_observations(
    conn: &Connection,
    scope: Option<&str>,
) -> rusqlite::Result<Vec<Observation>> {
    let mut statement = conn.prepare(
        "SELECT scope_folder, source_id, preset, session_id, folder_path, title, updated_at, model, git_branch, last_message, message_count, last_message_role
         FROM indexed_sessions WHERE ?1 IS NULL OR scope_folder = ?1",
    )?;
    let rows = statement.query_map(params![scope], |row| {
        let Some(source) = SourceId::from_db(&row.get::<_, String>(1)?) else {
            return Ok(None);
        };
        Ok(Some(Observation {
            key: SourceKey {
                scope: row.get(0)?,
                source,
                preset: row.get(2)?,
            },
            session_id: row.get(3)?,
            folder_path: row.get(4)?,
            title: row.get(5)?,
            updated_at: row.get(6)?,
            model: row.get(7)?,
            git_branch: row.get(8)?,
            last_message: match (
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(11)?,
            ) {
                (Some(text), Some(role)) => match role.as_str() {
                    "user" => Some(LastMessage {
                        role: MessageRole::User,
                        text,
                    }),
                    "agent" => Some(LastMessage {
                        role: MessageRole::Agent,
                        text,
                    }),
                    _ => None,
                },
                _ => None,
            },
            message_count: row.get(10)?,
        }))
    })?;
    rows.filter_map(|row| row.transpose()).collect()
}

fn source_folders(conn: &Connection, key: &SourceKey) -> rusqlite::Result<BTreeSet<String>> {
    let mut statement = conn.prepare(
        "SELECT DISTINCT folder_path FROM indexed_sessions
         WHERE scope_folder = ?1 AND source_id = ?2 AND preset = ?3",
    )?;
    let rows = statement.query_map(params![key.scope, key.source.db(), key.preset], |row| {
        row.get(0)
    })?;
    rows.collect()
}

pub(crate) fn configured_keys(folder: Option<&Path>) -> Result<Vec<SourceKey>, ApiError> {
    let catalog = ExtensionService::current();
    let resolver = SettingsResolver::load(&catalog.settings_ids).map_err(ApiError::Internal)?;
    Ok(keys_for(&catalog, &resolver, folder))
}

fn keys_for(
    catalog: &LoadedExtensionCatalog,
    resolver: &SettingsResolver<'_>,
    folder: Option<&Path>,
) -> Vec<SourceKey> {
    let settings = resolver.resolve_folder_path(folder).settings;
    let mut keys = Vec::new();
    for harness in &catalog.harnesses {
        let id = &harness.definition.id;
        if harness.definition.history.is_none() {
            continue;
        }
        let config = settings.harnesses.get(id);
        if config.is_some_and(|config| !config.enabled) {
            continue;
        }
        keys.push(SourceKey::new(folder, SourceId::Harness(id.clone()), None));
        for preset in config.into_iter().flat_map(|config| config.presets.keys()) {
            keys.push(SourceKey::new(
                folder,
                SourceId::Harness(id.clone()),
                Some(preset),
            ));
        }
    }
    if folder.is_some() {
        keys.push(SourceKey::new(folder, SourceId::Axo, None));
    }
    keys
}

pub(crate) fn build_sources(
    configured: &[SourceKey],
    rows: &[SourceRow],
    refreshing: &HashSet<SourceKey>,
) -> Vec<SessionIndexSource> {
    let rows: HashMap<&SourceKey, &SourceRow> = rows.iter().map(|row| (&row.key, row)).collect();
    configured
        .iter()
        .map(|key| {
            let row = rows.get(key);
            SessionIndexSource {
                source: match &key.source {
                    SourceId::Harness(id) => SessionSourceKey::Harness {
                        harness_id: id.clone(),
                        preset: key.preset().map(str::to_owned),
                    },
                    SourceId::Axo => SessionSourceKey::Axo,
                },
                status: row.map_or(SessionIndexStatus::Pending, |row| row.status.clone()),
                refreshing: refreshing.contains(key),
                refreshed_at: row.and_then(|row| row.refreshed_at.clone()),
                message: row.and_then(|row| row.message.clone()),
            }
        })
        .collect()
}

pub(crate) fn build_snapshot(
    configured: &[SourceKey],
    rows: &[SourceRow],
    observations: &[Observation],
    refreshing: &HashSet<SourceKey>,
) -> SessionIndexSnapshot {
    let current: HashSet<&SourceKey> = configured.iter().collect();
    let mut groups: BTreeMap<(&SourceId, &str), Vec<&Observation>> = BTreeMap::new();
    for observation in observations {
        if current.contains(&observation.key) {
            groups
                .entry((&observation.key.source, &observation.session_id))
                .or_default()
                .push(observation);
        }
    }
    let mut sessions: Vec<IndexedSession> = groups
        .into_iter()
        .map(|((source, session_id), group)| {
            let best = group
                .iter()
                .copied()
                .min_by(|a, b| {
                    b.updated_at
                        .cmp(&a.updated_at)
                        .then_with(|| a.key.preset.cmp(&b.key.preset))
                })
                .expect("group is nonempty");
            let presets: BTreeSet<&str> = group.iter().map(|row| row.key.preset.as_str()).collect();
            IndexedSession {
                source: match source {
                    SourceId::Harness(id) => SessionSource::Harness {
                        harness_id: id.clone(),
                        presets: presets
                            .into_iter()
                            .map(|preset| (!preset.is_empty()).then(|| preset.to_owned()))
                            .collect(),
                    },
                    SourceId::Axo => SessionSource::Axo,
                },
                session_id: session_id.to_owned(),
                title: best.title.clone(),
                last_message: best.last_message.clone(),
                message_count: best.message_count,
                updated_at: best.updated_at.clone(),
                model: best.model.clone(),
                git_branch: best.git_branch.clone(),
            }
        })
        .collect();
    sessions.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| session_source_order(&a.source).cmp(&session_source_order(&b.source)))
            .then_with(|| a.session_id.cmp(&b.session_id))
    });
    SessionIndexSnapshot {
        sessions,
        sources: build_sources(configured, rows, refreshing),
    }
}

fn session_source_order(source: &SessionSource) -> (&str, &str) {
    match source {
        SessionSource::Harness { harness_id, .. } => ("harness:", harness_id),
        SessionSource::Axo => ("axo", ""),
    }
}

struct Job {
    dirty: bool,
    done: watch::Sender<bool>,
    abort: AbortHandle,
}

#[derive(Default)]
struct Jobs {
    closed: bool,
    active: HashMap<SourceKey, Job>,
}

pub(crate) struct SessionIndexService {
    this: Weak<Self>,
    db: Arc<DatabaseService>,
    events: EventSink<HostEvent>,
    axo: Arc<AxoService>,
    runtime: OnceLock<Handle>,
    permits: Semaphore,
    jobs: Mutex<Jobs>,
    watcher: SessionWatcher,
}

impl SessionIndexService {
    pub(crate) fn new(
        db: Arc<DatabaseService>,
        events: EventSink<HostEvent>,
        axo: Arc<AxoService>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            db,
            events,
            axo,
            runtime: OnceLock::new(),
            permits: Semaphore::new(2),
            jobs: Mutex::new(Jobs::default()),
            watcher: SessionWatcher::new(this.clone()),
        })
    }

    fn runtime(&self) -> Option<&Handle> {
        if let Some(handle) = self.runtime.get() {
            return Some(handle);
        }
        let handle = Handle::try_current().ok()?;
        Some(self.runtime.get_or_init(|| handle))
    }

    pub(crate) fn bind_runtime(&self) {
        let _ = self.runtime();
    }

    fn emit(&self, folders: impl IntoIterator<Item = String>) {
        if self.jobs.lock().closed {
            return;
        }
        for folder in folders {
            deliver(&self.events, HostEvent::SessionsChanged(folder));
        }
    }

    pub(crate) fn shutdown(&self) {
        self.watcher.shutdown();
        let jobs = {
            let mut jobs = self.jobs.lock();
            jobs.closed = true;
            std::mem::take(&mut jobs.active)
        };
        for job in jobs.into_values() {
            job.abort.abort();
        }
    }

    /// A watched store changed. No start announcement: background passes must not flash
    /// the cached-history state; the finished pass announces itself.
    pub(crate) fn refresh_watched(&self, key: SourceKey) {
        self.schedule_key(key);
    }

    pub(crate) fn schedule(&self, folder: &Path, source: SourceId, preset: Option<&str>) {
        if let Some((_, true)) = self.schedule_key(SourceKey::new(Some(folder), source, preset)) {
            self.emit([folder.to_string_lossy().into_owned()]);
        }
    }

    fn schedule_key(&self, key: SourceKey) -> Option<(watch::Receiver<bool>, bool)> {
        let Some(handle) = self.runtime() else {
            tracing::warn!(?key, "session index refresh requested outside any runtime");
            return None;
        };
        let mut jobs = self.jobs.lock();
        if jobs.closed {
            return None;
        }
        if let Some(job) = jobs.active.get_mut(&key) {
            job.dirty = true;
            return Some((job.done.subscribe(), false));
        }
        let this = self.this.upgrade()?;
        let (done, receiver) = watch::channel(false);
        let task = handle.spawn(this.drain(key.clone()));
        jobs.active.insert(
            key,
            Job {
                dirty: false,
                done,
                abort: task.abort_handle(),
            },
        );
        Some((receiver, true))
    }

    fn refreshing(&self) -> HashSet<SourceKey> {
        self.jobs.lock().active.keys().cloned().collect()
    }

    /// Folders whose snapshots show `key`; a finished pass must reach them so `refreshing` clears.
    fn key_folders(&self, key: &SourceKey) -> BTreeSet<String> {
        if key.folder().is_some() {
            return BTreeSet::from([key.scope.clone()]);
        }
        source_folders(self.db.read().conn(), key).unwrap_or_else(|error| {
            tracing::warn!(?key, %error, "session index folders unavailable");
            BTreeSet::new()
        })
    }

    async fn drain(self: Arc<Self>, key: SourceKey) {
        loop {
            match self.jobs.lock().active.get_mut(&key) {
                Some(job) => job.dirty = false,
                None => return,
            }
            let Ok(permit) = self.permits.acquire().await else {
                return;
            };
            let this = Arc::clone(&self);
            let source = key.clone();
            let affected = match tokio::task::spawn_blocking(move || this.pass(&source)).await {
                Ok(Ok(folders)) => folders,
                Ok(Err(error)) => {
                    tracing::warn!(?key, %error, "session index refresh failed");
                    self.key_folders(&key)
                }
                Err(error) => {
                    tracing::warn!(?key, %error, "session index refresh failed");
                    self.key_folders(&key)
                }
            };
            drop(permit);
            let again = {
                let mut jobs = self.jobs.lock();
                if jobs.closed {
                    return;
                }
                let Some(job) = jobs.active.get(&key) else {
                    return;
                };
                let again = job.dirty;
                if !again {
                    if let Some(job) = jobs.active.remove(&key) {
                        job.done.send_replace(true);
                    }
                }
                again
            };
            self.emit(affected);
            if !again {
                return;
            }
        }
    }

    fn pass(&self, key: &SourceKey) -> Result<BTreeSet<String>, ApiError> {
        if self.jobs.lock().closed {
            return Ok(BTreeSet::new());
        }
        let result = match &key.source {
            SourceId::Harness(id) => {
                match resolve_harness_launch(&self.db, key.folder(), id, key.preset()) {
                    Err(error) => Err((SessionIndexStatus::Missing, error.to_string())),
                    Ok(launch) => match HistorySource::from_launch(&launch) {
                        Ok(Some(source)) => source
                            .list(key.folder(), None)
                            .map_err(|message| (SessionIndexStatus::Error, message)),
                        Ok(None) => Err((
                            SessionIndexStatus::Missing,
                            format!("Harness {id} has no history source"),
                        )),
                        Err(message) => Err((SessionIndexStatus::Missing, message)),
                    },
                }
            }
            SourceId::Axo => self
                .axo
                .list_history(key.folder().expect("axo sources are folder-scoped"))
                .map_err(|message| (SessionIndexStatus::Error, message)),
        };
        // A failed pass keeps its last good watch; an unresolvable source watches nothing.
        let (result, watch) = match result {
            Ok(mut listing) => {
                let watch = std::mem::take(&mut listing.watch);
                (Ok(listing), Some(watch))
            }
            Err((SessionIndexStatus::Missing, message)) => (
                Err((SessionIndexStatus::Missing, message)),
                Some(Vec::new()),
            ),
            Err(failure) => (Err(failure), None),
        };
        // Watch before committing so a watch failure reaches this source's message.
        let unwatched = watch.and_then(|watch| self.watcher.set(key, watch));
        let affected = {
            // Serialize the closed check with shutdown so a late blocking worker cannot write.
            let jobs = self.jobs.lock();
            if jobs.closed {
                return Ok(BTreeSet::new());
            }
            let db = self.db.write();
            let mut affected = if key.folder().is_some() {
                BTreeSet::from([key.scope.clone()])
            } else {
                source_folders(db.conn(), key)?
            };
            match result {
                Ok(listing) => {
                    let message = match (listing.message(), unwatched) {
                        (Some(skipped), Some(unwatched)) => Some(format!("{skipped}; {unwatched}")),
                        (skipped, unwatched) => skipped.or(unwatched),
                    };
                    commit_listing(
                        db.conn(),
                        key,
                        &Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
                        &listing.rows,
                        message.as_deref(),
                    )?
                }
                Err((status, message)) => record_failure(db.conn(), key, status, &message)?,
            }
            if key.folder().is_none() {
                affected.extend(source_folders(db.conn(), key)?);
            }
            affected
        };
        Ok(affected)
    }

    fn folder_snapshot_for(
        &self,
        folder: &Path,
        configured: &[SourceKey],
    ) -> Result<SessionIndexSnapshot, ApiError> {
        let scope = folder.to_string_lossy();
        let (rows, observations) = {
            let db = self.db.read();
            (
                load_sources(db.conn(), Some(&scope))?,
                load_observations(db.conn(), Some(&scope))?,
            )
        };
        Ok(build_snapshot(
            configured,
            &rows,
            &observations,
            &self.refreshing(),
        ))
    }

    pub(crate) fn folder_snapshot(&self, folder: &Path) -> Result<SessionIndexSnapshot, ApiError> {
        self.folder_snapshot_for(folder, &configured_keys(Some(folder))?)
    }

    pub(crate) fn refresh_folder(&self, folder: &Path) -> Result<SessionIndexSnapshot, ApiError> {
        let configured = configured_keys(Some(folder))?;
        let mut started = false;
        for key in &configured {
            started |= self
                .schedule_key(key.clone())
                .is_some_and(|(_, started)| started);
        }
        if started {
            self.emit([folder.to_string_lossy().into_owned()]);
        }
        self.folder_snapshot_for(folder, &configured)
    }

    pub(crate) fn has_global_rows(&self) -> Result<bool, ApiError> {
        Ok(self.db.read().conn().query_row(
            "SELECT EXISTS(SELECT 1 FROM session_index_sources WHERE scope_folder = '')",
            [],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn activity_map(&self) -> Result<Vec<DiscoveredProject>, ApiError> {
        let catalog = ExtensionService::current();
        let resolver = SettingsResolver::load(&catalog.settings_ids).map_err(ApiError::Internal)?;
        let mut enabled: HashMap<String, HashSet<SourceKey>> = HashMap::new();
        enabled.insert(
            String::new(),
            keys_for(&catalog, &resolver, None).into_iter().collect(),
        );
        let mut observations = load_observations(self.db.read().conn(), None)?;
        observations.retain(|observation| {
            if !matches!(&observation.key.source, SourceId::Harness(_)) {
                return false;
            }
            let scope = &observation.key.scope;
            if !enabled.contains_key(scope) {
                let keys = keys_for(&catalog, &resolver, Some(Path::new(scope)));
                enabled.insert(scope.clone(), keys.into_iter().collect());
            }
            enabled[scope].contains(&observation.key)
        });
        Ok(activity_map::aggregate(&observations))
    }

    pub(crate) fn refresh_global(&self) -> Result<(), ApiError> {
        let configured = configured_keys(None)?;
        let mut receivers = Vec::new();
        let mut started = Vec::new();
        for key in configured {
            if let Some((receiver, new)) = self.schedule_key(key.clone()) {
                receivers.push(receiver);
                if new {
                    started.push(key);
                }
            }
        }
        let mut folders = BTreeSet::new();
        {
            let db = self.db.read();
            for key in &started {
                folders.extend(source_folders(db.conn(), key)?);
            }
        }
        self.emit(folders);
        let Some(handle) = self.runtime() else {
            return Err(ApiError::Internal(
                "session index has no runtime to await the global refresh".to_owned(),
            ));
        };
        handle.block_on(async {
            for mut receiver in receivers {
                let _ = receiver.wait_for(|done| *done).await;
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct TempDb(PathBuf);
    impl Drop for TempDb {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn database() -> (TempDb, DatabaseService) {
        let dir = std::env::temp_dir().join(format!("sworm-index-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let db = DatabaseService::new(dir.join("test.db")).unwrap();
        (TempDb(dir), db)
    }

    const FOLDER: &str = "/work/project";
    fn key(preset: &str) -> SourceKey {
        SourceKey::new(
            Some(Path::new(FOLDER)),
            SourceId::Harness("omp".into()),
            Some(preset).filter(|p| !p.is_empty()),
        )
    }
    fn listed(id: &str, updated_at: Option<&str>) -> HistoryRow {
        HistoryRow {
            id: id.into(),
            cwd: Some(FOLDER.into()),
            title: Some(format!("title {id}")),
            last_message: Some(LastMessage {
                role: MessageRole::Agent,
                text: format!("last {id}"),
            }),
            message_count: Some(4),
            updated_at: updated_at.map(Into::into),
            model: Some("model".into()),
            git_branch: Some("main".into()),
            ..Default::default()
        }
    }
    fn snapshot(db: &DatabaseService, configured: &[SourceKey]) -> SessionIndexSnapshot {
        let db = db.read();
        build_snapshot(
            configured,
            &load_sources(db.conn(), Some(FOLDER)).unwrap(),
            &load_observations(db.conn(), Some(FOLDER)).unwrap(),
            &HashSet::new(),
        )
    }

    #[test]
    fn shared_id_survives_removal_from_one_source() {
        let (_dir, db) = database();
        let conn = db.write();
        let shared = listed("shared", Some("2026-10-01T00:00:00.000Z"));
        commit_listing(conn.conn(), &key(""), "t1", &[shared.clone()], None).unwrap();
        commit_listing(conn.conn(), &key("work"), "t1", &[shared], None).unwrap();
        commit_listing(conn.conn(), &key(""), "t2", &[], None).unwrap();
        drop(conn);
        let snapshot = snapshot(&db, &[key(""), key("work")]);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].session_id, "shared");
        assert_eq!(
            snapshot.sessions[0].source,
            SessionSource::Harness {
                harness_id: "omp".into(),
                presets: vec![Some("work".to_owned())],
            }
        );
        assert_eq!(snapshot.sessions[0].model.as_deref(), Some("model"));
        assert_eq!(
            snapshot.sessions[0].last_message,
            Some(LastMessage {
                role: MessageRole::Agent,
                text: "last shared".into(),
            })
        );
        assert_eq!(snapshot.sessions[0].message_count, Some(4));
        assert_eq!(snapshot.sessions[0].git_branch.as_deref(), Some("main"));
    }

    #[test]
    fn failed_refresh_keeps_rows_and_date() {
        let (_dir, db) = database();
        let conn = db.write();
        commit_listing(conn.conn(), &key(""), "t1", &[listed("one", None)], None).unwrap();
        record_failure(
            conn.conn(),
            &key(""),
            SessionIndexStatus::Error,
            "listing failed",
        )
        .unwrap();
        drop(conn);
        let snapshot = snapshot(&db, &[key("")]);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].session_id, "one");
        assert_eq!(
            snapshot.sessions[0].source,
            SessionSource::Harness {
                harness_id: "omp".into(),
                presets: vec![None],
            }
        );
        assert_eq!(snapshot.sessions[0].updated_at, None);
        assert_eq!(snapshot.sources[0].refreshed_at.as_deref(), Some("t1"));
        assert_eq!(snapshot.sources[0].status, SessionIndexStatus::Error);
        assert_eq!(
            snapshot.sources[0].message.as_deref(),
            Some("listing failed")
        );
    }

    #[test]
    fn empty_success_clears_only_its_source() {
        let (_dir, db) = database();
        let conn = db.write();
        commit_listing(conn.conn(), &key(""), "t1", &[listed("one", None)], None).unwrap();
        commit_listing(
            conn.conn(),
            &key("work"),
            "t1",
            &[listed("two", None)],
            None,
        )
        .unwrap();
        commit_listing(conn.conn(), &key(""), "t2", &[], None).unwrap();
        drop(conn);
        let snapshot = snapshot(&db, &[key(""), key("work")]);
        assert_eq!(snapshot.sessions.len(), 1);
        assert_eq!(snapshot.sessions[0].session_id, "two");
    }

    #[test]
    fn removed_sources_are_excluded() {
        let (_dir, db) = database();
        commit_listing(
            db.write().conn(),
            &key("gone"),
            "t1",
            &[listed("one", None)],
            None,
        )
        .unwrap();
        let snapshot = snapshot(&db, &[key("")]);
        assert!(snapshot.sessions.is_empty());
        assert_eq!(snapshot.sources.len(), 1);
        assert_eq!(snapshot.sources[0].status, SessionIndexStatus::Pending);
    }

    #[test]
    fn axo_and_harness_sources_stay_apart() {
        let (_dir, db) = database();
        let db = Arc::new(db);
        let axo_key = SourceKey::new(Some(Path::new(FOLDER)), SourceId::Axo, None);
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        {
            let conn = db.write();
            let row = listed("shared", Some(&now));
            commit_listing(conn.conn(), &key(""), &now, &[row.clone()], None).unwrap();
            commit_listing(conn.conn(), &axo_key, &now, &[row], None).unwrap();
        }
        let snapshot = snapshot(&db, &[key(""), axo_key]);
        assert_eq!(snapshot.sessions.len(), 2);
        assert_eq!(snapshot.sessions[0].source, SessionSource::Axo);
        assert_eq!(
            snapshot.sessions[1].source,
            SessionSource::Harness {
                harness_id: "omp".into(),
                presets: vec![None],
            }
        );
        assert!(snapshot
            .sessions
            .iter()
            .all(|session| session.session_id == "shared"));
        let axo = AxoService::new(Arc::clone(&db));
        let service = SessionIndexService::new(db, Arc::new(|_| Ok(())), axo);
        let projects = service.activity_map().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].harnesses.len(), 1);
        assert_eq!(projects[0].harnesses[0].harness_id, "omp");
        assert_eq!(projects[0].harnesses[0].daily_counts.iter().sum::<u32>(), 1);
    }

    #[tokio::test]
    async fn unresolvable_source_records_missing_and_announces_completion() {
        let (_dir, db) = database();
        let db = Arc::new(db);
        let (sender, receiver) = std::sync::mpsc::channel();
        let events: EventSink<HostEvent> =
            Arc::new(move |event| sender.send(event).map_err(|error| error.to_string()));
        let axo = AxoService::new(Arc::clone(&db));
        let service = SessionIndexService::new(Arc::clone(&db), events, axo);
        let (mut done, started) = service
            .schedule_key(SourceKey::new(
                Some(Path::new(FOLDER)),
                SourceId::Harness("no-such-harness".into()),
                None,
            ))
            .unwrap();
        assert!(started);
        done.wait_for(|done| *done).await.unwrap();
        assert!(matches!(
            receiver.recv_timeout(std::time::Duration::from_secs(1)),
            Ok(HostEvent::SessionsChanged(folder)) if folder == FOLDER
        ));
        let sources = load_sources(db.read().conn(), Some(FOLDER)).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].status, SessionIndexStatus::Missing);
    }
}
