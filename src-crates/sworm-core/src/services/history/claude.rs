use std::fs::{self, File};
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::Value;
use sworm_protocol::session::MessageRole;

use super::{readable, BuiltinScanner, HistoryQuery, HistoryRow, Listing};

pub(super) const SCANNER: BuiltinScanner = BuiltinScanner {
    name: "claude",
    created_precision: std::time::Duration::from_millis(1),
    list,
    exists,
};

pub(crate) fn project_dir_name(cwd: &str) -> String {
    cwd.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect()
}

fn projects_dir(query: &HistoryQuery<'_>) -> Option<PathBuf> {
    query
        .env
        .get("CLAUDE_CONFIG_DIR")
        .map(|root| root.trim())
        .filter(|root| !root.is_empty())
        .map(PathBuf::from)
        .or_else(|| super::store_home(query.env).map(|home| home.join(".claude")))
        .map(|root| root.join("projects"))
}

fn missing_ok<T>(result: io::Result<T>) -> Result<Option<T>, String> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn list(query: &HistoryQuery<'_>) -> Result<Listing, String> {
    let mut listing = Listing::default();
    let Some(projects) = projects_dir(query) else {
        return Ok(listing);
    };
    if let Some(folder) = query.folder {
        let project = projects.join(project_dir_name(&folder.to_string_lossy()));
        list_project(&project, query, &mut listing)?;
        listing.watch.push(project);
        return Ok(listing);
    }
    if let Some(entries) = missing_ok(fs::read_dir(&projects))? {
        for entry in entries {
            let Some(entry) = readable(entry, &mut listing.skipped) else {
                continue;
            };
            if !readable(entry.file_type(), &mut listing.skipped).is_some_and(|kind| kind.is_dir())
            {
                continue;
            }
            let project = entry.path();
            if list_project(&project, query, &mut listing).is_err() {
                listing.skipped += 1;
            }
            listing.watch.push(project);
        }
    }
    listing.watch.push(projects);
    Ok(listing)
}

fn list_project(
    project: &Path,
    query: &HistoryQuery<'_>,
    listing: &mut Listing,
) -> Result<(), String> {
    let Some(entries) = missing_ok(fs::read_dir(project))? else {
        return Ok(());
    };
    for entry in entries {
        let Some(entry) = readable(entry, &mut listing.skipped) else {
            continue;
        };
        let path = entry.path();
        if path
            .extension()
            .is_none_or(|extension| extension != "jsonl")
        {
            continue;
        }
        let Some(metadata) = readable(entry.metadata(), &mut listing.skipped) else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let modified = metadata.modified().ok();
        if query
            .since
            .zip(modified)
            .is_some_and(|(since, mtime)| mtime < since)
        {
            continue;
        }
        let Some(Some(mut row)) = readable(
            super::parsed(SCANNER.name, &path, &metadata, || {
                File::open(&path).and_then(read_session)
            }),
            &mut listing.skipped,
        ) else {
            continue;
        };
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        row.id = id.to_owned();
        row.updated_at = modified
            .map(|time| DateTime::<Utc>::from(time).to_rfc3339_opts(SecondsFormat::Millis, true));
        listing.rows.push(row);
    }
    Ok(())
}

#[derive(Deserialize)]
struct RecordType<'a> {
    #[serde(borrow, rename = "type")]
    kind: &'a str,
}

fn record_type(line: &str) -> Option<&str> {
    serde_json::from_str::<RecordType<'_>>(line)
        .ok()
        .map(|record| record.kind)
}

fn string_field(record: &Value, field: &str) -> Option<String> {
    record.get(field).and_then(Value::as_str).map(str::to_owned)
}

fn prompt_title(record: &Value) -> Option<String> {
    if record.get("isMeta").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let content = record.get("message")?.get("content")?;
    let text = content
        .as_str()
        .or_else(|| {
            content
                .as_array()?
                .iter()
                .find(|block| block.get("type").and_then(Value::as_str) == Some("text"))?
                .get("text")?
                .as_str()
        })?
        .trim();
    if super::is_marker(text) {
        return None;
    }
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(120).collect())
}

fn read_session(file: File) -> io::Result<Option<HistoryRow>> {
    let mut row = HistoryRow {
        message_count: Some(0),
        ..HistoryRow::default()
    };
    let mut has_user = false;
    let mut custom_title = None;
    let mut ai_title = None;
    let mut fallback_title = None;
    super::each_line(file, |line| {
        if !["\"user\"", "custom-title", "ai-title", "\"assistant\""]
            .iter()
            .any(|needle| line.contains(needle))
        {
            return ControlFlow::Continue(());
        }
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            return ControlFlow::Continue(());
        };
        match record
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "custom-title" => custom_title = string_field(&record, "customTitle"),
            "ai-title" => ai_title = string_field(&record, "aiTitle"),
            "user" => {
                if !has_user {
                    has_user = true;
                    row.cwd = string_field(&record, "cwd");
                    row.created_at = string_field(&record, "timestamp");
                }
                row.git_branch = string_field(&record, "gitBranch");
                if fallback_title.is_none() {
                    fallback_title = prompt_title(&record);
                }
                row.push_message(MessageRole::User, super::conversation_blocks(&record));
            }
            "assistant" => {
                row.model = record
                    .get("message")
                    .and_then(|message| string_field(message, "model"));
                row.push_message(MessageRole::Agent, super::conversation_blocks(&record));
            }
            _ => {}
        }
        ControlFlow::Continue(())
    })?;
    if !has_user {
        return Ok(None);
    }
    row.title = custom_title.or(ai_title).or(fallback_title);
    Ok(Some(row))
}

fn exists(query: &HistoryQuery<'_>, id: &str) -> Result<bool, String> {
    let (Some(projects), Some(folder)) = (projects_dir(query), query.folder) else {
        return Ok(false);
    };
    let path = projects
        .join(project_dir_name(&folder.to_string_lossy()))
        .join(format!("{id}.jsonl"));
    let Some(file) = missing_ok(File::open(path))? else {
        return Ok(false);
    };
    let mut found = false;
    super::each_line(file, |line| {
        found = line.contains("\"user\"") && record_type(line) == Some("user");
        if found {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })
    .map_err(|error| error.to_string())?;
    Ok(found)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use sworm_protocol::session::LastMessage;

    #[test]
    fn project_slug_replaces_all_non_ascii_alphanumeric_characters() {
        assert_eq!(project_dir_name("/home/x/my_app.v2"), "-home-x-my-app-v2");
    }

    #[test]
    fn titles_follow_precedence_and_sessions_require_a_user_record() {
        let home = std::env::temp_dir().join(format!("sworm-claude-{}", uuid::Uuid::new_v4()));
        let folder = home.join("project");
        let project = home
            .join(".claude/projects")
            .join(project_dir_name(&folder.to_string_lossy()));
        fs::create_dir_all(&project).unwrap();
        let env = HashMap::from([("HOME".to_owned(), home.to_string_lossy().into_owned())]);
        let query = HistoryQuery {
            folder: Some(&folder),
            args: &[],
            env: &env,
            since: None,
        };
        let mut records = vec![
            serde_json::json!({"type": "user", "cwd": folder, "timestamp": "2026-10-22T10:00:00Z", "isMeta": true, "message": {"content": "metadata"}}),
            serde_json::json!({"type": "user", "message": {"content": " \n<command>ignored</command>"}}),
            serde_json::json!({"type": "user", "message": {"content": [{"type": "text", "text": " \n[Request interrupted by user]"}]}}),
            serde_json::json!({"type": "user", "message": {"content": [{"type": "tool_result", "content": "Tool output"}]}}),
            serde_json::json!({"type": "system", "message": {"content": "System text"}}),
            serde_json::json!({"type": "user", "gitBranch": "main", "message": {"content": [{"type": "text", "text": "\nFirst prompt\nNext line"}]}}),
            serde_json::json!({"type": "user", "gitBranch": "feature", "message": {"content": "Later prompt"}}),
            serde_json::json!({"type": "assistant", "message": {"model": "old-model", "content": "Earlier answer"}}),
            serde_json::json!({"type": "assistant", "message": {"model": "answer-model", "content": [
                {"type": "thinking", "thinking": "Private reasoning"},
                {"type": "text", "text": " \n Final\tanswer "},
                {"type": "tool_use", "name": "shell"},
                {"type": "text", "text": format!(" \n{} ", "界".repeat(210))}
            ]}}),
            serde_json::json!({"type": "assistant", "message": {"model": "latest-model", "content": [{"type": "tool_use", "name": "shell"}]}}),
            serde_json::json!({"type": "user", "isMeta": true, "gitBranch": "feature", "message": {"content": "Ignored trailing metadata"}}),
        ];
        for (id, expected_title) in [
            ("prompt", "First prompt"),
            ("ai", "Latest AI"),
            ("custom", "Latest custom"),
        ] {
            if id == "ai" {
                records.push(serde_json::json!({"type": "ai-title", "aiTitle": "Old AI"}));
                records.push(serde_json::json!({"type": "ai-title", "aiTitle": "Latest AI"}));
            }
            if id == "custom" {
                records
                    .push(serde_json::json!({"type": "custom-title", "customTitle": "Old custom"}));
                records.push(
                    serde_json::json!({"type": "custom-title", "customTitle": "Latest custom"}),
                );
            }
            let contents = records
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(project.join(format!("{id}.jsonl")), contents).unwrap();
            let rows = list(&query).unwrap().rows;
            let row = rows.iter().find(|row| row.id == id).unwrap();
            assert_eq!(row.title.as_deref(), Some(expected_title));
            assert_eq!(row.cwd.as_deref(), folder.to_str());
            assert_eq!(row.created_at.as_deref(), Some("2026-10-22T10:00:00Z"));
            assert_eq!(row.git_branch.as_deref(), Some("feature"));
            assert_eq!(row.model.as_deref(), Some("latest-model"));
            assert_eq!(row.message_count, Some(4));
            assert_eq!(
                row.last_message,
                super::super::normalize_title(&format!("Final answer {}", "界".repeat(210))).map(
                    |text| LastMessage {
                        role: MessageRole::Agent,
                        text,
                    }
                )
            );
            assert!(exists(&query, id).unwrap());
        }
        fs::write(
            project.join("no-user.jsonl"),
            "{\"type\":\"assistant\",\"message\":{\"model\":\"model\"}}\n",
        )
        .unwrap();
        assert_eq!(list(&query).unwrap().rows.len(), 3);
        // A torn multi-byte write loses that record, not the session or the source.
        let mut torn = b"{\"type\":\"user\",\"message\":{\"content\":\"\xE2\x82\"}}\n".to_vec();
        torn.extend_from_slice(
            format!(
                "{}\n",
                serde_json::json!({"type": "user", "cwd": folder, "message": {"content": "After the tear"}})
            )
            .as_bytes(),
        );
        fs::write(project.join("torn.jsonl"), torn).unwrap();
        let listing = list(&query).unwrap();
        assert_eq!(listing.skipped, 0);
        assert_eq!(
            listing
                .rows
                .iter()
                .find(|row| row.id == "torn")
                .and_then(|row| row.title.as_deref()),
            Some("After the tear")
        );
        let torn_row = listing.rows.iter().find(|row| row.id == "torn").unwrap();
        assert_eq!(
            torn_row.last_message,
            Some(LastMessage {
                role: MessageRole::User,
                text: "After the tear".to_owned(),
            })
        );
        assert_eq!(torn_row.message_count, Some(1));
        assert_eq!(torn_row.model, None);
        assert!(!exists(&query, "no-user").unwrap());
        fs::remove_dir_all(home).unwrap();
    }
}
