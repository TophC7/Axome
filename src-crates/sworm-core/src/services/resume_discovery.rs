//! Post-spawn session-id discovery for harnesses that mint their own
//! conversation identity after launch (Codex, Antigravity, OMP).
//!
//! Attribution is causal: only artifacts created at or after the spawn
//! instant are eligible, there is no lookback and no deadline. A run
//! stays pending until it is bound or cancelled.
//!
//! Attribution rule: a conversation is attributed to the earliest-spawned
//! eligible unbound run in the scanner's store scope. Codex and OMP stores
//! are cwd-scoped; Antigravity's conversation store is global.

use crate::services::codex_state::CodexStateReader;
use crate::services::harness_scanners::{antigravity_visit_conversations_created_since, Scanner};
use crate::services::omp;
use crate::services::pty::PtyEventSink;
use parking_lot::Mutex;
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sworm_protocol::pty::PtyEvent;
use tracing::{error, info, warn};

const DISCOVERY_POLL: Duration = Duration::from_secs(1);
const CLAIM_OVERLAP: Duration = Duration::from_secs(1);

/// A spawned run whose harness-side conversation is not yet known.
pub struct PendingRun {
    pub run_id: String,
    pub(crate) scanner: Scanner,
    pub cwd: String,
    /// Taken immediately before the PTY spawn so nothing the process
    /// creates can predate it.
    pub spawned_at: SystemTime,
    pub event_sink: PtyEventSink,
    /// Keep the Host's current session id independent of any attached stream.
    pub on_bound: Box<dyn FnOnce(String) + Send>,
}

#[derive(Clone)]
struct PendingSnapshot {
    run_id: String,
    scanner: Scanner,
    cwd: String,
    spawned_at: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DiscoveryGroupKey {
    scanner: Scanner,
    /// Antigravity has one global store; Codex and OMP are cwd-scoped.
    cwd: Option<String>,
}

struct DiscoveryGroup {
    key: DiscoveryGroupKey,
    runs: Vec<PendingSnapshot>,
}

struct Inner {
    /// Insertion order preserves tracking order; groups sort by the
    /// pre-spawn timestamp before attribution.
    pending: Mutex<Vec<PendingRun>>,
    /// Tokens bound or resumed recently enough to overlap pending runs.
    claimed: Mutex<HashMap<String, SystemTime>>,
    /// Lock order: `worker` before `pending`. Held while deciding whether
    /// the worker must (re)start or may exit, so a `track` racing an
    /// exiting worker never loses its wake-up.
    worker: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone)]
pub struct ResumeDiscoveryService {
    inner: Arc<Inner>,
}

impl ResumeDiscoveryService {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                pending: Mutex::new(Vec::new()),
                claimed: Mutex::new(HashMap::new()),
                worker: Mutex::new(None),
            }),
        }
    }

    /// Record a token this process already resumed so discovery never
    /// re-attributes it.
    pub fn claim(&self, token: &str) {
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

    /// Start watching for the conversation `run_id` will create. Spawns
    /// the worker thread on first use.
    pub fn track(&self, run: PendingRun) {
        let mut worker = self.inner.worker.lock();
        self.inner.pending.lock().push(run);
        if worker.is_some() {
            return;
        }
        let inner = Arc::clone(&self.inner);
        match std::thread::Builder::new()
            .name("resume-discovery".to_string())
            .spawn(move || worker_loop(inner))
        {
            Ok(handle) => *worker = Some(handle),
            Err(err) => error!("Failed to spawn resume discovery worker: {err}"),
        }
    }

    /// Stop watching (PTY exited or was killed). No-op when unknown.
    pub fn cancel(&self, run_id: &str) {
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
    loop {
        std::thread::sleep(DISCOVERY_POLL);

        {
            let mut worker = inner.worker.lock();
            if inner.pending.lock().is_empty() {
                prune_claimed(&mut inner.claimed.lock(), SystemTime::now());
                *worker = None;
                return;
            }
        }

        let snapshot: Vec<PendingSnapshot> = inner
            .pending
            .lock()
            .iter()
            .map(|run| PendingSnapshot {
                run_id: run.run_id.clone(),
                scanner: run.scanner,
                cwd: run.cwd.clone(),
                spawned_at: run.spawned_at,
            })
            .collect();
        let Some(earliest_pending) = snapshot.iter().map(|run| run.spawned_at).min() else {
            continue;
        };
        let claimed_snapshot: HashSet<String> = {
            let mut claimed = inner.claimed.lock();
            prune_claimed(&mut claimed, earliest_pending);
            claimed.keys().cloned().collect()
        };

        for mut group in group_pending(snapshot) {
            let scanner = group.key.scanner;
            let cwd = group.key.cwd.as_deref().unwrap_or_default();
            let since = group
                .runs
                .first()
                .map(|run| run.spawned_at)
                .unwrap_or(earliest_pending);
            let mut bindings = Vec::new();
            scan_created_since(scanner, cwd, since, |created_at, token| {
                if claimed_snapshot.contains(&token) {
                    return true;
                }
                let Some(index) = eligible_run_index(&group.runs, scanner, created_at) else {
                    return true;
                };
                let reserved = {
                    let mut claimed = inner.claimed.lock();
                    match claimed.entry(token.clone()) {
                        Entry::Vacant(entry) => {
                            entry.insert(SystemTime::now());
                            true
                        }
                        Entry::Occupied(_) => false,
                    }
                };
                if !reserved {
                    return true;
                }
                let run = group.runs.remove(index);
                bindings.push((run.run_id, token));
                !group.runs.is_empty()
            });

            for (run_id, token) in bindings {
                // Claim before removing the run: a conversation this run
                // created must never migrate to a sibling even if the run
                // was cancelled meanwhile.
                let run = {
                    let mut pending = inner.pending.lock();
                    pending
                        .iter()
                        .position(|run| run.run_id == run_id)
                        .map(|index| pending.remove(index))
                };
                let Some(run) = run else {
                    continue;
                };
                info!("Bound {scanner:?} conversation {token} to run {run_id}");
                (run.on_bound)(token.clone());
                run.event_sink.emit(PtyEvent::SessionIdBound {
                    run_id,
                    session_id: token,
                });
            }
        }
    }
}

fn group_pending(snapshot: Vec<PendingSnapshot>) -> Vec<DiscoveryGroup> {
    let mut grouped: HashMap<DiscoveryGroupKey, Vec<PendingSnapshot>> = HashMap::new();
    for run in snapshot {
        let key = DiscoveryGroupKey {
            scanner: run.scanner,
            cwd: (run.scanner != Scanner::Antigravity).then(|| run.cwd.clone()),
        };
        grouped.entry(key).or_default().push(run);
    }
    let mut groups: Vec<DiscoveryGroup> = grouped
        .into_iter()
        .map(|(key, mut runs)| {
            runs.sort_by_key(|run| run.spawned_at);
            DiscoveryGroup { key, runs }
        })
        .collect();
    groups.sort_by_key(|group| group.runs.first().map(|run| run.spawned_at));
    groups
}

fn eligible_run_index(
    runs: &[PendingSnapshot],
    scanner: Scanner,
    created_at: SystemTime,
) -> Option<usize> {
    runs.iter()
        .position(|run| created_at_or_after(scanner, created_at, run.spawned_at))
}

fn created_at_or_after(scanner: Scanner, created_at: SystemTime, since: SystemTime) -> bool {
    match (
        created_at.duration_since(UNIX_EPOCH),
        since.duration_since(UNIX_EPOCH),
    ) {
        (Ok(created), Ok(since)) => match scanner {
            Scanner::Codex => created.as_secs() >= since.as_secs(),
            Scanner::Omp => created.as_millis() >= since.as_millis(),
            _ => created >= since,
        },
        _ => created_at >= since,
    }
}

fn prune_claimed(claimed: &mut HashMap<String, SystemTime>, earliest_pending: SystemTime) {
    let cutoff = earliest_pending
        .checked_sub(CLAIM_OVERLAP)
        .unwrap_or(UNIX_EPOCH);
    claimed.retain(|_, claimed_at| *claimed_at >= cutoff);
}

fn scan_created_since(
    scanner: Scanner,
    cwd: &str,
    since: SystemTime,
    mut visit: impl FnMut(SystemTime, String) -> bool,
) {
    match scanner {
        Scanner::Codex => {
            // `threads.created_at` is whole seconds; flooring `since`
            // keeps a thread created later in the spawn second eligible.
            let Some(since_unix) = since
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|duration| i64::try_from(duration.as_secs()).ok())
            else {
                return;
            };
            if let Err(err) = CodexStateReader::visit_threads_created_since(
                cwd,
                since_unix,
                |token, created_at| {
                    let Some(created_at) = u64::try_from(created_at)
                        .ok()
                        .and_then(|seconds| UNIX_EPOCH.checked_add(Duration::from_secs(seconds)))
                    else {
                        return true;
                    };
                    visit(created_at, token)
                },
            ) {
                warn!("Codex discovery failed for {cwd}: {err}");
            }
        }
        Scanner::Antigravity => {
            antigravity_visit_conversations_created_since(since, &mut visit);
        }
        Scanner::Omp => {
            omp::visit_sessions_created_since(cwd, since, &mut visit);
        }
        Scanner::ClaudeCode => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(run_id: &str, scanner: Scanner, cwd: &str, millis: u64) -> PendingSnapshot {
        PendingSnapshot {
            run_id: run_id.to_string(),
            scanner,
            cwd: cwd.to_string(),
            spawned_at: UNIX_EPOCH + Duration::from_millis(millis),
        }
    }

    #[test]
    fn groups_cwd_scoped_stores_but_not_antigravity() {
        let groups = group_pending(vec![
            snapshot("c1", Scanner::Codex, "/a", 1),
            snapshot("c2", Scanner::Codex, "/a", 2),
            snapshot("c3", Scanner::Codex, "/b", 3),
            snapshot("a1", Scanner::Antigravity, "/a", 4),
            snapshot("a2", Scanner::Antigravity, "/b", 5),
        ]);

        assert_eq!(
            groups
                .iter()
                .find(|group| {
                    group.key.scanner == Scanner::Codex && group.key.cwd.as_deref() == Some("/a")
                })
                .map(|group| group.runs.len()),
            Some(2)
        );
        assert_eq!(
            groups
                .iter()
                .find(|group| group.key.scanner == Scanner::Antigravity)
                .map(|group| group.runs.len()),
            Some(2)
        );
    }

    #[test]
    fn scanner_timestamp_precision_matches_store() {
        let candidate = UNIX_EPOCH + Duration::from_millis(10_000);
        let later_in_second = UNIX_EPOCH + Duration::from_millis(10_999);

        assert!(created_at_or_after(
            Scanner::Codex,
            candidate,
            later_in_second
        ));
        assert!(!created_at_or_after(
            Scanner::Antigravity,
            candidate,
            later_in_second
        ));
    }

    #[test]
    fn candidate_skips_runs_spawned_after_it() {
        let mut runs = vec![
            snapshot("early", Scanner::Codex, "/a", 10_000),
            snapshot("late", Scanner::Codex, "/a", 100_000),
        ];
        let first = UNIX_EPOCH + Duration::from_millis(11_000);
        let gap = UNIX_EPOCH + Duration::from_millis(12_000);
        let last = UNIX_EPOCH + Duration::from_millis(101_000);

        assert_eq!(eligible_run_index(&runs, Scanner::Codex, first), Some(0));
        runs.remove(0);
        assert_eq!(eligible_run_index(&runs, Scanner::Codex, gap), None);
        assert_eq!(eligible_run_index(&runs, Scanner::Codex, last), Some(0));
    }

    #[test]
    fn prunes_claims_outside_pending_overlap() {
        let mut claimed = HashMap::from([
            ("old".to_string(), UNIX_EPOCH + Duration::from_secs(10)),
            ("recent".to_string(), UNIX_EPOCH + Duration::from_secs(20)),
        ]);

        prune_claimed(&mut claimed, UNIX_EPOCH + Duration::from_secs(20));

        assert_eq!(claimed.keys().cloned().collect::<Vec<_>>(), ["recent"]);
    }
}
