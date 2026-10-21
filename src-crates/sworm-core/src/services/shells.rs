use crate::services::extensions::resolve_contribution_command;
use crate::services::harnesses::binary_override;
use std::collections::HashMap;
use std::path::Path;
use sworm_protocol::settings::{ShellPresetSettings, ShellSettings};
use sworm_protocol::shell::{ShellListing, ShellPresetEntry};

pub(crate) struct ShellLaunch {
    pub cmd: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub startup: Option<String>,
}

fn command(
    settings: &ShellSettings,
    preset: Option<(&str, &ShellPresetSettings)>,
    path: &str,
    detected_shell: &str,
) -> Result<String, String> {
    if let Some((name, preset)) = preset {
        if let Some(command) = &preset.command {
            if command.contains('/') {
                return Ok(command.clone());
            }
            return resolve_contribution_command(command, None, path, None)
                .map_err(|_| format!("Shell preset {name}: {command} not found on PATH"));
        }
    }
    Ok(binary_override(settings.binary_path_override.as_deref())
        .unwrap_or(detected_shell)
        .to_owned())
}

pub(crate) fn resolve(
    settings: &ShellSettings,
    preset: Option<&str>,
    mut folder_env: HashMap<String, String>,
    detected_shell: &str,
) -> Result<ShellLaunch, String> {
    let preset = preset
        .map(|name| {
            settings
                .presets
                .get(name)
                .map(|preset| (name, preset))
                .ok_or_else(|| format!("Unknown shell preset {name}"))
        })
        .transpose()?;
    let cmd = command(
        settings,
        preset,
        folder_env
            .get("PATH")
            .map(String::as_str)
            .unwrap_or_default(),
        detected_shell,
    )?;
    let mut args = settings.extra_args.clone();
    let mut startup = None;
    if let Some((_, preset)) = preset {
        args.extend_from_slice(&preset.args);
        folder_env.extend(
            preset
                .env
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        startup = preset.startup.clone();
    }
    Ok(ShellLaunch {
        cmd,
        args,
        env: folder_env,
        startup,
    })
}

fn program_name(command: &str) -> String {
    Path::new(command)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| command.to_owned())
}

pub(crate) fn listing(settings: &ShellSettings, path: &str, detected_shell: &str) -> ShellListing {
    let base = binary_override(settings.binary_path_override.as_deref()).unwrap_or(detected_shell);
    ShellListing {
        shell: program_name(base),
        presets: settings
            .presets
            .iter()
            .map(|(name, preset)| {
                let shell = match command(settings, Some((name, preset)), path, detected_shell) {
                    Ok(command) => program_name(&command),
                    Err(_) => preset.command.as_deref().unwrap_or(base).to_owned(),
                };
                ShellPresetEntry {
                    name: name.clone(),
                    shell,
                }
            })
            .collect(),
        default_preset: settings.default_preset.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_order_and_preset_resolution() {
        let mut settings = ShellSettings {
            binary_path_override: Some("/override/sh".into()),
            extra_args: vec!["--base".into()],
            ..Default::default()
        };
        settings.presets.insert(
            "dev".into(),
            ShellPresetSettings {
                command: Some("/preset/sh".into()),
                args: vec!["--preset".into()],
                env: [("A".into(), "preset".into())].into(),
                startup: Some("echo ready".into()),
            },
        );
        let env = [("A".into(), "folder".into()), ("B".into(), "kept".into())].into();
        let launch = resolve(&settings, Some("dev"), env, "/detected/sh").unwrap();
        assert_eq!(launch.cmd, "/preset/sh");
        assert_eq!(launch.args, ["--base", "--preset"]);
        assert_eq!(launch.env["A"], "preset");
        assert_eq!(launch.env["B"], "kept");
        assert_eq!(launch.startup.as_deref(), Some("echo ready"));
        settings.presets.get_mut("dev").unwrap().command = None;
        assert_eq!(
            resolve(&settings, Some("dev"), HashMap::new(), "/detected/sh")
                .unwrap()
                .cmd,
            "/override/sh"
        );
        settings.binary_path_override = None;
        assert_eq!(
            resolve(&settings, Some("dev"), HashMap::new(), "/detected/sh")
                .unwrap()
                .cmd,
            "/detected/sh"
        );
        assert_eq!(
            resolve(&settings, Some("missing"), HashMap::new(), "/detected/sh")
                .err()
                .as_deref(),
            Some("Unknown shell preset missing")
        );
    }
}
