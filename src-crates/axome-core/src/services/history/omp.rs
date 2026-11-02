use super::{readable, store_home, BuiltinScanner, HistoryQuery, HistoryRow, Listing};
use crate::services::folders::home_dir;
use chrono::{DateTime, NaiveDateTime, Utc};
use std::collections::HashMap;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use axome_protocol::session::MessageRole;

pub(super) const SCANNER: BuiltinScanner = BuiltinScanner {
    name: "omp",
    // File-name timestamps carry milliseconds.
    created_precision: std::time::Duration::from_millis(1),
    list,
    exists,
};

/// Timestamp layout of OMP session file names; fixed width, so
/// lexicographic order equals chronological order.
pub(crate) const OMP_TS_FORMAT: &str = "%Y-%m-%dT%H-%M-%S-%3fZ";

/// Bucket directory name for `cwd`.
///
/// Folders under `$HOME` encode as `-` + the home-relative path with `/`
/// mapped to `-` (`~/Development/axome` → `-Development-axome`, `$HOME`
/// itself → `-`). Folders outside `$HOME` encode as `--` + the absolute
/// path without its leading `/` and `/` mapped to `-`, followed by `--`
/// (`/repo/Nix/omp.nix` → `--repo-Nix-omp.nix--`). Dots pass through.
fn bucket_name(cwd: &str, home: &Path) -> String {
    match Path::new(cwd).strip_prefix(home) {
        Ok(relative) => {
            let relative = relative.to_string_lossy();
            format!("-{}", relative.replace('/', "-"))
        }
        Err(_) => format!("--{}--", cwd.trim_start_matches('/').replace('/', "-")),
    }
}

/// `~/.omp/agent/sessions/<bucket>` for `cwd`; `None` when `$HOME` is unset.
pub(crate) fn session_bucket(cwd: &str) -> Option<PathBuf> {
    let home = home_dir()?;
    let bucket = bucket_name(cwd, &home);
    Some(
        home.join(".omp")
            .join("agent")
            .join("sessions")
            .join(bucket),
    )
}

/// Split an OMP session file name into `(ts, id)`; `None` for anything
/// that is not `<ts>_<id>.jsonl`.
pub(crate) fn parse_session_file_name(name: &str) -> Option<(&str, &str)> {
    let stem = name.strip_suffix(".jsonl")?;
    stem.split_once('_')
}

#[derive(Debug, PartialEq, Eq)]
enum SessionsRoot {
    Buckets(PathBuf),
    Flat(PathBuf),
}

fn last_arg<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    let mut result = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == flag {
            if let Some(value) = args.next() {
                result = Some(value.as_str());
            }
        } else if let Some(value) = arg.strip_prefix(flag).and_then(|s| s.strip_prefix('=')) {
            result = Some(value);
        }
    }
    result
}

fn sessions_root(
    args: &[String],
    env: &HashMap<String, String>,
    folder: Option<&Path>,
) -> Option<SessionsRoot> {
    // ponytail: mirrors omp dirs.ts for profile, PI_* and XDG; other overrides read the default store
    if let Some(dir) = last_arg(args, "--session-dir") {
        let dir = PathBuf::from(dir);
        return Some(SessionsRoot::Flat(if dir.is_absolute() {
            dir
        } else {
            folder?.join(dir)
        }));
    }
    let profile = last_arg(args, "--profile")
        .or_else(|| env.get("OMP_PROFILE").map(String::as_str))
        .or_else(|| env.get("PI_PROFILE").map(String::as_str))
        .filter(|p| !p.is_empty());
    let home = store_home(env)?;
    let mut root = home.join(
        env.get("PI_CONFIG_DIR")
            .filter(|s| !s.is_empty())
            .map_or(".omp", String::as_str),
    );
    if let Some(profile) = profile {
        root = root.join("profiles").join(profile);
    }
    let override_dir = env.get("PI_CODING_AGENT_DIR").filter(|s| !s.is_empty());
    let mut agent = match (profile, override_dir) {
        (None, Some(dir)) => PathBuf::from(dir),
        _ => root.join("agent"),
    };
    if profile.is_some() || override_dir.is_none() {
        if let Some(xdg) = env.get("XDG_DATA_HOME").filter(|s| !s.is_empty()) {
            let mut xdg = Path::new(xdg).join("omp");
            if let Some(profile) = profile {
                xdg = xdg.join("profiles").join(profile);
            }
            if xdg.is_dir() {
                agent = xdg;
            }
        }
    }
    Some(SessionsRoot::Buckets(agent.join("sessions")))
}

fn read_dir(path: &Path) -> Result<Option<std::fs::ReadDir>, String> {
    match std::fs::read_dir(path) {
        Ok(entries) => Ok(Some(entries)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// Session directories to read, plus the bucket root when every bucket is listed.
struct SessionDirs {
    dirs: Vec<PathBuf>,
    buckets_root: Option<PathBuf>,
}

fn session_dirs(query: &HistoryQuery<'_>, skipped: &mut usize) -> Result<SessionDirs, String> {
    let mut found = SessionDirs {
        dirs: Vec::new(),
        buckets_root: None,
    };
    let Some(root) = sessions_root(query.args, query.env, query.folder) else {
        return Ok(found);
    };
    match root {
        SessionsRoot::Flat(dir) => found.dirs.push(dir),
        SessionsRoot::Buckets(root) => {
            if let Some(folder) = query.folder {
                if let Some(home) = store_home(query.env) {
                    found
                        .dirs
                        .push(root.join(bucket_name(&folder.to_string_lossy(), &home)));
                }
                return Ok(found);
            }
            if let Some(entries) = read_dir(&root)? {
                for entry in entries {
                    let Some(entry) = readable(entry, skipped) else {
                        continue;
                    };
                    if readable(entry.file_type(), skipped).is_some_and(|kind| kind.is_dir()) {
                        found.dirs.push(entry.path());
                    }
                }
            }
            found.buckets_root = Some(root);
        }
    }
    Ok(found)
}

fn list(query: &HistoryQuery<'_>) -> Result<Listing, String> {
    let since = query.since.map(DateTime::<Utc>::from);
    let mut listing = Listing::default();
    let SessionDirs { dirs, buckets_root } = session_dirs(query, &mut listing.skipped)?;
    let listing_all = buckets_root.is_some();
    for dir in &dirs {
        let entries = match read_dir(dir) {
            Ok(Some(entries)) => entries,
            Ok(None) => continue,
            // One unreadable bucket costs only its sessions; a folder's own bucket fails the source.
            Err(_) if listing_all => {
                listing.skipped += 1;
                continue;
            }
            Err(error) => return Err(error),
        };
        for entry in entries {
            let Some(entry) = readable(entry, &mut listing.skipped) else {
                continue;
            };
            let name = entry.file_name();
            let Some((timestamp, id)) = name.to_str().and_then(parse_session_file_name) else {
                continue;
            };
            let Ok(created) = NaiveDateTime::parse_from_str(timestamp, OMP_TS_FORMAT) else {
                continue;
            };
            let created = created.and_utc();
            if since.is_some_and(|since| created < since) {
                continue;
            }
            let path = entry.path();
            let Some(metadata) = readable(std::fs::metadata(&path), &mut listing.skipped) else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let Some(Some(mut row)) = readable(
                super::parsed(SCANNER.name, &path, &metadata, || {
                    let mut row = HistoryRow {
                        id: id.to_owned(),
                        created_at: Some(created.to_rfc3339()),
                        ..HistoryRow::default()
                    };
                    read_session(std::fs::File::open(&path)?, &mut row)?;
                    Ok(Some(row))
                }),
                &mut listing.skipped,
            ) else {
                continue;
            };
            row.updated_at = metadata
                .modified()
                .ok()
                .map(|mtime| DateTime::<Utc>::from(mtime).to_rfc3339());
            listing.rows.push(row);
        }
    }
    listing.watch = dirs;
    listing.watch.extend(buckets_root);
    Ok(listing)
}

fn read_session(file: std::fs::File, row: &mut HistoryRow) -> io::Result<()> {
    row.message_count = Some(0);
    super::each_line(file, |line| {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            return ControlFlow::Continue(());
        };
        match value.get("type").and_then(serde_json::Value::as_str) {
            Some("title") => {
                row.title = value
                    .get("title")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
            }
            Some("session") => {
                if let Some(id) = value.get("id").and_then(serde_json::Value::as_str) {
                    row.id = id.to_owned();
                }
                row.cwd = value
                    .get("cwd")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned);
                if let Some(timestamp) = value.get("timestamp").and_then(serde_json::Value::as_str)
                {
                    row.created_at = Some(timestamp.to_owned());
                }
            }
            Some("message") => {
                let Some(message) = value.get("message") else {
                    return ControlFlow::Continue(());
                };
                let role = message.get("role").and_then(serde_json::Value::as_str);
                // Message records with text count; toolResult, meta, thinking/tool-only and markers don't.
                match role {
                    Some("assistant") => {
                        row.model = message
                            .get("model")
                            .and_then(serde_json::Value::as_str)
                            .map(|model| {
                                match message.get("provider").and_then(serde_json::Value::as_str) {
                                    Some(provider) if !provider.is_empty() => {
                                        format!("{provider}/{model}")
                                    }
                                    _ => model.to_owned(),
                                }
                            });
                        row.push_message(MessageRole::Agent, super::conversation_blocks(&value));
                    }
                    Some("user") => {
                        row.push_message(MessageRole::User, super::conversation_blocks(&value));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    })
}

fn exists(query: &HistoryQuery<'_>, id: &str) -> Result<bool, String> {
    for dir in session_dirs(query, &mut 0)?.dirs {
        let Some(entries) = read_dir(&dir)? else {
            continue;
        };
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry
                .file_name()
                .to_str()
                .and_then(parse_session_file_name)
                .is_some_and(|(_, file_id)| file_id == id)
                && entry
                    .file_type()
                    .map_err(|error| error.to_string())?
                    .is_file()
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    use axome_protocol::session::LastMessage;

    // Bucket names observed in a real ~/.omp/agent/sessions/.
    #[test]
    fn bucket_under_home() {
        let home = Path::new("/home/toph");
        assert_eq!(
            bucket_name("/home/toph/Development/axome", home),
            "-Development-axome"
        );
        assert_eq!(bucket_name("/home/toph", home), "-");
    }

    #[test]
    fn bucket_outside_home() {
        let home = Path::new("/home/toph");
        assert_eq!(
            bucket_name("/repo/Nix/omp.nix", home),
            "--repo-Nix-omp.nix--"
        );
    }

    #[test]
    fn session_file_name_splits_ts_and_id() {
        assert_eq!(
            parse_session_file_name(
                "2026-08-30T04-15-34-605Z_01a050e1-40cd-7104-abfb-204121a81217.jsonl"
            ),
            Some((
                "2026-08-30T04-15-34-605Z",
                "01a050e1-40cd-7104-abfb-204121a81217"
            ))
        );
        // Sibling directories share the stem but carry no extension.
        assert_eq!(
            parse_session_file_name(
                "2026-08-30T04-15-34-605Z_01a050e1-40cd-7104-abfb-204121a81217"
            ),
            None
        );
    }

    #[test]
    fn since_format_matches_file_name_layout() {
        let since = SystemTime::UNIX_EPOCH + std::time::Duration::from_millis(1_787_000_000_123);
        assert_eq!(
            DateTime::<Utc>::from(since)
                .format(OMP_TS_FORMAT)
                .to_string(),
            "2026-08-17T20-53-20-123Z"
        );
    }

    #[test]
    fn sessions_root_precedence() {
        let home = std::env::temp_dir().join(format!("axome-omp-roots-{}", uuid::Uuid::new_v4()));
        let mut env = HashMap::from([
            ("HOME".to_owned(), home.to_string_lossy().into_owned()),
            ("OMP_PROFILE".to_owned(), "env-profile".to_owned()),
            ("PI_PROFILE".to_owned(), "legacy".to_owned()),
            (
                "PI_CODING_AGENT_DIR".to_owned(),
                home.join("override").to_string_lossy().into_owned(),
            ),
        ]);
        assert_eq!(
            sessions_root(
                &["--profile=first".into(), "--profile".into(), "cli".into()],
                &env,
                None
            ),
            Some(SessionsRoot::Buckets(
                home.join(".omp/profiles/cli/agent/sessions")
            ))
        );
        assert_eq!(
            sessions_root(&[], &env, None),
            Some(SessionsRoot::Buckets(
                home.join(".omp/profiles/env-profile/agent/sessions")
            ))
        );
        env.insert("OMP_PROFILE".into(), String::new());
        assert_eq!(
            sessions_root(&[], &env, None),
            Some(SessionsRoot::Buckets(home.join("override/sessions")))
        );
        env.remove("PI_CODING_AGENT_DIR");
        assert_eq!(
            sessions_root(&[], &env, None),
            Some(SessionsRoot::Buckets(home.join(".omp/agent/sessions")))
        );
        let xdg = home.join("data");
        std::fs::create_dir_all(xdg.join("omp")).unwrap();
        env.insert("XDG_DATA_HOME".into(), xdg.to_string_lossy().into_owned());
        assert_eq!(
            sessions_root(&[], &env, None),
            Some(SessionsRoot::Buckets(xdg.join("omp/sessions")))
        );
        let args = [
            "--session-dir=old".into(),
            "--session-dir".into(),
            "flat".into(),
        ];
        assert_eq!(
            sessions_root(&args, &env, Some(&home)),
            Some(SessionsRoot::Flat(home.join("flat")))
        );
        assert_eq!(sessions_root(&args, &env, None), None);
        assert_eq!(
            sessions_root(&[format!("--session-dir={}", home.display())], &env, None),
            Some(SessionsRoot::Flat(home.clone()))
        );
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn lists_session_metadata_with_explicit_home() {
        let home = std::env::temp_dir().join(format!("axome-omp-history-{}", uuid::Uuid::new_v4()));
        let folder = home.join("project");
        let bucket = home.join(".omp/agent/sessions/-project");
        std::fs::create_dir_all(&bucket).unwrap();
        let env = HashMap::from([("HOME".into(), home.to_string_lossy().into_owned())]);
        for (id, title, models) in [
            ("titled", "A session", vec![]),
            ("untitled", "", vec!["configured-only"]),
            ("model", "Model session", vec!["old", "new"]),
        ] {
            let mut contents = format!(
                "{}\n{}\n",
                serde_json::json!({"type": "title", "title": title}),
                serde_json::json!({"type": "session", "id": id, "timestamp": "2026-10-22T12:00:00Z", "cwd": folder})
            );
            for model in models {
                contents.push_str(&format!(
                    "{}\n",
                    serde_json::json!({"type": "model_change", "model": model})
                ));
            }
            if id == "model" {
                for record in [
                    serde_json::json!({"type": "message", "message": {"role": "user", "content": "First prompt"}}),
                    serde_json::json!({"type": "message", "message": {"role": "assistant", "provider": "provider", "model": "old", "content": [{"type": "text", "text": "Earlier reply"}]}}),
                    // Put the latest assistant beyond the old 16 KiB header window.
                    serde_json::json!({"type": "message", "message": {"role": "toolResult", "content": [{"type": "text", "text": "x".repeat(20_000)}]}}),
                    serde_json::json!({"type": "message", "message": {"role": "assistant", "provider": "provider", "model": "answer-model", "content": [
                        {"type": "thinking", "thinking": "Private reasoning"},
                        {"type": "text", "text": " \nLatest\tanswer "},
                        {"type": "toolCall", "name": "shell"},
                        {"type": "text", "text": format!(" \n{} ", "界".repeat(210))}
                    ]}}),
                    serde_json::json!({"type": "message", "message": {"role": "assistant", "provider": "provider", "model": "latest", "content": [{"type": "toolCall", "name": "shell"}]}}),
                    serde_json::json!({"type": "model_change", "model": "configured-after-answer"}),
                    serde_json::json!({"type": "message", "message": {"role": "system", "content": "System text"}}),
                    serde_json::json!({"type": "message", "isMeta": true, "message": {"role": "user", "content": "Metadata"}}),
                    serde_json::json!({"type": "message", "message": {"role": "user", "isMeta": true, "content": "More metadata"}}),
                    serde_json::json!({"type": "message", "message": {"role": "user", "content": " \n[Request interrupted by user]"}}),
                    serde_json::json!({"type": "message", "message": {"role": "user", "content": " \n<command>ignored</command>"}}),
                ] {
                    contents.push_str(&format!("{record}\n"));
                }
            } else if id == "titled" {
                contents.push_str("{\"type\":\"message\",\"message\":\n");
                contents.push_str(&format!(
                    "{}\n",
                    serde_json::json!({"type": "message", "message": {"role": "user", "content": [{"type": "text", "text": " \nLatest\tuser prompt "}]}})
                ));
            }
            std::fs::write(
                bucket.join(format!("2026-10-22T12-00-00-000Z_{id}.jsonl")),
                contents,
            )
            .unwrap();
        }
        std::fs::write(
            bucket.join("2026-10-22T12-00-00-000Z_activity.jsonl"),
            serde_json::json!({"type": "session", "cwd": folder}).to_string(),
        )
        .unwrap();
        let query = HistoryQuery {
            folder: Some(&folder),
            args: &[],
            env: &env,
            since: None,
        };
        let rows = (SCANNER.list)(&query).unwrap().rows;
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows.iter()
                .find(|row| row.id == "titled")
                .unwrap()
                .title
                .as_deref(),
            Some("A session")
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.id == "untitled")
                .unwrap()
                .title
                .as_deref(),
            Some("")
        );
        assert_eq!(
            rows.iter()
                .find(|row| row.id == "model")
                .unwrap()
                .model
                .as_deref(),
            Some("provider/latest")
        );
        let model = rows.iter().find(|row| row.id == "model").unwrap();
        assert_eq!(model.message_count, Some(3));
        assert_eq!(
            model.last_message,
            super::super::normalize_title(&format!("Latest answer {}", "界".repeat(210))).map(
                |text| LastMessage {
                    role: MessageRole::Agent,
                    text,
                }
            )
        );
        let titled = rows.iter().find(|row| row.id == "titled").unwrap();
        assert_eq!(
            titled.last_message,
            Some(LastMessage {
                role: MessageRole::User,
                text: "Latest user prompt".to_owned(),
            })
        );
        assert_eq!(titled.message_count, Some(1));
        assert_eq!(titled.model, None);
        let untitled = rows.iter().find(|row| row.id == "untitled").unwrap();
        assert_eq!(untitled.model, None);
        assert_eq!(untitled.last_message, None);
        assert_eq!(untitled.message_count, Some(0));
        let activity = rows.iter().find(|row| row.id == "activity").unwrap();
        assert_eq!(
            activity.created_at.as_deref(),
            Some("2026-10-22T12:00:00+00:00")
        );
        assert_eq!(activity.cwd.as_deref(), folder.to_str());
        assert_eq!(activity.last_message, None);
        assert_eq!(activity.message_count, Some(0));
        assert!((SCANNER.exists)(&query, "activity").unwrap());
        std::fs::remove_dir_all(home).unwrap();
    }
}
