//! Session-store scanners keyed by bundled harness id.

use crate::services::folders::home_dir;
use crate::services::{codex_state::CodexStateReader, omp};
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Scanner {
    ClaudeCode,
    Codex,
    Omp,
    Antigravity,
}

impl Scanner {
    pub(crate) fn for_harness(id: &str) -> Option<Self> {
        match id {
            "claude_code" => Some(Self::ClaudeCode),
            "codex" => Some(Self::Codex),
            "omp" => Some(Self::Omp),
            "antigravity" => Some(Self::Antigravity),
            _ => None,
        }
    }

    pub(crate) fn session_exists(self, cwd: &str, id: &str) -> bool {
        match self {
            Self::ClaudeCode => claude_session_transcript_exists(cwd, id),
            Self::Codex => CodexStateReader::thread_exists(id, cwd).unwrap_or(false),
            Self::Omp => omp::session_exists(cwd, id),
            Self::Antigravity => antigravity_conversation_exists(id),
        }
    }

    pub(crate) fn discovers(self) -> bool {
        matches!(self, Self::Codex | Self::Omp | Self::Antigravity)
    }
}

/// Encode a working directory the same way Claude Code does for its
/// `~/.claude/projects/<dir>/<uuid>.jsonl` transcript layout: every `/`
/// and `.` becomes `-`. Hyphens pass through unchanged.
pub fn claude_project_dir_name(cwd: &str) -> String {
    cwd.chars()
        .map(|c| if c == '/' || c == '.' { '-' } else { c })
        .collect()
}

/// Path to the Claude Code transcript file for a given cwd + session UUID.
/// Returns None if `$HOME` is unset.
pub fn claude_transcript_path(cwd: &str, session_uuid: &str) -> Option<PathBuf> {
    let home = home_dir()?;
    let dir = claude_project_dir_name(cwd);
    Some(
        home.join(".claude")
            .join("projects")
            .join(dir)
            .join(format!("{}.jsonl", session_uuid)),
    )
}

/// Whether a Claude Code session transcript already exists on disk.
/// Used to choose between `--session-id` (new) and `--resume` (existing).
fn claude_session_transcript_exists(cwd: &str, session_uuid: &str) -> bool {
    claude_transcript_path(cwd, session_uuid)
        .map(|p| p.exists())
        .unwrap_or(false)
}

/// `~/.gemini/antigravity-cli` — the Antigravity CLI (`agy`) state root.
fn antigravity_dir() -> Option<PathBuf> {
    Some(home_dir()?.join(".gemini").join("antigravity-cli"))
}

/// Whether an Antigravity conversation store exists for `id`.
/// Used to choose between `--conversation <id>` and a fresh start.
fn antigravity_conversation_exists(id: &str) -> bool {
    antigravity_dir()
        .map(|dir| dir.join("conversations").join(format!("{id}.db")).exists())
        .unwrap_or(false)
}

/// Visit `conversations/*.db` files whose birth time is at or after
/// `since`, oldest first. Returning `false` stops iteration.
///
/// Fails closed: any error from `read_dir`, an entry, its metadata or
/// `created()` yields `None`, so a filesystem without birth times never
/// binds a conversation rather than binding the wrong one.
pub fn antigravity_visit_conversations_created_since(
    since: SystemTime,
    mut visit: impl FnMut(SystemTime, String) -> bool,
) -> Option<()> {
    let entries = std::fs::read_dir(antigravity_dir()?.join("conversations")).ok()?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.ok()?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("db") {
            continue;
        }
        let created_at = entry.metadata().ok()?.created().ok()?;
        if created_at < since {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        candidates.push((created_at, id.to_string()));
    }
    candidates.sort_by_key(|(created_at, _)| *created_at);
    for (created_at, id) in candidates {
        if !visit(created_at, id) {
            break;
        }
    }
    Some(())
}
#[cfg(test)]
mod tests {
    use super::*;
    // Encoding observed in real ~/.claude/projects/* dirs.
    #[test]
    fn claude_project_dir_simple_path() {
        assert_eq!(
            claude_project_dir_name("/home/toph/Development/ADE"),
            "-home-toph-Development-ADE"
        );
    }

    #[test]
    fn claude_project_dir_dot_in_segment() {
        // `/home/toph/.catnip/workspace/KwahsCore/fluffy` was observed
        // on disk as `-home-toph--catnip-workspace-KwahsCore-fluffy`.
        assert_eq!(
            claude_project_dir_name("/home/toph/.catnip/workspace/KwahsCore/fluffy"),
            "-home-toph--catnip-workspace-KwahsCore-fluffy"
        );
    }

    #[test]
    fn claude_project_dir_hyphens_pass_through() {
        // `/home/toph/Development/nerf-this` was observed on disk as
        // `-home-toph-Development-nerf-this` (existing hyphens stay).
        assert_eq!(
            claude_project_dir_name("/home/toph/Development/nerf-this"),
            "-home-toph-Development-nerf-this"
        );
    }

    #[test]
    fn claude_transcript_path_shape() {
        let home = home_dir().expect("HOME should be set for this test");
        let path = claude_transcript_path("/repo/x", "abc-123").unwrap();
        assert_eq!(path, home.join(".claude/projects/-repo-x/abc-123.jsonl"));
    }
}
