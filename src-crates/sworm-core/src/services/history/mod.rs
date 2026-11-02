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
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, SecondsFormat, Utc};
use parking_lot::Mutex;
use sworm_protocol::session::{LastMessage, MessageRole};

use crate::services::harnesses::HarnessLaunch;

/// One session as a source reports it, before or after normalization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HistoryRow {
    pub id: String,
    pub cwd: Option<String>,
    pub title: Option<String>,
    pub last_message: Option<LastMessage>,
    pub message_count: Option<u32>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub model: Option<String>,
    pub git_branch: Option<String>,
}

impl HistoryRow {
    /// Counts one user/assistant record with conversation text and makes it the last
    /// message, reusing the preview's buffer. Blank and marker blocks drop out; a record
    /// with nothing left (tool-only, meta, markers) leaves the row untouched. Rows that
    /// read a whole conversation start at `message_count: Some(0)`.
    pub(super) fn push_message<'a>(
        &mut self,
        role: MessageRole,
        texts: impl IntoIterator<Item = &'a str>,
    ) {
        let words = texts
            .into_iter()
            .filter(|text| !is_marker(text))
            .flat_map(str::split_whitespace);
        let fresh = self.last_message.is_none();
        let message = self.last_message.get_or_insert_with(|| LastMessage {
            role,
            text: String::new(),
        });
        // Append after the old preview and drain it only once new text arrived, so an
        // empty record keeps the previous preview.
        let start = message.text.len();
        if !push_words(&mut message.text, words) {
            if fresh {
                self.last_message = None;
            }
            return;
        }
        message.text.drain(..start);
        message.role = role;
        self.message_count = Some(self.message_count.unwrap_or_default().saturating_add(1));
    }
}

/// One pass over a source: its sessions, the entries it could not read, and the
/// paths whose changes can alter the next pass.
#[derive(Debug, Default)]
pub(crate) struct Listing {
    pub rows: Vec<HistoryRow>,
    /// Unreadable session files or invalid command lines; the rest still list.
    pub skipped: usize,
    /// A path, or a directory and its direct children. Missing paths are
    /// watched from their nearest existing ancestor.
    pub watch: Vec<PathBuf>,
}

impl Listing {
    /// Ready-source note for entries the pass had to leave out.
    pub(crate) fn message(&self) -> Option<String> {
        match self.skipped {
            0 => None,
            1 => Some("1 history entry couldn't be read".into()),
            count => Some(format!("{count} history entries couldn't be read")),
        }
    }
}

/// Per-entry I/O outcome: a vanished entry is no session, any other failure counts
/// as skipped so one bad file never fails its whole source.
pub(super) fn readable<T>(result: std::io::Result<T>, skipped: &mut usize) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => {
            *skipped += 1;
            None
        }
    }
}

/// Feeds each line to `visit` until it breaks; a torn or non-UTF-8 line drops only
/// that record, never the session.
pub(super) fn each_line(
    file: File,
    mut visit: impl FnMut(&str) -> ControlFlow<()>,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer)? == 0 {
            return Ok(());
        }
        if let Ok(line) = std::str::from_utf8(&buffer) {
            if visit(line).is_break() {
                return Ok(());
            }
        }
    }
}

struct Parsed {
    scanner: &'static str,
    len: u64,
    modified: SystemTime,
    row: Option<HistoryRow>,
}

/// Last parse of each session file; a rescan re-reads only files whose length or mtime moved.
// ponytail: entries for deleted files live for the process; prune on global passes if memory ever matters
static PARSED: LazyLock<Mutex<HashMap<PathBuf, Parsed>>> = LazyLock::new(Mutex::default);

/// `parse`'s result for `path`, reused while the file keeps `metadata`'s length and mtime.
/// Parsing runs outside the lock; a file that grows mid-read is re-read next pass.
pub(super) fn parsed(
    scanner: &'static str,
    path: &Path,
    metadata: &std::fs::Metadata,
    parse: impl FnOnce() -> std::io::Result<Option<HistoryRow>>,
) -> std::io::Result<Option<HistoryRow>> {
    let Ok(modified) = metadata.modified() else {
        return parse();
    };
    if let Some(hit) = PARSED.lock().get(path).filter(|hit| {
        hit.scanner == scanner && hit.len == metadata.len() && hit.modified == modified
    }) {
        return Ok(hit.row.clone());
    }
    let row = parse()?;
    PARSED.lock().insert(
        path.to_owned(),
        Parsed {
            scanner,
            len: metadata.len(),
            modified,
            row: row.clone(),
        },
    );
    Ok(row)
}

/// What a builtin scanner reads with: the launch args and env of the preset.
pub(crate) struct HistoryQuery<'a> {
    /// `None` lists every folder the store knows.
    pub folder: Option<&'a Path>,
    /// Harness `extra_args` then preset args, as the CLI would receive them.
    pub args: &'a [String],
    pub env: &'a HashMap<String, String>,
    /// Hint only, already floored to the source's creation precision: scanners may skip
    /// older sessions; normalization filters anyway.
    pub since: Option<SystemTime>,
}

pub(crate) struct BuiltinScanner {
    pub name: &'static str,
    created_precision: Duration,
    list: fn(&HistoryQuery) -> Result<Listing, String>,
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

    /// Stores record creation at varying precision; spawn-time comparisons use the source's.
    pub(crate) fn created_precision(&self) -> Duration {
        match &self.kind {
            SourceKind::Builtin(scanner) => scanner.created_precision,
            // The command protocol declares no precision; whole seconds is the safe floor.
            SourceKind::Command { .. } => Duration::from_secs(1),
        }
    }

    pub(crate) fn list(
        &self,
        folder: Option<&Path>,
        since: Option<SystemTime>,
    ) -> Result<Listing, String> {
        let since = since.map(|time| floor_to(time, self.created_precision()));
        let mut listing = match &self.kind {
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
        listing.rows = normalize(std::mem::take(&mut listing.rows), folder, since);
        Ok(listing)
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
                .rows
                .iter()
                .any(|row| row.id == id)),
        }
    }
}

/// Appends `words` single-spaced to `out`, capped at 200 characters with no trailing
/// space; whether anything was appended.
fn push_words<'a>(out: &mut String, words: impl Iterator<Item = &'a str>) -> bool {
    let start = out.len();
    out.extend(
        words
            .enumerate()
            .flat_map(|(index, word)| (index != 0).then_some(' ').into_iter().chain(word.chars()))
            .take(200),
    );
    let end = start + out[start..].trim_end().len();
    out.truncate(end);
    out.len() > start
}

pub(crate) fn normalize_title(raw: &str) -> Option<String> {
    let mut text = String::new();
    push_words(&mut text, raw.split_whitespace()).then_some(text)
}

/// Harness-injected text that is not conversation: a block wrapped whole in tags
/// (`<system-reminder>…</system-reminder>`, `<command-name>…</command-args>`), or a
/// bracketed harness notice. Real messages that only start with a tag are kept.
pub(super) fn is_marker(text: &str) -> bool {
    let text = text.trim();
    (text
        .strip_prefix('<')
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_alphabetic()))
        && text.ends_with('>'))
        || [
            "[Request interrupted",
            "[Wait interrupted",
            "[budget notice]",
        ]
        .iter()
        .any(|notice| text.starts_with(notice))
}

/// The conversation text blocks of a Claude-style `message` record; none for meta records.
pub(super) fn conversation_blocks(record: &serde_json::Value) -> impl Iterator<Item = &str> {
    let is_meta = |value: &serde_json::Value| {
        value.get("isMeta").and_then(serde_json::Value::as_bool) == Some(true)
    };
    let content = record
        .get("message")
        .filter(|message| !is_meta(record) && !is_meta(message))
        .and_then(|message| message.get("content"));
    content
        .and_then(serde_json::Value::as_str)
        .into_iter()
        .chain(
            content
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|block| {
                    (block.get("type").and_then(serde_json::Value::as_str) == Some("text"))
                        .then(|| block.get("text").and_then(serde_json::Value::as_str))
                        .flatten()
                }),
        )
}

pub(crate) fn normalize(
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
    let since = since.map(DateTime::<Utc>::from);
    let mut result = Vec::new();
    let mut seen: HashMap<String, (usize, Option<DateTime<Utc>>)> = HashMap::new();
    for mut row in rows {
        let Some(id) = trim(Some(row.id)) else {
            continue;
        };
        row.id = id;
        row.cwd = trim(row.cwd);
        row.title = row.title.as_deref().and_then(normalize_title);
        row.last_message = row.last_message.and_then(|message| {
            normalize_title(&message.text).map(|text| LastMessage { text, ..message })
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

/// Stores record creation at varying precision; flooring a spawn time to it keeps a
/// session created later in the same tick eligible.
fn floor_to(time: SystemTime, precision: Duration) -> SystemTime {
    let elapsed = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let remainder = elapsed.as_nanos() % precision.as_nanos().max(1);
    UNIX_EPOCH + elapsed - Duration::from_nanos(remainder as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::io::Write;

    #[test]
    fn markers_are_whole_tag_blocks_or_harness_notices() {
        for marker in [
            "<system-reminder>x</system-reminder>",
            " \n<command-name>/clear</command-name>\n<command-args></command-args>",
            "<environment_context>m</environment_context>",
            "[Request interrupted by user]",
            "[Wait interrupted by message]\n<irc from=\"parent\">hi</irc>",
            "[budget notice] wrap up",
        ] {
            assert!(is_marker(marker), "{marker:?} should be a marker");
        }
        for text in [
            "<svg viewBox=\"0 0 1 1\"></svg> make it blue",
            "<!DOCTYPE html><html></html>",
            "< 3 apples",
            "[Workflow harness — user request] do it",
        ] {
            assert!(!is_marker(text), "{text:?} should be conversation");
        }

        let mut row = HistoryRow {
            message_count: Some(0),
            ..Default::default()
        };
        row.push_message(MessageRole::User, ["first"]);
        row.push_message(MessageRole::Agent, ["<system-reminder>x</system-reminder>"]);
        row.push_message(MessageRole::User, ["<svg></svg>  make", " it blue "]);
        assert_eq!(row.message_count, Some(2));
        assert_eq!(
            row.last_message,
            Some(LastMessage {
                role: MessageRole::User,
                text: "<svg></svg> make it blue".into(),
            })
        );
    }

    #[test]
    fn parsed_rereads_only_changed_files() {
        let path =
            std::env::temp_dir().join(format!("sworm-parsed-{}.jsonl", uuid::Uuid::new_v4()));
        std::fs::write(&path, "one\n").unwrap();
        let parses = Cell::new(0);
        let read = |scanner: &'static str| {
            let metadata = std::fs::metadata(&path).unwrap();
            parsed(scanner, &path, &metadata, || {
                parses.set(parses.get() + 1);
                Ok(Some(HistoryRow {
                    title: Some(std::fs::read_to_string(&path)?),
                    ..HistoryRow::default()
                }))
            })
            .unwrap()
            .unwrap()
        };

        let first = read("test");
        assert_eq!(parses.get(), 1);
        assert_eq!(read("test"), first);
        assert_eq!(parses.get(), 1);

        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"two\n")
            .unwrap();
        assert!(read("test").title.unwrap().contains("two"));
        assert_eq!(parses.get(), 2);

        read("other");
        assert_eq!(parses.get(), 3);
        std::fs::remove_file(path).unwrap();
    }
}
