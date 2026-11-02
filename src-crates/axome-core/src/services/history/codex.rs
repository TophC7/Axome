use super::{store_home, BuiltinScanner, HistoryQuery, HistoryRow, Listing};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params_from_iter, Connection, OpenFlags, OptionalExtension, ToSql};
use serde_json::Value;
use std::collections::HashSet;
use std::fs::File;
use std::io;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use axome_protocol::session::MessageRole;

pub(super) const SCANNER: BuiltinScanner = BuiltinScanner {
    name: "codex",
    // `threads.created_at` is whole seconds.
    created_precision: std::time::Duration::from_secs(1),
    list,
    exists,
};

fn database(query: &HistoryQuery<'_>) -> Option<PathBuf> {
    let root = query
        .env
        .get("CODEX_HOME")
        .map(|root| root.trim())
        .filter(|root| !root.is_empty())
        .map(PathBuf::from)
        .or_else(|| store_home(query.env).map(|home| home.join(".codex")))?;
    Some(root.join("state_5.sqlite"))
}

fn open(query: &HistoryQuery<'_>) -> Result<Option<Connection>, String> {
    let Some(path) = database(query) else {
        return Ok(None);
    };
    if !path.try_exists().map_err(|error| error.to_string())? {
        return Ok(None);
    }
    Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map(Some)
    .map_err(|error| error.to_string())
}

fn timestamp(milliseconds: Option<i64>) -> Option<String> {
    milliseconds
        .and_then(DateTime::<Utc>::from_timestamp_millis)
        .map(|time| time.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn list(query: &HistoryQuery<'_>) -> Result<Listing, String> {
    // Codex commits through its WAL, so the main file alone misses most writes.
    let mut watch = database(query)
        .map(|path| vec![path.with_extension("sqlite-wal"), path])
        .unwrap_or_default();
    Ok(Listing {
        rows: list_threads(query, &mut watch)?,
        skipped: 0,
        watch,
    })
}

fn list_threads(
    query: &HistoryQuery<'_>,
    watch: &mut Vec<PathBuf>,
) -> Result<Vec<HistoryRow>, String> {
    let Some(connection) = open(query)? else {
        return Ok(Vec::new());
    };
    let mut statement = connection
        .prepare("PRAGMA table_info(threads)")
        .map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|error| error.to_string())?;
    if ["id", "cwd", "created_at", "updated_at", "archived"]
        .iter()
        .any(|column| !columns.contains(*column))
    {
        return Err("Codex threads table lacks required columns".into());
    }
    let optional = |column: &'static str| {
        if columns.contains(column) {
            column
        } else {
            "NULL"
        }
    };
    let mut sql = format!(
        "SELECT id, cwd, COALESCE(NULLIF({}, ''), NULLIF({}, '')), \
         COALESCE({}, created_at * 1000), COALESCE({}, updated_at * 1000), \
         {}, {} FROM threads WHERE archived = 0",
        optional("name"),
        optional("title"),
        optional("created_at_ms"),
        optional("updated_at_ms"),
        optional("git_branch"),
        optional("rollout_path"),
    );
    if columns.contains("source") {
        sql.push_str(" AND source NOT LIKE '{%'");
    }
    let folder = query.folder.map(|folder| folder.to_string_lossy());
    if folder.is_some() {
        sql.push_str(" AND cwd = ?");
    }
    let since = query
        .since
        .map(|time| DateTime::<Utc>::from(time).timestamp());
    if since.is_some() {
        sql.push_str(" AND created_at >= ?");
    }
    let params = [
        folder.as_ref().map(|folder| folder as &dyn ToSql),
        since.as_ref().map(|since| since as &dyn ToSql),
    ];
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params_from_iter(params.into_iter().flatten()), |row| {
            Ok((
                HistoryRow {
                    id: row.get(0)?,
                    cwd: row.get(1)?,
                    title: row.get(2)?,
                    created_at: timestamp(row.get(3)?),
                    updated_at: timestamp(row.get(4)?),
                    git_branch: row.get(5)?,
                    ..HistoryRow::default()
                },
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(rows
        .into_iter()
        .map(|(mut row, rollout)| {
            if let Some(path) = rollout.filter(|path| !path.trim().is_empty()) {
                let path = PathBuf::from(path);
                if let Ok(Some(rollout)) = std::fs::metadata(&path).and_then(|metadata| {
                    super::parsed(SCANNER.name, &path, &metadata, || {
                        read_rollout(&path).map(Some)
                    })
                }) {
                    row.last_message = rollout.last_message;
                    row.message_count = rollout.message_count;
                    row.model = rollout.model;
                }
                watch.push(path);
            }
            row
        })
        .collect())
}

fn read_rollout(path: &Path) -> io::Result<HistoryRow> {
    let mut rollout = HistoryRow {
        message_count: Some(0),
        ..HistoryRow::default()
    };
    super::each_line(File::open(path)?, |line| {
        let Ok(record) = serde_json::from_str::<Value>(line) else {
            return ControlFlow::Continue(());
        };
        let payload = &record["payload"];
        match record["type"].as_str() {
            Some("turn_context") => {
                // The latest recorded turn context, never `threads.model` or
                // session configuration, which may change without a response.
                rollout.model = payload["model"]
                    .as_str()
                    .map(str::trim)
                    .filter(|model| !model.is_empty())
                    .map(str::to_owned);
            }
            Some("response_item") if record["isMeta"].as_bool() != Some(true) => {
                // Count canonical response-item user/assistant text messages once;
                // event_msg mirrors, system messages, and tool-only items do not count.
                let role = match payload["role"].as_str() {
                    Some("user") => MessageRole::User,
                    Some("assistant") => MessageRole::Agent,
                    _ => return ControlFlow::Continue(()),
                };
                rollout.push_message(role, response_item_blocks(payload));
            }
            _ => {}
        }
        ControlFlow::Continue(())
    })?;
    Ok(rollout)
}

fn response_item_blocks(payload: &Value) -> impl Iterator<Item = &str> {
    let is_message =
        payload["type"].as_str() == Some("message") && payload["isMeta"].as_bool() != Some(true);
    payload["content"]
        .as_array()
        .filter(|_| is_message)
        .into_iter()
        .flatten()
        .filter(|block| {
            matches!(
                block["type"].as_str(),
                Some("input_text" | "output_text" | "text")
            )
        })
        .filter_map(|block| block["text"].as_str())
        .filter(|text| !text.trim_start().starts_with("# AGENTS.md instructions"))
}

fn exists(query: &HistoryQuery<'_>, id: &str) -> Result<bool, String> {
    let Some(connection) = open(query)? else {
        return Ok(false);
    };
    let Some(folder) = query.folder else {
        return Ok(false);
    };
    connection
        .query_row(
            "SELECT 1 FROM threads WHERE id = ?1 AND cwd = ?2 AND archived = 0",
            rusqlite::params![id, folder.to_string_lossy()],
            |_| Ok(()),
        )
        .optional()
        .map(|row| row.is_some())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use axome_protocol::session::LastMessage;

    #[test]
    fn optional_columns_and_folder_filter() {
        let root = std::env::temp_dir().join(format!("axome-codex-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let connection = Connection::open(root.join("state_5.sqlite")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE threads (
                    id TEXT PRIMARY KEY, cwd TEXT NOT NULL, created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL, archived INTEGER NOT NULL, title TEXT, source TEXT
                );
                INSERT INTO threads VALUES
                    ('visible', '/project', 100, 101, 0, 'A session', 'cli'),
                    ('archived', '/project', 100, 101, 1, 'Archived', 'cli'),
                    ('subagent', '/project', 100, 101, 0, 'Child', '{\"subagent\":{}}'),
                    ('other', '/other', 102, 103, 0, 'Other project', 'cli');",
            )
            .unwrap();
        drop(connection);
        let env = HashMap::from([("CODEX_HOME".into(), root.to_string_lossy().into_owned())]);
        let mut query = HistoryQuery {
            folder: None,
            args: &[],
            env: &env,
            since: None,
        };
        let rows = list(&query).unwrap().rows;
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.model.is_none()));
        assert!(rows.iter().all(|row| row.git_branch.is_none()));
        assert!(rows
            .iter()
            .all(|row| row.id == "visible" || row.id == "other"));
        query.folder = Some(std::path::Path::new("/project"));
        let rows = list(&query).unwrap().rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "visible");
        assert_eq!(rows[0].title.as_deref(), Some("A session"));
        assert_eq!(
            rows[0].created_at.as_deref(),
            Some("1970-01-01T00:01:40.000Z")
        );
        assert_eq!(
            rows[0].updated_at.as_deref(),
            Some("1970-01-01T00:01:41.000Z")
        );
        assert!(exists(&query, "visible").unwrap());
        assert!(!exists(&query, "archived").unwrap());
        assert!(!exists(&query, "other").unwrap());
        let connection = Connection::open(root.join("state_5.sqlite")).unwrap();
        connection
            .execute_batch(
                "ALTER TABLE threads ADD COLUMN model TEXT;
                 ALTER TABLE threads ADD COLUMN preview TEXT;
                 ALTER TABLE threads ADD COLUMN first_user_message TEXT;
                 ALTER TABLE threads ADD COLUMN git_branch TEXT;
                 UPDATE threads SET model = 'configured-but-unused', preview = 'Initial preview',
                    first_user_message = 'First user message', git_branch = 'main'
                    WHERE id = 'visible';",
            )
            .unwrap();
        let rows = list(&query).unwrap().rows;
        assert!(rows[0].model.is_none());
        assert!(rows[0].last_message.is_none());
        assert!(rows[0].message_count.is_none());
        assert_eq!(rows[0].git_branch.as_deref(), Some("main"));
        connection
            .execute_batch("ALTER TABLE threads ADD COLUMN rollout_path TEXT;")
            .unwrap();
        let rollout = root.join("rollout.jsonl");
        let latest = format!(" Latest\n answer\t{} ", "界".repeat(220));
        let records = [
            serde_json::json!({"type":"turn_context","payload":{"model":"old-model"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions for /project"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<environment_context>metadata</environment_context>"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"First\n prompt"},{"type":"input_image","text":"not text"},{"type":"input_text","text":"more"}]}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"user_message","message":"First prompt more"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Earlier answer"},{"type":"tool_use","text":"not conversation text"}]}}),
            serde_json::json!({"type":"turn_context","payload":{"model":"latest-model"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":latest}]}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"agent_message","message":"duplicate answer"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"function_call","name":"tool","arguments":"not a message"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"function_call_output","output":"tool output"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"system","content":[{"type":"input_text","text":"system instructions"}]}}),
            serde_json::json!({"type":"response_item","isMeta":true,"payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"metadata"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","isMeta":true,"content":[{"type":"input_text","text":"metadata"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"[Request interrupted by user]"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<turn_aborted>Interrupted</turn_aborted>"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"reasoning","text":"not conversation text"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":" \n\t "} ]}}),
        ];
        let contents = records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        // A torn final write does not hide the preceding canonical messages.
        std::fs::write(&rollout, format!("{contents}\n{{\"type\":")).unwrap();
        connection
            .execute(
                "UPDATE threads SET rollout_path = ?1 WHERE id = 'visible'",
                [rollout.to_string_lossy()],
            )
            .unwrap();
        let listing = list(&query).unwrap();
        assert!(listing.watch.contains(&rollout));
        let row = &listing.rows[0];
        assert_eq!(row.model.as_deref(), Some("latest-model"));
        assert_eq!(row.message_count, Some(3));
        assert_eq!(
            row.last_message,
            super::super::normalize_title(&latest).map(|text| LastMessage {
                role: MessageRole::Agent,
                text,
            })
        );
        assert_eq!(row.last_message.as_ref().unwrap().text.chars().count(), 200);
        assert_eq!(row.title.as_deref(), Some("A session"));
        assert_eq!(row.git_branch.as_deref(), Some("main"));
        let user_message = serde_json::json!({
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": " Follow-up\n prompt "}]
            }
        });
        std::fs::write(&rollout, format!("{contents}\n{user_message}\n{{\"type\":")).unwrap();
        let listing = list(&query).unwrap();
        assert_eq!(listing.rows[0].message_count, Some(4));
        assert_eq!(
            listing.rows[0].last_message,
            Some(LastMessage {
                role: MessageRole::User,
                text: "Follow-up prompt".into(),
            })
        );
        connection
            .execute(
                "UPDATE threads SET rollout_path = ?1 WHERE id = 'visible'",
                [root.join("missing.jsonl").to_string_lossy()],
            )
            .unwrap();
        let rows = list(&query).unwrap().rows;
        assert!(rows[0].model.is_none());
        assert!(rows[0].last_message.is_none());
        assert!(rows[0].message_count.is_none());
        drop(connection);
        std::fs::remove_dir_all(root).unwrap();
    }
}
