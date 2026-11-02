use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::Value;

use super::{store_home, HistoryRow, Listing};
use sworm_protocol::session::LastMessage;

const HISTORY_COMMAND_TIMEOUT: Duration = Duration::from_secs(if cfg!(test) { 2 } else { 30 });
const PIPE_DRAIN_GRACE: Duration = Duration::from_secs(1);

pub(super) fn run(
    program: &str,
    argv: &[String],
    env: &HashMap<String, String>,
    folder: Option<&Path>,
    args: &[String],
    since: Option<SystemTime>,
) -> Result<Listing, String> {
    let mut request = serde_json::to_vec(&serde_json::json!({
        "api": 1,
        "folder": folder,
        "args": args,
        "since": since.map(|time| DateTime::<Utc>::from(time)
            .to_rfc3339_opts(SecondsFormat::Millis, true)),
    }))
    .map_err(|error| format!("history command request could not be encoded: {error}"))?;
    request.push(b'\n');
    let home = store_home(env);
    let started = Instant::now();
    let mut command = Command::new(program);
    command
        .args(argv)
        .current_dir(folder.or(home.as_deref()).unwrap_or_else(|| Path::new("/")))
        .env_clear()
        .envs(env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("history command failed to start: {error}"))?;

    let mut stdin = child.stdin.take().expect("piped history stdin");
    // A command may never read stdin, or leave its pipes open in a descendant.
    // Keep all pipe I/O off the thread enforcing the deadline.
    thread::spawn(move || {
        let _ = stdin.write_all(&request);
    });
    let stdout = child.stdout.take().expect("piped history stdout");
    let (rows_tx, rows_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = rows_tx.send(read_rows(stdout));
    });
    let mut stderr = child.stderr.take().expect("piped history stderr");
    let (stderr_tx, stderr_rx) = mpsc::channel();
    thread::spawn(move || {
        let mut tail = Vec::with_capacity(4096);
        let mut buffer = [0; 2048];
        loop {
            match stderr.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    tail.extend_from_slice(&buffer[..count]);
                    if tail.len() > 2048 {
                        tail.drain(..tail.len() - 2048);
                    }
                }
            }
        }
        let _ = stderr_tx.send(tail);
    });

    let mut status = None;
    let mut rows = None;
    let mut stderr = None;
    loop {
        if status.is_none() {
            match child.try_wait() {
                Ok(exit) => status = exit,
                Err(error) => {
                    kill(&mut child);
                    return Err(format!("history command could not be waited for: {error}"));
                }
            }
        }
        if rows.is_none() {
            match rows_rx.try_recv() {
                Ok(result) => rows = Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    rows = Some(Err("history command stdout reader stopped".into()));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if stderr.is_none() {
            match stderr_rx.try_recv() {
                Ok(tail) => stderr = Some(tail),
                Err(mpsc::TryRecvError::Disconnected) => stderr = Some(Vec::new()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let (Some(status), Some(_), Some(stderr)) = (&status, &rows, &stderr) {
            if !status.success() {
                let code = status
                    .code()
                    .map(|code| code.to_string())
                    .unwrap_or_else(|| status.to_string());
                let stderr = String::from_utf8_lossy(stderr);
                let last_line = stderr
                    .lines()
                    .rev()
                    .map(str::trim)
                    .find(|line| !line.is_empty())
                    .unwrap_or("");
                return Err(format!(
                    "history command exited with status {code}: {last_line}"
                ));
            }
            return rows.take().expect("completed history stdout");
        }
        if started.elapsed() >= HISTORY_COMMAND_TIMEOUT {
            kill(&mut child);
            let deadline = Instant::now() + PIPE_DRAIN_GRACE;
            let finished = |result: Result<(), mpsc::RecvTimeoutError>| {
                !matches!(result, Err(mpsc::RecvTimeoutError::Timeout))
            };
            let drained = (rows.is_some()
                || finished(
                    rows_rx
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .map(drop),
                ))
                && (stderr.is_some()
                    || finished(
                        stderr_rx
                            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                            .map(drop),
                    ));
            if !drained {
                // ponytail: a descendant that left the group (setsid) keeps its pipe and its
                // reader thread; poll-based readers with a deadline if that ever shows up.
                tracing::warn!(
                    "history command output stayed open after kill; abandoning its readers"
                );
            }
            return Err(format!(
                "history command timed out after {}s",
                HISTORY_COMMAND_TIMEOUT.as_secs()
            ));
        }
        thread::sleep(
            Duration::from_millis(20)
                .min(HISTORY_COMMAND_TIMEOUT.saturating_sub(started.elapsed())),
        );
    }
}

/// Kill the whole group: descendants holding the output pipes would otherwise keep
/// the reader threads blocked past the deadline.
fn kill(child: &mut Child) {
    #[cfg(unix)]
    if let Err(error) =
        crate::services::pty::signal_process(-(child.id() as libc::pid_t), libc::SIGKILL)
    {
        tracing::warn!("history command group could not be killed: {error}");
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Each line is a session (`id` set) or a path to watch (`watch` set, absolute);
/// anything else is skipped and reported.
fn read_rows(stdout: impl Read) -> Result<Listing, String> {
    let mut listing = Listing::default();
    for line in BufReader::new(stdout).split(b'\n') {
        let line =
            line.map_err(|error| format!("history command stdout could not be read: {error}"))?;
        if line.trim_ascii().is_empty() {
            continue;
        }
        let Ok(Value::Object(mut object)) = serde_json::from_slice::<Value>(&line) else {
            listing.skipped += 1;
            continue;
        };
        let message_count = object
            .get("message_count")
            .and_then(Value::as_u64)
            .and_then(|count| u32::try_from(count).ok());
        let last_message = object
            .remove("last_message")
            .and_then(|value| serde_json::from_value::<LastMessage>(value).ok());
        let mut field = |key: &str| match object.remove(key) {
            Some(Value::String(value)) => Some(value),
            _ => None,
        };
        if let Some(id) = field("id").filter(|id| !id.is_empty()) {
            listing.rows.push(HistoryRow {
                id,
                cwd: field("cwd"),
                title: field("title"),
                last_message,
                // Extension commands count meaningful user/assistant messages.
                message_count,
                created_at: field("created_at"),
                updated_at: field("updated_at"),
                model: field("model"),
                git_branch: field("git_branch"),
            });
        } else if let Some(path) = field("watch")
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_absolute())
        {
            listing.watch.push(path);
        } else {
            listing.skipped += 1;
        }
    }
    Ok(listing)
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use super::*;
    use crate::services::history::{HistorySource, SourceKind};

    fn script_source(script: &str) -> (PathBuf, HistorySource) {
        let folder =
            std::env::temp_dir().join(format!("sworm-history-command-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&folder).unwrap();
        let program = folder.join("history.sh");
        fs::write(&program, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        let env = HashMap::from([
            (
                "REQ".into(),
                folder.join("request.json").to_string_lossy().into_owned(),
            ),
            ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
        ]);
        let source = HistorySource {
            harness_id: "test".into(),
            kind: SourceKind::Command {
                program: program.to_string_lossy().into_owned(),
                argv: Vec::new(),
            },
            args: vec!["--x".into()],
            env,
        };
        (folder, source)
    }

    #[test]
    fn command_request_and_normalized_folder_listing() {
        let (folder, source) = script_source(
            "cat > \"$REQ\"\nprintf '%s\\n' '{\"id\":\"a\",\"title\":\"One\",\"last_message\":{\"role\":\"agent\",\"text\":\" Last  reply \"},\"message_count\":3,\"model\":\"latest-model\",\"updated_at\":\"2026-10-01T00:00:00Z\"}' 'not json' '' '{\"watch\":\"/store/sessions\"}' '{\"watch\":\"relative\"}' '{\"id\":\"b\",\"cwd\":\"/elsewhere\"}'\nprintf '{\"id\":\"c\",\"cwd\":\"%s\"}\\n' \"$PWD\"",
        );
        let listing = source.list(Some(&folder), None).unwrap();
        assert_eq!(
            listing
                .rows
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "c"]
        );
        assert_eq!(
            listing.rows[0].last_message,
            Some(LastMessage {
                role: sworm_protocol::session::MessageRole::Agent,
                text: "Last reply".into(),
            })
        );
        assert_eq!(listing.rows[0].message_count, Some(3));
        assert_eq!(listing.rows[0].model.as_deref(), Some("latest-model"));
        assert_eq!(listing.rows[1].last_message, None);
        assert_eq!(listing.rows[1].message_count, None);
        assert_eq!(listing.watch, [PathBuf::from("/store/sessions")]);
        // `not json` and the relative watch path; the blank line is no entry.
        assert_eq!(listing.skipped, 2);
        let request: Value =
            serde_json::from_slice(&fs::read(folder.join("request.json")).unwrap()).unwrap();
        assert_eq!(request["api"], 1);
        assert_eq!(request["folder"], folder.to_string_lossy().as_ref());
        assert_eq!(request["args"], serde_json::json!(["--x"]));
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn command_nonzero_exit_is_an_error() {
        let (folder, source) = script_source("echo 'history failed' >&2\nexit 3");
        let error = source.list(Some(&folder), None).unwrap_err();
        assert!(error.contains("exited with status 3"), "{error}");
        assert!(error.contains("history failed"), "{error}");
        fs::remove_dir_all(folder).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn timeout_kills_descendants_holding_pipes() {
        let (folder, source) = script_source("echo $$ > \"$PWD/group\"\nsleep 60 &\nwait");
        let started = Instant::now();
        let error = source.list(Some(&folder), None).unwrap_err();
        assert!(error.contains("timed out"), "{error}");
        assert!(
            started.elapsed() < HISTORY_COMMAND_TIMEOUT + PIPE_DRAIN_GRACE + Duration::from_secs(1)
        );
        let group: libc::pid_t = fs::read_to_string(folder.join("group"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let poll_until = Instant::now() + Duration::from_secs(1);
        while crate::services::pty::groups_terminated(&[group]) != Ok(true)
            && Instant::now() < poll_until
        {
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(crate::services::pty::groups_terminated(&[group]), Ok(true));
        fs::remove_dir_all(folder).unwrap();
    }
}
