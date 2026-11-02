//! Stores can retain summaries after conversation DB deletion and use pre-1970 dates as unset.
//! Skip missing DBs and fall back to their mtime for unset summary timestamps.

use super::{BuiltinScanner, HistoryQuery, HistoryRow, Listing};
use chrono::{DateTime, NaiveDateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub(super) const SCANNER: BuiltinScanner = BuiltinScanner {
    name: "antigravity",
    // File birth time, normalized to milliseconds.
    created_precision: std::time::Duration::from_millis(1),
    list,
    exists,
};

fn store_root(query: &HistoryQuery<'_>) -> Option<PathBuf> {
    // ponytail: no store override known
    Some(super::store_home(query.env)?.join(".gemini/antigravity-cli"))
}

fn file_uri_path(uri: &str) -> Option<String> {
    let path = uri.strip_prefix("file://")?;
    let path = path.strip_prefix("localhost").unwrap_or(path);
    if !path.starts_with('/') {
        return None;
    }
    let mut bytes = path.bytes();
    let mut decoded = Vec::with_capacity(path.len());
    while let Some(byte) = bytes.next() {
        decoded.push(if byte == b'%' {
            let high = char::from(bytes.next()?).to_digit(16)?;
            let low = char::from(bytes.next()?).to_digit(16)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    String::from_utf8(decoded).ok()
}

fn updated_at(value: &str) -> Option<String> {
    DateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f%:z")
        .map(|date| date.with_timezone(&Utc))
        .or_else(|_| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S%.f").map(|date| date.and_utc())
        })
        .ok()
        .filter(|date| date.timestamp() >= 0)
        .map(|date| date.to_rfc3339())
}

fn list(query: &HistoryQuery<'_>) -> Result<Listing, String> {
    let Some(root) = store_root(query) else {
        return Ok(Listing::default());
    };
    Ok(Listing {
        rows: list_summaries(query, &root)?,
        skipped: 0,
        // A row lists only once its conversation DB exists, which can trail the summary.
        watch: vec![
            root.join("conversation_summaries.db"),
            root.join("conversation_summaries.db-wal"),
            root.join("conversations"),
        ],
    })
}

fn list_summaries(query: &HistoryQuery<'_>, root: &Path) -> Result<Vec<HistoryRow>, String> {
    let db_path = root.join("conversation_summaries.db");
    if !db_path.try_exists().map_err(|error| error.to_string())? {
        return Ok(Vec::new());
    }
    let connection = Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Failed to open Antigravity summaries: {error}"))?;
    let since = query.since.map(|time| {
        DateTime::<Utc>::from(time)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    });
    // Unset (pre-1970) times mark summaries not yet written; new conversations still need binding.
    let sql = if since.is_some() {
        "SELECT conversation_id, title, last_modified_time, workspace_uris
         FROM conversation_summaries
         WHERE nesting_depth = 0 AND (last_modified_time >= ?1 OR last_modified_time < '1970')"
    } else {
        "SELECT conversation_id, title, last_modified_time, workspace_uris
         FROM conversation_summaries WHERE nesting_depth = 0"
    };
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| format!("Failed to query Antigravity summaries: {error}"))?;
    let mut rows = statement
        .query(rusqlite::params_from_iter(since.iter()))
        .map_err(|error| format!("Failed to query Antigravity summaries: {error}"))?;
    let folder = query.folder.map(|folder| folder.to_string_lossy());
    let mut result = Vec::new();
    let read_error = |error| format!("Failed to read Antigravity summary: {error}");
    while let Some(row) = rows.next().map_err(read_error)? {
        let id: String = row.get(0).map_err(read_error)?;
        let workspace_uris: Option<String> = row.get(3).map_err(read_error)?;
        let uris: Vec<String> = workspace_uris
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok())
            .unwrap_or_default();
        let mut paths = uris.iter().filter_map(|uri| file_uri_path(uri));
        let cwd = if let Some(folder) = folder.as_deref() {
            let normalized = folder.strip_suffix('/').unwrap_or(folder);
            if !paths.any(|path| path.strip_suffix('/').unwrap_or(&path) == normalized) {
                continue;
            }
            Some(folder.to_owned())
        } else {
            paths.next()
        };
        let modified: Option<String> = row.get(2).map_err(read_error)?;
        let Ok(metadata) = root
            .join("conversations")
            .join(format!("{id}.db"))
            .metadata()
        else {
            continue;
        };
        let created_at = metadata
            .created()
            .ok()
            .map(|time| DateTime::<Utc>::from(time).to_rfc3339());
        result.push(HistoryRow {
            id,
            cwd,
            title: row.get(1).map_err(read_error)?,
            // Summary previews are titles/initial prompts, not the last message.
            // `step_count` includes tools, not just user+assistant text records.
            // Text and assistant model require decoding per-conversation protobufs.
            last_message: None,
            message_count: None,
            created_at,
            updated_at: modified.as_deref().and_then(updated_at).or_else(|| {
                metadata
                    .modified()
                    .ok()
                    .map(|time| DateTime::<Utc>::from(time).to_rfc3339())
            }),
            model: None,
            git_branch: None,
        });
    }
    Ok(result)
}

fn exists(query: &HistoryQuery<'_>, id: &str) -> Result<bool, String> {
    let Some(root) = store_root(query) else {
        return Ok(false);
    };
    root.join("conversations")
        .join(format!("{id}.db"))
        .try_exists()
        .map_err(|error| format!("Failed to verify Antigravity conversation: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::SystemTime;

    #[test]
    fn summaries_filter_workspaces_and_parse_dates() {
        let home = std::env::temp_dir().join(format!("axome-antigravity-{}", uuid::Uuid::new_v4()));
        let root = home.join(".gemini/antigravity-cli");
        let folder = home.join("my dir");
        std::fs::create_dir_all(root.join("conversations")).unwrap();
        let connection = Connection::open(root.join("conversation_summaries.db")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE conversation_summaries (
                    conversation_id TEXT, title TEXT, last_modified_time TEXT,
                    workspace_uris TEXT, nesting_depth INTEGER,
                    preview TEXT NOT NULL DEFAULT '', step_count INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .unwrap();
        let uri = format!("file://{}", folder.to_string_lossy().replace(' ', "%20"));
        let local_uri = format!("file://localhost{}/", folder.to_string_lossy());
        for (id, modified, uris, depth) in [
            (
                "offset",
                "2026-10-20 22:57:14.957646397+00:00",
                serde_json::json!(["file:///elsewhere", uri]),
                0,
            ),
            (
                "naive",
                "2026-10-20 22:57:15.123456789",
                serde_json::json!([local_uri]),
                0,
            ),
            (
                "sentinel",
                "0001-01-01 00:00:00",
                serde_json::json!([uri]),
                0,
            ),
            (
                "missing",
                "2026-10-20 22:57:16",
                serde_json::json!([uri]),
                0,
            ),
            ("nested", "2026-10-20 22:57:16", serde_json::json!([uri]), 1),
            (
                "elsewhere",
                "2026-10-20 22:57:17",
                serde_json::json!(["file:///elsewhere"]),
                0,
            ),
            (
                "no-workspace",
                "2026-10-20 22:57:18",
                serde_json::json!([]),
                0,
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO conversation_summaries
                     (conversation_id, title, last_modified_time, workspace_uris, nesting_depth,
                      preview, step_count) VALUES (?1, ?2, ?3, ?4, ?5, 'Initial prompt', 20)",
                    rusqlite::params![id, id, modified, uris.to_string(), depth],
                )
                .unwrap();
            if id != "missing" {
                std::fs::write(root.join("conversations").join(format!("{id}.db")), []).unwrap();
            }
        }
        drop(connection);
        let env = HashMap::from([("HOME".to_owned(), home.to_string_lossy().into_owned())]);
        let mut query = HistoryQuery {
            folder: Some(&folder),
            args: &[],
            env: &env,
            since: None,
        };
        let rows = list(&query).unwrap().rows;
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row.last_message.is_none()));
        assert!(rows.iter().all(|row| row.message_count.is_none()));
        assert!(rows.iter().all(|row| row.model.is_none()));
        assert!(!rows.iter().any(|row| row.id == "missing"));
        assert!(
            DateTime::parse_from_rfc3339(
                rows.iter()
                    .find(|row| row.id == "sentinel")
                    .unwrap()
                    .updated_at
                    .as_deref()
                    .unwrap()
            )
            .unwrap()
            .timestamp()
                >= 0
        );
        for (id, expected) in [
            ("offset", "2026-10-20T22:57:14.957646397+00:00"),
            ("naive", "2026-10-20T22:57:15.123456789+00:00"),
        ] {
            let row = rows.iter().find(|row| row.id == id).unwrap();
            assert_eq!(row.cwd.as_deref(), folder.to_str());
            assert_eq!(row.updated_at.as_deref(), Some(expected));
        }
        query.folder = None;
        let rows = list(&query).unwrap().rows;
        assert_eq!(rows.len(), 5);
        assert!(rows
            .iter()
            .any(|row| row.id == "no-workspace" && row.cwd.is_none()));
        query.folder = Some(&folder);
        query.since = Some(SystemTime::from(
            DateTime::parse_from_rfc3339("2026-10-20T22:57:15Z").unwrap(),
        ));
        let mut ids: Vec<_> = list(&query)
            .unwrap()
            .rows
            .into_iter()
            .map(|row| row.id)
            .collect();
        ids.sort();
        assert_eq!(ids, ["naive", "sentinel"]);
        std::fs::remove_dir_all(home).unwrap();
    }
}
