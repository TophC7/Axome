//! Harness session history: builtin store scanners and extension history
//! commands behind one source type. Stores are read-only; Sworm never writes
//! harness state.

mod antigravity;
pub(crate) mod binding;
mod claude;
mod codex;
mod command;
pub(crate) mod omp;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, SecondsFormat, Utc};

use crate::services::harnesses::HarnessLaunch;

/// One session as a source reports it, before or after normalization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HistoryRow {
    pub id: String,
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub model: Option<String>,
    pub git_branch: Option<String>,
}

/// What a builtin scanner reads with: the launch args and env of the preset.
pub(crate) struct HistoryQuery<'a> {
    /// `None` lists every folder the store knows.
    pub folder: Option<&'a Path>,
    /// Harness `extra_args` then preset args, as the CLI would receive them.
    pub args: &'a [String],
    pub env: &'a HashMap<String, String>,
    /// Hint only: scanners may skip older sessions; normalization filters anyway.
    pub since: Option<SystemTime>,
}

pub(crate) struct BuiltinScanner {
    pub name: &'static str,
    list: fn(&HistoryQuery) -> Result<Vec<HistoryRow>, String>,
    exists: fn(&HistoryQuery, &str) -> Result<bool, String>,
}

pub(crate) static BUILTIN_SCANNERS: [BuiltinScanner; 4] = [
    omp::SCANNER,
    claude::SCANNER,
    codex::SCANNER,
    antigravity::SCANNER,
];

pub(crate) fn builtin_scanner(name: &str) -> Option<&'static BuiltinScanner> {
    BUILTIN_SCANNERS.iter().find(|scanner| scanner.name == name)
}

pub(crate) struct HistorySource {
    pub harness_id: String,
    kind: SourceKind,
    args: Vec<String>,
    env: HashMap<String, String>,
}

enum SourceKind {
    Builtin(&'static BuiltinScanner),
    Command { program: String, argv: Vec<String> },
}

impl HistorySource {
    /// Ok(None): no `history`. Err: the history command cannot be resolved (status `missing`).
    pub(crate) fn from_launch(launch: &HarnessLaunch) -> Result<Option<Self>, String> {
        let harness = launch.harness();
        let Some(history) = &harness.definition.history else {
            return Ok(None);
        };
        let mut env = launch.env.clone();
        let kind = if let Some(name) = &history.builtin {
            SourceKind::Builtin(
                builtin_scanner(name)
                    .ok_or_else(|| format!("unknown builtin history scanner {name}"))?,
            )
        } else {
            let command = history
                .command
                .as_deref()
                .ok_or("history command is missing")?;
            env.extend(
                history
                    .env
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );
            let program = crate::services::extensions::resolve_contribution_command(
                command,
                harness.extension_dir.as_deref(),
                env.get("PATH").map(String::as_str).unwrap_or(""),
                None,
            )?;
            SourceKind::Command {
                program,
                argv: history.args.clone(),
            }
        };
        let mut args = launch.config.extra_args.clone();
        args.extend_from_slice(&launch.preset.args);
        Ok(Some(Self {
            harness_id: harness.definition.id.clone(),
            kind,
            args,
            env,
        }))
    }

    pub(crate) fn list(
        &self,
        folder: Option<&Path>,
        since: Option<SystemTime>,
    ) -> Result<Vec<HistoryRow>, String> {
        let rows = match &self.kind {
            SourceKind::Builtin(scanner) => (scanner.list)(&HistoryQuery {
                folder,
                args: &self.args,
                env: &self.env,
                since,
            })?,
            SourceKind::Command { program, argv } => {
                command::run(program, argv, &self.env, folder, &self.args, since)?
            }
        };
        Ok(normalize(rows, folder, since))
    }

    pub(crate) fn contains(&self, folder: &Path, id: &str) -> Result<bool, String> {
        match &self.kind {
            SourceKind::Builtin(scanner) => (scanner.exists)(
                &HistoryQuery {
                    folder: Some(folder),
                    args: &self.args,
                    env: &self.env,
                    since: None,
                },
                id,
            ),
            SourceKind::Command { .. } => Ok(self
                .list(Some(folder), None)?
                .iter()
                .any(|row| row.id == id)),
        }
    }
}

fn normalize(
    rows: Vec<HistoryRow>,
    folder: Option<&Path>,
    since: Option<SystemTime>,
) -> Vec<HistoryRow> {
    fn trim(value: Option<String>) -> Option<String> {
        value.and_then(|mut value| {
            let end = value.trim_end().len();
            value.truncate(end);
            let start = value.len() - value.trim_start().len();
            value.drain(..start);
            (!value.is_empty()).then_some(value)
        })
    }

    fn timestamp(value: Option<String>) -> Option<DateTime<Utc>> {
        value
            .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
            .map(|value| value.with_timezone(&Utc))
    }

    fn without_trailing_slash(value: &str) -> &str {
        value.strip_suffix('/').unwrap_or(value)
    }

    let folder = folder.map(Path::to_string_lossy);
    let since = since.map(|time| DateTime::<Utc>::from(floor_to_second(time)));
    let mut result = Vec::new();
    let mut seen: HashMap<String, (usize, Option<DateTime<Utc>>)> = HashMap::new();
    for mut row in rows {
        let Some(id) = trim(Some(row.id)) else {
            continue;
        };
        row.id = id;
        row.cwd = trim(row.cwd);
        row.title = trim(row.title).map(|title| {
            title
                .split_whitespace()
                .enumerate()
                .flat_map(|(index, word)| {
                    (index != 0).then_some(' ').into_iter().chain(word.chars())
                })
                .take(200)
                .collect()
        });
        row.model = trim(row.model);
        row.git_branch = trim(row.git_branch);
        let created_at = timestamp(trim(row.created_at));
        let updated_at = timestamp(trim(row.updated_at));
        row.created_at = created_at.map(|time| time.to_rfc3339_opts(SecondsFormat::Millis, true));
        row.updated_at = updated_at.map(|time| time.to_rfc3339_opts(SecondsFormat::Millis, true));
        if let Some(folder) = &folder {
            if row
                .cwd
                .as_deref()
                .is_some_and(|cwd| without_trailing_slash(cwd) != without_trailing_slash(folder))
            {
                continue;
            }
            row.cwd = Some(folder.to_string());
        } else if !row
            .cwd
            .as_deref()
            .is_some_and(|cwd| Path::new(cwd).is_absolute())
        {
            continue;
        }
        if since.is_some_and(|since| created_at.is_none_or(|created| created < since)) {
            continue;
        }
        if let Some((index, previous_update)) = seen.get_mut(&row.id) {
            if updated_at > *previous_update {
                result[*index] = row;
                *previous_update = updated_at;
            }
        } else {
            seen.insert(row.id.clone(), (result.len(), updated_at));
            result.push(row);
        }
    }
    result
}

/// Home the stores live under: the launch env's non-empty `HOME`, else the process `HOME`.
pub(crate) fn store_home(env: &HashMap<String, String>) -> Option<PathBuf> {
    env.get("HOME")
        .map(|home| home.trim())
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
        .or_else(super::folders::home_dir)
}

/// Stores record creation with varying precision; compare at whole seconds.
pub(crate) fn floor_to_second(time: SystemTime) -> SystemTime {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    UNIX_EPOCH + Duration::from_secs(seconds)
}
