//! Bind fresh terminal runs to the sessions their history source creates.

use super::{floor_to, HistoryRow, HistorySource};
use crate::services::pty::PtyEventSink;
use chrono::DateTime;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use axome_protocol::pty::PtyEvent;
use tracing::{debug, error, info};

const BINDING_POLL: Duration = Duration::from_secs(1);
const CLAIM_OVERLAP: Duration = Duration::from_secs(1);

pub(crate) struct PendingRun {
    pub run_id: String,
    pub source: Arc<HistorySource>,
    pub preset: Option<String>,
    pub folder: PathBuf,
    pub spawned_at: SystemTime,
    pub event_sink: PtyEventSink,
    pub on_bound: Box<dyn FnOnce(String) + Send>,
}

struct PendingSnapshot {
    run_id: String,
    spawned_at: SystemTime,
}

#[derive(PartialEq, Eq, Hash)]
struct GroupKey {
    harness_id: String,
    preset: Option<String>,
    folder: PathBuf,
}

struct Group {
    source: Arc<HistorySource>,
    runs: Vec<PendingSnapshot>,
}

struct Inner {
    pending: Mutex<Vec<PendingRun>>,
    claimed: Mutex<HashMap<String, SystemTime>>,
    // Lock order: worker before pending, so tracking cannot lose a wake-up
    // while the worker decides whether to exit.
    worker: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone)]
pub(crate) struct HistoryBinder {
    inner: Arc<Inner>,
}

impl HistoryBinder {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                pending: Mutex::new(Vec::new()),
                claimed: Mutex::new(HashMap::new()),
                worker: Mutex::new(None),
            }),
        }
    }

    pub(crate) fn claim(&self, token: &str) {
        let now = SystemTime::now();
        let earliest_pending = self
            .inner
            .pending
            .lock()
            .iter()
            .map(|run| run.spawned_at)
            .min()
            .unwrap_or(now);
        let mut claimed = self.inner.claimed.lock();
        prune_claimed(&mut claimed, earliest_pending);
        claimed.insert(token.to_string(), now);
    }

    pub(crate) fn track(&self, run: PendingRun) {
        let mut worker = self.inner.worker.lock();
        self.inner.pending.lock().push(run);
        if worker.is_some() {
            return;
        }
        let inner = Arc::clone(&self.inner);
        match std::thread::Builder::new()
            .name("history-binding".to_string())
            .spawn(move || worker_loop(inner))
        {
            Ok(handle) => *worker = Some(handle),
            Err(error) => error!("Failed to spawn history binding worker: {error}"),
        }
    }

    pub(crate) fn cancel(&self, run_id: &str) {
        self.inner.pending.lock().retain(|run| run.run_id != run_id);
    }

    #[cfg(test)]
    pub(crate) fn has_pending(&self, run_id: &str) -> bool {
        self.inner
            .pending
            .lock()
            .iter()
            .any(|run| run.run_id == run_id)
    }
}

fn worker_loop(inner: Arc<Inner>) {
    let mut tick = 0;
    loop {
        std::thread::sleep(BINDING_POLL);
        tick = (tick + 1) % 5;
        {
            let mut worker = inner.worker.lock();
            if inner.pending.lock().is_empty() {
                prune_claimed(&mut inner.claimed.lock(), SystemTime::now());
                *worker = None;
                return;
            }
        }

        let mut groups: HashMap<GroupKey, Group> = HashMap::new();
        let mut earliest_pending = SystemTime::now();
        for run in inner.pending.lock().iter() {
            earliest_pending = earliest_pending.min(run.spawned_at);
            let key = GroupKey {
                harness_id: run.source.harness_id.clone(),
                preset: run.preset.clone(),
                folder: run.folder.clone(),
            };
            groups
                .entry(key)
                .or_insert_with(|| Group {
                    source: Arc::clone(&run.source),
                    runs: Vec::new(),
                })
                .runs
                .push(PendingSnapshot {
                    run_id: run.run_id.clone(),
                    spawned_at: run.spawned_at,
                });
        }
        prune_claimed(&mut inner.claimed.lock(), earliest_pending);
        let mut groups: Vec<_> = groups.into_iter().collect();
        for (_, group) in &mut groups {
            group.runs.sort_by_key(|run| run.spawned_at);
        }
        groups.sort_by_key(|(_, group)| group.runs.first().map(|run| run.spawned_at));
        let now = SystemTime::now();
        let due: Vec<(GroupKey, Group)> = groups
            .into_iter()
            .filter(|(_, group)| {
                tick == 0
                    || group.runs.iter().any(|run| {
                        now.duration_since(run.spawned_at).unwrap_or_default()
                            < Duration::from_secs(30)
                    })
            })
            .collect();
        // Sources are independent; listing them together bounds a tick by the slowest
        // source instead of their sum. Attribution below stays serialized in spawn order
        // because presets of one harness share a store and must claim earliest runs first.
        let listed: Vec<Result<Vec<HistoryRow>, String>> = std::thread::scope(|scope| {
            let handles: Vec<_> = due
                .iter()
                .map(|(key, group)| {
                    let since = group.runs.first().map(|run| run.spawned_at);
                    scope.spawn(move || {
                        group
                            .source
                            .list(Some(&key.folder), since)
                            .map(|listing| listing.rows)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .unwrap_or_else(|_| Err("history listing panicked".to_owned()))
                })
                .collect()
        });
        for ((key, group), rows) in due.into_iter().zip(listed) {
            let rows = match rows {
                Ok(rows) => rows,
                Err(error) => {
                    debug!(
                        "{} history binding failed for {}: {error}",
                        key.harness_id,
                        key.folder.display()
                    );
                    continue;
                }
            };
            let bindings = {
                let mut claimed = inner.claimed.lock();
                let precision = group.source.created_precision();
                let bindings = attribute(rows, group.runs, &claimed, precision);
                let now = SystemTime::now();
                for (_, token) in &bindings {
                    claimed.insert(token.clone(), now);
                }
                bindings
            };
            for (run_id, token) in bindings {
                // Claim before removing the run, even if it was cancelled,
                // so its session never migrates to a sibling.
                let run = {
                    let mut pending = inner.pending.lock();
                    pending
                        .iter()
                        .position(|run| run.run_id == run_id)
                        .map(|index| pending.remove(index))
                };
                let Some(run) = run else { continue };
                info!(
                    "Bound {} conversation {token} to run {run_id}",
                    key.harness_id
                );
                (run.on_bound)(token.clone());
                run.event_sink.emit(PtyEvent::SessionIdBound {
                    run_id,
                    session_id: token,
                });
            }
        }
    }
}

fn attribute(
    rows: Vec<HistoryRow>,
    mut runs: Vec<PendingSnapshot>,
    claimed: &HashMap<String, SystemTime>,
    precision: Duration,
) -> Vec<(String, String)> {
    let mut rows: Vec<_> = rows
        .into_iter()
        .filter_map(|row| {
            let created_at = DateTime::parse_from_rfc3339(row.created_at.as_deref()?).ok()?;
            Some((SystemTime::from(created_at), row.id))
        })
        .collect();
    rows.sort_by_key(|(created_at, _)| *created_at);
    runs.sort_by_key(|run| run.spawned_at);
    let mut bindings = Vec::new();
    for (created_at, id) in rows {
        if claimed.contains_key(&id) {
            continue;
        }
        if let Some(index) = runs
            .iter()
            .position(|run| created_at >= floor_to(run.spawned_at, precision))
        {
            bindings.push((runs.remove(index).run_id, id));
        }
        if runs.is_empty() {
            break;
        }
    }
    bindings
}

fn prune_claimed(claimed: &mut HashMap<String, SystemTime>, earliest_pending: SystemTime) {
    let cutoff = earliest_pending
        .checked_sub(CLAIM_OVERLAP)
        .unwrap_or(UNIX_EPOCH);
    claimed.retain(|_, claimed_at| *claimed_at >= cutoff);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: &str, millis: u64) -> PendingSnapshot {
        PendingSnapshot {
            run_id: id.into(),
            spawned_at: UNIX_EPOCH + Duration::from_millis(millis),
        }
    }

    fn row(id: &str, created_at: Option<&str>) -> HistoryRow {
        HistoryRow {
            id: id.into(),
            created_at: created_at.map(str::to_string),
            ..HistoryRow::default()
        }
    }

    #[test]
    fn sessions_before_spawn_second_are_not_attributed() {
        let bindings = attribute(
            vec![
                row("old", Some("1970-01-01T00:00:09.999Z")),
                row("unknown", None),
                row("current", Some("1970-01-01T00:00:10.000Z")),
            ],
            vec![run("run", 10_999)],
            &HashMap::new(),
            Duration::from_secs(1),
        );
        assert_eq!(bindings, [("run".into(), "current".into())]);
    }

    #[test]
    fn sessions_bind_in_spawn_order() {
        let bindings = attribute(
            vec![
                row("last", Some("1970-01-01T00:00:21.000Z")),
                row("claimed", Some("1970-01-01T00:00:10.000Z")),
                row("first", Some("1970-01-01T00:00:11.000Z")),
                row("gap", Some("1970-01-01T00:00:12.000Z")),
            ],
            vec![run("late", 20_500), run("early", 10_500)],
            &HashMap::from([("claimed".into(), UNIX_EPOCH)]),
            Duration::from_secs(1),
        );
        assert_eq!(
            bindings,
            [
                ("early".into(), "first".into()),
                ("late".into(), "last".into())
            ]
        );
    }

    #[test]
    fn millisecond_sources_ignore_sessions_earlier_in_spawn_second() {
        let bindings = attribute(
            vec![
                row("external", Some("1970-01-01T00:00:10.200Z")),
                row("own", Some("1970-01-01T00:00:10.900Z")),
            ],
            vec![run("run", 10_900)],
            &HashMap::new(),
            Duration::from_millis(1),
        );
        assert_eq!(bindings, [("run".into(), "own".into())]);
    }
}
