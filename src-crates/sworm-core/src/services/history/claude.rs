//! Claude stores interruption markers as user text; skip them when choosing fallback titles.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Deserialize;
use serde_json::Value;

use super::{BuiltinScanner, HistoryQuery, HistoryRow};

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

fn list(query: &HistoryQuery<'_>) -> Result<Vec<HistoryRow>, String> {
    let Some(projects) = projects_dir(query) else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    if let Some(folder) = query.folder {
        list_project(
            &projects.join(project_dir_name(&folder.to_string_lossy())),
            query,
            &mut rows,
        )?;
    } else if let Some(entries) = missing_ok(fs::read_dir(projects))? {
        for entry in entries {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                list_project(&entry.path(), query, &mut rows)?;
            }
        }
    }
    Ok(rows)
}

fn list_project(
    project: &Path,
    query: &HistoryQuery<'_>,
    rows: &mut Vec<HistoryRow>,
) -> Result<(), String> {
    let Some(entries) = missing_ok(fs::read_dir(project))? else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path
            .extension()
            .is_none_or(|extension| extension != "jsonl")
        {
            continue;
        }
        let Some(metadata) = missing_ok(entry.metadata())? else {
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
        let Some(file) = missing_ok(File::open(&path))? else {
            continue;
        };
        let Some(mut row) = read_session(file)? else {
            continue;
        };
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        row.id = id.to_owned();
        row.updated_at = modified
            .map(|time| DateTime::<Utc>::from(time).to_rfc3339_opts(SecondsFormat::Millis, true));
        rows.push(row);
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
    let text = content.as_str().or_else(|| {
        content
            .as_array()?
            .iter()
            .find(|block| block.get("type").and_then(Value::as_str) == Some("text"))?
            .get("text")?
            .as_str()
    })?;
    if text.starts_with('<') || text.starts_with("[Request interrupted") {
        return None;
    }
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(120).collect())
}

fn read_session(file: File) -> Result<Option<HistoryRow>, String> {
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut last_assistant = String::new();
    let mut row = HistoryRow::default();
    let mut has_user = false;
    let mut custom_title = None;
    let mut ai_title = None;
    let mut fallback_title = None;
    loop {
        line.clear();
        if reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            break;
        }
        if !["\"user\"", "custom-title", "ai-title", "\"assistant\""]
            .iter()
            .any(|needle| line.contains(needle))
        {
            continue;
        }
        if line.contains("\"assistant\"") && record_type(&line) == Some("assistant") {
            last_assistant.clone_from(&line);
            continue;
        }
        let Ok(record) = serde_json::from_str::<Value>(&line) else {
            continue;
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
            }
            _ => {}
        }
    }
    if !has_user {
        return Ok(None);
    }
    row.title = custom_title.or(ai_title).or(fallback_title);
    if let Ok(record) = serde_json::from_str::<Value>(&last_assistant) {
        row.model = record
            .get("message")
            .and_then(|message| string_field(message, "model"));
    }
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
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        if reader
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            return Ok(false);
        }
        if line.contains("\"user\"") && record_type(&line) == Some("user") {
            return Ok(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

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
            serde_json::json!({"type": "user", "message": {"content": "<command>ignored</command>"}}),
            serde_json::json!({"type": "user", "message": {"content": [{"type": "text", "text": "[Request interrupted by user]"}]}}),
            serde_json::json!({"type": "user", "gitBranch": "main", "message": {"content": [{"type": "text", "text": "\nFirst prompt\nNext line"}]}}),
            serde_json::json!({"type": "user", "gitBranch": "feature", "message": {"content": "Later prompt"}}),
            serde_json::json!({"type": "assistant", "message": {"model": "old-model"}}),
            serde_json::json!({"type": "assistant", "message": {"model": "latest-model"}}),
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
            let rows = list(&query).unwrap();
            let row = rows.iter().find(|row| row.id == id).unwrap();
            assert_eq!(row.title.as_deref(), Some(expected_title));
            assert_eq!(row.cwd.as_deref(), folder.to_str());
            assert_eq!(row.created_at.as_deref(), Some("2026-10-22T10:00:00Z"));
            assert_eq!(row.git_branch.as_deref(), Some("feature"));
            assert_eq!(row.model.as_deref(), Some("latest-model"));
            assert!(exists(&query, id).unwrap());
        }
        fs::write(
            project.join("no-user.jsonl"),
            "{\"type\":\"assistant\",\"message\":{\"model\":\"model\"}}\n",
        )
        .unwrap();
        assert_eq!(list(&query).unwrap().len(), 3);
        assert!(!exists(&query, "no-user").unwrap());
        fs::remove_dir_all(home).unwrap();
    }
}
