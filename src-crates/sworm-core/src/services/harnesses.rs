use crate::services::extensions::{
    missing_requirement, resolve_contribution_command, LoadedExtensionCatalog, LoadedHarness,
};
use std::collections::HashMap;
use sworm_protocol::extensions::{
    HarnessChat, HarnessTerminal, VersionStream, SESSION_ID_VARIABLE,
};
use sworm_protocol::harness::{HarnessConnectionStatus, HarnessPresetEntry, HarnessStatus};
use sworm_protocol::session::SessionView;
use sworm_protocol::settings::{EffectiveSettings, HarnessSettings};
use tracing::{info, warn};

pub(crate) fn binary_override(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

/// Override verbatim when non-blank; otherwise resolve the contribution command.
pub fn resolve_harness_command(
    harness: &LoadedHarness,
    merged_path: &str,
    override_path: Option<&str>,
) -> Result<String, String> {
    if let Some(path) = binary_override(override_path) {
        return Ok(path.to_string());
    }
    resolve_contribution_command(
        &harness.definition.command,
        harness.extension_dir.as_deref(),
        merged_path,
        None,
    )
}

/// The harness binary override applies when the chat agent is the harness CLI itself (`omp acp`).
pub fn resolve_chat_command(
    harness: &LoadedHarness,
    chat: &HarnessChat,
    merged_path: &str,
    override_path: Option<&str>,
) -> Result<String, String> {
    if chat.command == harness.definition.command {
        if let Some(path) = binary_override(override_path) {
            return Ok(path.to_string());
        }
    }
    resolve_contribution_command(
        &chat.command,
        harness.extension_dir.as_deref(),
        merged_path,
        None,
    )
}

/// Harness extra args, preset args, then the agent's own args.
pub(crate) fn chat_args(
    chat: &HarnessChat,
    extra_args: &[String],
    preset_args: &[String],
) -> Vec<String> {
    let mut args = Vec::with_capacity(extra_args.len() + preset_args.len() + chat.args.len());
    args.extend_from_slice(extra_args);
    args.extend_from_slice(preset_args);
    args.extend_from_slice(&chat.args);
    args
}

pub(crate) enum SessionLaunch<'a> {
    Fresh,
    Resume(&'a str),
    Mint(&'a str),
}

/// Base args, substituted session args, extra args, then preset args.
pub(crate) fn terminal_args(
    terminal: &HarnessTerminal,
    launch: SessionLaunch<'_>,
    extra_args: &[String],
    preset_args: &[String],
) -> Vec<String> {
    let (session_args, session_id) = match launch {
        SessionLaunch::Fresh => (&[][..], ""),
        SessionLaunch::Resume(id) => (terminal.resume_args.as_slice(), id),
        SessionLaunch::Mint(id) => (terminal.new_session_args.as_slice(), id),
    };
    let mut args = Vec::with_capacity(
        terminal.args.len() + session_args.len() + extra_args.len() + preset_args.len(),
    );
    args.extend(terminal.args.iter().cloned());
    args.extend(
        session_args
            .iter()
            .map(|arg| arg.replace(SESSION_ID_VARIABLE, session_id)),
    );
    args.extend_from_slice(extra_args);
    args.extend_from_slice(preset_args);
    args
}

pub fn detect_all(
    catalog: &LoadedExtensionCatalog,
    settings: &EffectiveSettings,
    env: &HashMap<String, String>,
) -> Vec<HarnessStatus> {
    let fallback = HarnessSettings::default();
    let config_for = |harness: &LoadedHarness| {
        settings
            .harnesses
            .get(&harness.definition.id)
            .unwrap_or(&fallback)
    };
    std::thread::scope(|scope| {
        let handles = catalog
            .harnesses
            .iter()
            .map(|harness| {
                let config = config_for(harness);
                let handle = scope.spawn(move || detect_harness(harness, env, config));
                (harness, handle)
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|(harness, handle)| {
                handle.join().unwrap_or_else(|_| {
                    let mut status = harness_status(harness, config_for(harness));
                    status.status = HarnessConnectionStatus::Error;
                    status.message = Some("Harness detection panicked".to_string());
                    status
                })
            })
            .collect()
    })
}

fn harness_status(harness: &LoadedHarness, config: &HarnessSettings) -> HarnessStatus {
    HarnessStatus {
        id: harness.definition.id.clone(),
        label: harness.definition.label.clone(),
        status: HarnessConnectionStatus::Missing,
        enabled: config.enabled,
        presets: config
            .presets
            .iter()
            .map(|(name, preset)| HarnessPresetEntry {
                name: name.clone(),
                view: match preset.view {
                    Some(SessionView::Chat) if harness.definition.chat.is_some() => {
                        SessionView::Chat
                    }
                    _ => SessionView::Terminal,
                },
            })
            .collect(),
        version: None,
        resolved_path: None,
        message: None,
        install_hint: harness.install_hint.clone(),
    }
}

fn detect_harness(
    harness: &LoadedHarness,
    env: &HashMap<String, String>,
    config: &HarnessSettings,
) -> HarnessStatus {
    let merged_path = env.get("PATH").map(String::as_str).unwrap_or("");
    let mut status = harness_status(harness, config);
    if let Some(message) = missing_requirement(&harness.requires, merged_path) {
        status.message = Some(message);
        return status;
    }
    let Ok(path) =
        resolve_harness_command(harness, merged_path, config.binary_path_override.as_deref())
    else {
        return status;
    };
    let definition = &harness.definition;
    match std::process::Command::new(&path)
        .env_clear()
        .envs(env)
        .args(&definition.detect.version_args)
        .output()
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // NOTE: Older OMP versions print `--version` on stderr. The fallback
            // is opt-in per manifest so stderr banners (deprecation, login
            // warnings) from other CLIs never surface as fake version strings.
            let source = if definition.detect.version_stream == VersionStream::StdoutOrStderr
                && stdout.trim().is_empty()
            {
                String::from_utf8_lossy(&output.stderr)
            } else {
                stdout
            };
            let version = source.trim().lines().next().unwrap_or("").to_string();
            info!("{} detected: {} ({})", definition.label, version, path);
            status.status = HarnessConnectionStatus::Connected;
            status.version = Some(version);
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!(
                "{} found but version check failed: {}",
                definition.label, stderr
            );
            status.status = HarnessConnectionStatus::Error;
            status.message = Some(format!("Version check failed: {}", stderr.trim()));
        }
        Err(error) => {
            warn!(
                "{} found but could not execute: {}",
                definition.label, error
            );
            status.status = HarnessConnectionStatus::Error;
            status.message = Some(format!("Execution failed: {}", error));
        }
    }
    status.resolved_path = Some(path);
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::extensions::load_catalog;

    #[cfg(unix)]
    #[test]
    fn blank_override_detects_harness_on_path() {
        let folder = std::env::temp_dir().join(format!("sworm-harness-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let binary = folder.join("claude");
        std::os::unix::fs::symlink(which::which("bash").unwrap(), &binary).unwrap();
        let path = folder.to_str().unwrap();
        let env = HashMap::from([("PATH".to_string(), path.to_string())]);
        let catalog = load_catalog(None);
        let harness = catalog.find_harness("claude_code").unwrap();
        let config = HarnessSettings {
            binary_path_override: Some(" \t ".into()),
            ..Default::default()
        };
        let detected = detect_harness(harness, &env, &config);
        let resolved =
            resolve_harness_command(harness, path, config.binary_path_override.as_deref());
        std::fs::remove_dir_all(&folder).unwrap();

        assert!(
            matches!(detected.status, HarnessConnectionStatus::Connected),
            "{detected:?}"
        );
        assert_eq!(detected.resolved_path.as_deref(), binary.to_str());
        assert_eq!(resolved.unwrap(), binary.to_str().unwrap());
    }

    #[test]
    fn terminal_args_order_and_substitution() {
        let catalog = load_catalog(None);
        assert_eq!(
            terminal_args(
                &catalog.find_harness("omp").unwrap().definition.terminal,
                SessionLaunch::Resume("abc"),
                &["-x".into()],
                &["--profile".into(), "web".into(), "--allow-home".into()],
            ),
            ["--resume", "abc", "-x", "--profile", "web", "--allow-home"]
        );
        assert_eq!(
            terminal_args(
                &catalog
                    .find_harness("claude_code")
                    .unwrap()
                    .definition
                    .terminal,
                SessionLaunch::Mint("u"),
                &[],
                &[],
            ),
            ["--session-id", "u"]
        );
        assert!(terminal_args(
            &catalog.find_harness("codex").unwrap().definition.terminal,
            SessionLaunch::Fresh,
            &[],
            &[],
        )
        .is_empty());
    }
}
