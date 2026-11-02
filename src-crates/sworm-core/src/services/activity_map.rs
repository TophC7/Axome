//! Home activity aggregated from cached session-index observations.

use crate::services::{
    folders::folder_name,
    session_index::{Observation, SourceId},
};
use chrono::{DateTime, Local, TimeZone};
use std::{
    collections::{BTreeMap, HashMap},
    path::Path,
};
use sworm_protocol::activity_map::{DiscoveredHarnessActivity, DiscoveredProject};

const MAX_RESULTS: usize = 50;
const WINDOW_DAYS: usize = 7;

#[derive(Default)]
struct Tally<'a> {
    last: &'a str,
    daily: [u32; WINDOW_DAYS],
}

fn rank(observation: &Observation) -> (bool, Option<&str>) {
    (
        observation.key.scope == observation.folder_path,
        observation.updated_at.as_deref(),
    )
}

pub(crate) fn aggregate(observations: &[Observation]) -> Vec<DiscoveredProject> {
    let mut sessions: HashMap<(&str, &str, &str), &Observation> = HashMap::new();
    for observation in observations {
        let SourceId::Harness(harness_id) = &observation.key.source else {
            continue;
        };
        if observation.updated_at.is_none() {
            continue;
        }
        sessions
            .entry((
                observation.folder_path.as_str(),
                harness_id.as_str(),
                observation.session_id.as_str(),
            ))
            .and_modify(|best| {
                if rank(observation) > rank(best) {
                    *best = observation;
                }
            })
            .or_insert(observation);
    }
    let day_starts = day_boundaries();
    let mut projects: BTreeMap<&str, BTreeMap<&str, Tally>> = BTreeMap::new();
    for ((folder, harness, _), observation) in sessions {
        let Some(updated_at) = observation.updated_at.as_deref() else {
            continue;
        };
        let Ok(time) = DateTime::parse_from_rfc3339(updated_at) else {
            continue;
        };
        let tally = projects
            .entry(folder)
            .or_default()
            .entry(harness)
            .or_default();
        tally.last = tally.last.max(updated_at);
        if let Some(day) = bucket_timestamp(time.timestamp(), &day_starts) {
            tally.daily[day] += 1;
        }
    }
    let mut results: Vec<DiscoveredProject> = projects
        .into_iter()
        .map(|(path, harnesses)| {
            let harnesses: Vec<DiscoveredHarnessActivity> = harnesses
                .into_iter()
                .map(|(harness_id, tally)| DiscoveredHarnessActivity {
                    harness_id: harness_id.to_owned(),
                    last_active: tally.last.to_owned(),
                    daily_counts: tally.daily,
                })
                .collect();
            DiscoveredProject {
                path: path.to_owned(),
                name: folder_name(Path::new(path)),
                path_exists: Path::new(path).is_dir(),
                last_active: harnesses
                    .iter()
                    .map(|harness| harness.last_active.as_str())
                    .max()
                    .unwrap_or_default()
                    .to_owned(),
                harnesses,
            }
        })
        .collect();
    results.sort_by(|a, b| b.last_active.cmp(&a.last_active));
    results.truncate(MAX_RESULTS);
    results
}

/// day_starts[0] is six days ago and day_starts[6] is today, in the local timezone.
fn day_boundaries() -> [i64; WINDOW_DAYS] {
    let today = Local::now().date_naive();
    let mut starts = [0i64; WINDOW_DAYS];
    for (i, start) in starts.iter_mut().enumerate() {
        let date = today - chrono::Duration::days((WINDOW_DAYS - 1 - i) as i64);
        if let Some(dt) = date.and_hms_opt(0, 0, 0) {
            if let Some(local) = Local.from_local_datetime(&dt).earliest() {
                *start = local.timestamp();
            }
        }
    }
    starts
}

fn bucket_timestamp(ts: i64, day_starts: &[i64; WINDOW_DAYS]) -> Option<usize> {
    day_starts.iter().rposition(|&start| ts >= start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::session_index::SourceKey;
    use chrono::{SecondsFormat, Utc};

    fn observation(
        scope: Option<&str>,
        preset: Option<&str>,
        folder: &str,
        id: &str,
        updated_at: Option<&str>,
    ) -> Observation {
        Observation {
            key: SourceKey::new(
                scope.map(Path::new),
                SourceId::Harness("omp".into()),
                preset,
            ),
            session_id: id.into(),
            folder_path: folder.into(),
            title: None,
            last_message: None,
            message_count: None,
            updated_at: updated_at.map(Into::into),
            model: None,
            git_branch: None,
        }
    }

    #[test]
    fn bucket_timestamp_edges() {
        let starts = day_boundaries();
        assert_eq!(bucket_timestamp(0, &starts), None);
        assert_eq!(bucket_timestamp(starts[6], &starts), Some(6));
    }

    #[test]
    fn duplicate_observations_count_once() {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let project = "/work/project";
        let projects = aggregate(&[
            observation(Some(project), None, project, "a", Some(&now)),
            observation(Some(project), Some("work"), project, "a", Some(&now)),
            observation(None, None, project, "a", Some(&now)),
            observation(None, Some("work"), project, "a", Some(&now)),
            observation(Some(project), None, project, "undated", None),
            observation(None, None, "/work/other", "b", None),
        ]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, project);
        assert_eq!(projects[0].last_active, now);
        assert_eq!(projects[0].harnesses[0].daily_counts.iter().sum::<u32>(), 1);
    }
}
