//! Re-lists history sources when the stores their last pass reported change on disk.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use notify::event::ModifyKind;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;

use crate::services::session_index::{SessionIndexService, SourceKey};
use crate::services::settings_watcher::existing_watch_parent;

/// Agents append to their stores every turn; one pass per window per source keeps up.
const FOLDER_INTERVAL: Duration = Duration::from_secs(2);
/// A global pass reads every store, and only the Home view shows it.
const GLOBAL_INTERVAL: Duration = Duration::from_secs(30);
/// Settle time before a burst's first pass.
const TICK: Duration = Duration::from_millis(500);

pub(crate) struct SessionWatcher {
    index: Weak<SessionIndexService>,
    // Only `set` locks this. notify's `watch` waits on the event thread that runs the
    // callback, so the callback must never wait on this lock.
    os: Mutex<Os>,
    shared: Arc<Mutex<Shared>>,
}

#[derive(Default)]
struct Os {
    watcher: Option<RecommendedWatcher>,
    /// Registered directories by how many source paths use them.
    targets: HashMap<PathBuf, usize>,
    /// The directories each source's paths registered.
    registrations: HashMap<SourceKey, Vec<PathBuf>>,
}

#[derive(Default)]
struct Shared {
    closed: bool,
    /// Source paths by the registered directory that reports their changes, so an event
    /// only checks its own path and its parent.
    // ponytail: sources stay watched for the app run once listed; drop them on folder release if that ever costs
    watched: HashMap<PathBuf, Vec<(SourceKey, PathBuf)>>,
    /// Registered directories created, removed or renamed since they were watched: the OS
    /// watch no longer follows the path, so the next `set` that lists one watches it again.
    lost: HashSet<PathBuf>,
    dirty: HashSet<SourceKey>,
    last_pass: HashMap<SourceKey, Instant>,
    flushing: bool,
}

impl SessionWatcher {
    pub(crate) fn new(index: Weak<SessionIndexService>) -> Self {
        Self {
            index,
            os: Mutex::default(),
            shared: Arc::default(),
        }
    }

    /// Replace `key`'s watched paths with what its latest pass reported. `Some` says why
    /// part of it can't be watched live; the next listing retries.
    pub(crate) fn set(&self, key: &SourceKey, paths: Vec<PathBuf>) -> Option<String> {
        let mut os = self.os.lock();
        if self.shared.lock().closed {
            return None;
        }
        if os.watcher.is_none() {
            if paths.is_empty() {
                return None;
            }
            match self.os_watcher() {
                Ok(watcher) => os.watcher = Some(watcher),
                Err(error) => {
                    tracing::warn!(%error, "session store watcher unavailable");
                    return Some(format!("Live updates unavailable: {error}"));
                }
            }
        }
        let Os {
            watcher,
            targets,
            registrations,
        } = &mut *os;
        let watcher = watcher.as_mut().expect("watcher created above");
        let released = registrations.remove(key).unwrap_or_default();
        for target in &released {
            release(watcher, targets, target);
        }
        let mut unwatched = None;
        let mut registered = Vec::with_capacity(paths.len());
        let mut watched = Vec::with_capacity(paths.len());
        for path in paths {
            let Some(target) = (if path.is_dir() {
                Some(path.clone())
            } else {
                existing_watch_parent(&path)
            }) else {
                continue;
            };
            let count = targets.entry(target.clone()).or_default();
            // Never hold `shared` across `watch`: notify runs the callback that locks it on
            // the thread `watch` waits for.
            let lost = self.shared.lock().lost.remove(&target);
            if *count == 0 || lost {
                if let Err(error) = watcher.watch(&target, RecursiveMode::NonRecursive) {
                    tracing::debug!(?key, target = %target.display(), %error, "session store unwatchable");
                    unwatched.get_or_insert_with(|| format!("Live updates unavailable: {error}"));
                    if *count == 0 {
                        targets.remove(&target);
                        continue;
                    }
                    // Other sources still count this directory; keep it lost so a later
                    // listing retries.
                    self.shared.lock().lost.insert(target.clone());
                }
            }
            *count += 1;
            registered.push(target);
            watched.push(path);
        }
        let mut shared = self.shared.lock();
        for target in &released {
            if let Some(sources) = shared.watched.get_mut(target) {
                sources.retain(|(owner, _)| owner != key);
                if sources.is_empty() {
                    shared.watched.remove(target);
                }
            }
        }
        for (target, path) in registered.iter().zip(watched) {
            shared
                .watched
                .entry(target.clone())
                .or_default()
                .push((key.clone(), path));
        }
        drop(shared);
        if !registered.is_empty() {
            registrations.insert(key.clone(), registered);
        }
        unwatched
    }

    pub(crate) fn shutdown(&self) {
        let mut os = self.os.lock();
        *self.shared.lock() = Shared {
            closed: true,
            ..Shared::default()
        };
        *os = Os::default();
    }

    fn os_watcher(&self) -> notify::Result<RecommendedWatcher> {
        let shared = Arc::downgrade(&self.shared);
        let index = self.index.clone();
        notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let (Ok(event), Some(state)) = (event, shared.upgrade()) else {
                return;
            };
            let mut guard = state.lock();
            let Shared {
                closed,
                watched,
                lost,
                dirty,
                flushing,
                ..
            } = &mut *guard;
            if *closed {
                return;
            }
            let moved = matches!(
                event.kind,
                EventKind::Create(_)
                    | EventKind::Remove(_)
                    | EventKind::Modify(ModifyKind::Name(_))
            );
            for changed in &event.paths {
                if moved && watched.contains_key(changed) {
                    lost.insert(changed.clone());
                }
                // A non-recursive watch reports its directory and that directory's direct children.
                for directory in std::iter::once(changed.as_path()).chain(changed.parent()) {
                    for (key, path) in watched.get(directory).into_iter().flatten() {
                        if !dirty.contains(key) && affects(changed, path) {
                            dirty.insert(key.clone());
                        }
                    }
                }
            }
            if !dirty.is_empty() && !*flushing {
                *flushing = true;
                let (shared, index) = (shared.clone(), index.clone());
                std::thread::spawn(move || flush(shared, index));
            }
        })
    }
}

/// A change to the path itself, a direct child of it, or an ancestor still on its way.
fn affects(changed: &Path, watched: &Path) -> bool {
    changed == watched || changed.parent() == Some(watched) || watched.starts_with(changed)
}

fn release(watcher: &mut RecommendedWatcher, targets: &mut HashMap<PathBuf, usize>, target: &Path) {
    let Some(count) = targets.get_mut(target) else {
        return;
    };
    *count -= 1;
    if *count == 0 {
        targets.remove(target);
        // A deleted directory has already dropped its OS watch.
        let _ = watcher.unwatch(target);
    }
}

fn flush(shared: Weak<Mutex<Shared>>, index: Weak<SessionIndexService>) {
    loop {
        std::thread::sleep(TICK);
        let (Some(state), Some(index)) = (shared.upgrade(), index.upgrade()) else {
            return;
        };
        let (due, finished) = {
            let mut guard = state.lock();
            let Shared {
                closed,
                dirty,
                last_pass,
                flushing,
                ..
            } = &mut *guard;
            if *closed {
                return;
            }
            let now = Instant::now();
            let due: Vec<SourceKey> = dirty
                .iter()
                .filter(|key| {
                    let interval = if key.scope.is_empty() {
                        GLOBAL_INTERVAL
                    } else {
                        FOLDER_INTERVAL
                    };
                    last_pass
                        .get(*key)
                        .is_none_or(|last| now.duration_since(*last) >= interval)
                })
                .cloned()
                .collect();
            for key in &due {
                dirty.remove(key);
                last_pass.insert(key.clone(), now);
            }
            *flushing = !dirty.is_empty();
            (due, !*flushing)
        };
        for key in due {
            index.refresh_watched(key);
        }
        if finished {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{affects, SessionWatcher};
    use crate::services::session_index::{SourceId, SourceKey};
    use std::fs;
    use std::path::Path;
    use std::sync::Weak;
    use std::time::{Duration, Instant};

    fn wait_until(condition: &str, mut done: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !done() {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {condition}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn recreated_shared_directory_is_watched_again() {
        let root =
            std::env::temp_dir().join(format!("sworm-session-watcher-{}", uuid::Uuid::new_v4()));
        let bucket = root.join("bucket");
        fs::create_dir_all(&bucket).unwrap();
        // A dead index makes `flush` return before it clears `dirty`.
        let watcher = SessionWatcher::new(Weak::new());
        let folder = SourceKey::new(Some(Path::new("/repo")), SourceId::Axo, None);
        let global = SourceKey::new(None, SourceId::Axo, None);
        assert_eq!(watcher.set(&folder, vec![bucket.clone()]), None);
        assert_eq!(watcher.set(&global, vec![bucket.clone()]), None);

        fs::remove_dir(&bucket).unwrap();
        wait_until("the deleted bucket to be lost", || {
            watcher.shared.lock().lost.contains(&bucket)
        });
        fs::create_dir(&bucket).unwrap();
        // The bucket's count goes 2 -> 1 -> 2, so only `lost` re-watches it.
        assert_eq!(watcher.set(&folder, vec![bucket.clone()]), None);
        std::thread::sleep(Duration::from_millis(200));
        watcher.shared.lock().dirty.clear();

        fs::write(bucket.join("session.jsonl"), "{}\n").unwrap();
        wait_until("both sources to be dirty", || {
            let shared = watcher.shared.lock();
            shared.dirty.contains(&folder) && shared.dirty.contains(&global)
        });
        watcher.shutdown();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn changes_reach_paths_children_and_pending_ancestors() {
        let bucket = Path::new("/home/u/.omp/agent/sessions/-repo");
        assert!(affects(bucket, bucket));
        assert!(affects(&bucket.join("2026_x.jsonl"), bucket));
        assert!(affects(Path::new("/home/u/.omp/agent/sessions"), bucket));
        assert!(!affects(&bucket.join("x/artifact.md"), bucket));
        let database = Path::new("/home/u/.codex/state_5.sqlite");
        assert!(!affects(
            Path::new("/home/u/.codex/logs_2.sqlite-wal"),
            database
        ));
    }
}
