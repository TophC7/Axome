use super::{store_home, BuiltinScanner, HistoryQuery, HistoryRow};
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params_from_iter, Connection, OpenFlags, OptionalExtension, ToSql};
use std::collections::HashSet;
use std::path::PathBuf;

pub(super) const SCANNER: BuiltinScanner = BuiltinScanner {
    name: "codex",
    // `threads.created_at` is whole seconds.
    created_precision: std::time::Duration::from_secs(1),
    list,
    exists,
};

fn open(query: &HistoryQuery<'_>) -> Result<Option<Connection>, String> {
    let root = query
        .env
        .get("CODEX_HOME")
        .map(|root| root.trim())
        .filter(|root| !root.is_empty())
        .map(PathBuf::from)
        .or_else(|| store_home(query.env).map(|home| home.join(".codex")));
    let Some(root) = root else {
        return Ok(None);
    };
    let path = root.join("state_5.sqlite");
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

fn list(query: &HistoryQuery<'_>) -> Result<Vec<HistoryRow>, String> {
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
        optional("model"),
        optional("git_branch"),
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
            Ok(HistoryRow {
                id: row.get(0)?,
                cwd: row.get(1)?,
                title: row.get(2)?,
                created_at: timestamp(row.get(3)?),
                updated_at: timestamp(row.get(4)?),
                model: row.get(5)?,
                git_branch: row.get(6)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string());
    rows
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

    #[test]
    fn optional_columns_and_folder_filter() {
        let root = std::env::temp_dir().join(format!("sworm-codex-{}", uuid::Uuid::new_v4()));
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
        let rows = list(&query).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| row.model.is_none()));
        assert!(rows.iter().all(|row| row.git_branch.is_none()));
        assert!(rows
            .iter()
            .all(|row| row.id == "visible" || row.id == "other"));
        query.folder = Some(std::path::Path::new("/project"));
        let rows = list(&query).unwrap();
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
        std::fs::remove_dir_all(root).unwrap();
    }
}
