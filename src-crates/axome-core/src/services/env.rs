use crate::services::{db::DatabaseService, nix::NixService};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, LazyLock};
use tracing::{info, warn};

/// Host values a folder's Nix shell must never override; its capture can hold stale session values.
const HOST_AUTHORITATIVE_VARS: &[&str] = &[
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_TYPE",
    "SSH_AUTH_SOCK",
];

static CURRENT: LazyLock<RwLock<Arc<EnvironmentService>>> =
    LazyLock::new(|| RwLock::new(Arc::new(EnvironmentService::new())));

/// Launchers Axome owns record a variable's pre-edit value under this prefix.
const ORIGINAL_PREFIX: &str = "AXOME_ORIG_";
/// Recorded value for a variable that was unset before the launcher set it.
const UNSET_ORIGINAL: &str = "__AXOME_UNSET__";

static LAUNCH_ENV: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    restore_originals(
        std::env::vars_os()
            .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?))),
    )
});

/// Snapshot the launch environment. Call first in `main`, before this process edits its own
/// environment; hosts that never edit it may rely on the lazy capture in `EnvironmentService::new`.
pub fn capture_launch_env() {
    LazyLock::force(&LAUNCH_ENV);
}

/// Undo launcher edits recorded as `AXOME_ORIG_<NAME>` and drop the records.
fn restore_originals(vars: impl IntoIterator<Item = (String, String)>) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = vars.into_iter().collect();
    let originals: Vec<(String, String)> = env
        .iter()
        .filter_map(|(key, value)| {
            Some((
                key.strip_prefix(ORIGINAL_PREFIX)?.to_string(),
                value.clone(),
            ))
        })
        .collect();
    for (name, original) in originals {
        env.remove(&format!("{ORIGINAL_PREFIX}{name}"));
        if original == UNSET_ORIGINAL {
            env.remove(&name);
        } else {
            env.insert(name, original);
        }
    }
    env
}

/// Resolve one child environment; unavailable Nix state falls back to the host.
pub(crate) fn folder_env(db: &DatabaseService, folder: &Path) -> HashMap<String, String> {
    EnvironmentService::current().with_nix(folder_nix(db, folder).as_ref())
}

pub(crate) fn folder_nix(db: &DatabaseService, folder: &Path) -> Option<HashMap<String, String>> {
    let nix = {
        let db = db.read();
        NixService::load_env_vars(db.conn(), &folder.to_string_lossy())
    };
    match nix {
        Ok(nix) => nix,
        Err(error) => {
            warn!(
                "Failed to load Nix env for folder {}: {error}; using host environment",
                folder.display()
            );
            None
        }
    }
}

pub struct EnvironmentService {
    pub detected_shell: String,
    /// Launch environment (launcher edits undone), resolved through the login shell, plus terminal defaults.
    pub base: HashMap<String, String>,
}

impl EnvironmentService {
    fn new() -> Self {
        let detected_shell = detect_login_shell();
        let launch = &*LAUNCH_ENV;
        let mut base = probe_login_env(&detected_shell, launch).unwrap_or_else(|| {
            warn!("Login environment probe failed; children get the launch environment");
            launch.clone()
        });

        // The launching terminal's capabilities win; the probe ran without a terminal.
        for (key, default) in [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")] {
            match launch.get(key) {
                Some(value) => {
                    base.insert(key.to_string(), value.clone());
                }
                None => {
                    base.entry(key.to_string())
                        .or_insert_with(|| default.to_string());
                }
            }
        }

        Self {
            detected_shell,
            base,
        }
    }

    pub fn current() -> Arc<EnvironmentService> {
        CURRENT.read().clone()
    }

    pub fn reload() {
        let environment = Arc::new(Self::new());
        *CURRENT.write() = environment;
    }

    /// Tests inject fake tools here instead of through the launch snapshot.
    #[doc(hidden)]
    pub fn update_base_for_tests(edit: impl FnOnce(&mut HashMap<String, String>)) {
        let mut current = CURRENT.write();
        let mut base = current.base.clone();
        edit(&mut base);
        *current = Arc::new(Self {
            detected_shell: current.detected_shell.clone(),
            base,
        });
    }

    pub fn path(&self) -> &str {
        self.base.get("PATH").map(String::as_str).unwrap_or("")
    }

    /// Overlay Nix tools while preserving host session integration.
    pub fn with_nix(&self, nix: Option<&HashMap<String, String>>) -> HashMap<String, String> {
        let mut merged = self.base.clone();
        let Some(nix) = nix else {
            return merged;
        };

        for (key, value) in nix {
            if key == "PATH" {
                merged.insert(key.clone(), merge_paths(value, self.path()));
            } else if !HOST_AUTHORITATIVE_VARS.contains(&key.as_str()) {
                merged.insert(key.clone(), value.clone());
            }
        }

        // NixOS shell init re-sources set-environment, replacing PATH, unless
        // this is set. Session-launched hosts pass it through `nix develop`; a
        // systemd daemon has none, so its fish children would drop the devshell.
        merged
            .entry("__NIXOS_SET_ENVIRONMENT_DONE".to_string())
            .or_insert_with(|| "1".to_string());
        merged
    }
}

/// Detect the user's login shell.
///
/// `$SHELL` can be overridden by tooling (e.g. `nix develop` sets it to
/// a Nix-store bash). The authoritative source is `/etc/passwd`, so we
/// check that first via `getent passwd $USER` and fall back to `$SHELL`.
fn detect_login_shell() -> String {
    // Check /etc/passwd — this is the user's configured login shell
    if let Ok(user) = std::env::var("USER") {
        if let Ok(output) = std::process::Command::new("getent")
            .args(["passwd", &user])
            .env_clear()
            .envs(std::env::var("PATH").ok().map(|path| ("PATH", path)))
            .output()
        {
            if output.status.success() {
                let line = String::from_utf8_lossy(&output.stdout);
                if let Some(shell) = line.trim().rsplit(':').next() {
                    if !shell.is_empty() && shell != "/bin/false" && shell != "/usr/sbin/nologin" {
                        info!("Login shell from /etc/passwd: {}", shell);
                        return shell.to_string();
                    }
                }
            }
        }
    }

    // Fall back to $SHELL
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    info!("Login shell from $SHELL: {}", shell);
    shell
}

fn probe_login_env(
    shell: &str,
    launch: &HashMap<String, String>,
) -> Option<HashMap<String, String>> {
    let marker = format!("__AXOME_ENV_{}__", uuid::Uuid::new_v4().simple());
    let script = format!("printf '%s' {marker}; /usr/bin/env -0; printf '%s' {marker}");
    match std::process::Command::new(shell)
        .args(["-lc", &script])
        .env_clear()
        .envs(launch)
        .output()
    {
        Ok(output) if output.status.success() => {
            let env = parse_probe_output(&output.stdout, marker.as_bytes());
            if env.is_none() {
                warn!("Login environment probe returned missing markers or an empty environment");
            }
            env
        }
        Ok(output) => {
            warn!("Login environment probe exited with {}", output.status);
            None
        }
        Err(error) => {
            warn!("Login environment probe failed to execute: {error}");
            None
        }
    }
}

fn parse_probe_output(output: &[u8], marker: &[u8]) -> Option<HashMap<String, String>> {
    let start = output
        .windows(marker.len())
        .position(|bytes| bytes == marker)?
        + marker.len();
    let end = output
        .windows(marker.len())
        .rposition(|bytes| bytes == marker)?;
    let body = output.get(start..end)?;
    let mut env = HashMap::new();
    for entry in body.split(|&byte| byte == 0) {
        let Some(separator) = entry.iter().position(|&byte| byte == b'=') else {
            continue;
        };
        let key = &entry[..separator];
        if !key
            .first()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_')
            || !key
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            continue;
        }
        let key = std::str::from_utf8(key).ok()?;
        // The probe runs without a terminal, so the TERM it reports (e.g. fish's `dumb`) describes nothing.
        if matches!(key, "SHLVL" | "PWD" | "OLDPWD" | "_" | "TERM") {
            continue;
        }
        env.insert(
            key.to_string(),
            String::from_utf8_lossy(&entry[separator + 1..]).into_owned(),
        );
    }
    (!env.is_empty()).then_some(env)
}

/// Merge two PATH strings, preferring entries from `primary` but
/// appending unique entries from `secondary`.
fn merge_paths(primary: &str, secondary: &str) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut parts = Vec::new();

    for entry in primary.split(':').chain(secondary.split(':')) {
        if !entry.is_empty() && seen.insert(entry) {
            parts.push(entry);
        }
    }

    parts.join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_output_parses_between_markers() {
        let output = b"login noise\n__MARK__KEY=a=b\0GOOD_2=yes\0bad-key=no\0SHLVL=1\0PWD=/\0OLDPWD=/tmp\0_=env\0TERM=dumb\0__MARK__logout noise";
        assert_eq!(
            parse_probe_output(output, b"__MARK__").unwrap(),
            HashMap::from([
                ("KEY".to_string(), "a=b".to_string()),
                ("GOOD_2".to_string(), "yes".to_string()),
            ])
        );
        assert!(parse_probe_output(b"noise", b"__MARK__").is_none());
        assert!(parse_probe_output(b"__MARK____MARK__", b"__MARK__").is_none());
    }

    #[test]
    fn host_vars_stay_authoritative_over_nix() {
        let mut base = HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]);
        let mut nix = HashMap::from([
            ("PATH".to_string(), "/nix/bin:/usr/bin".to_string()),
            ("NEW_VAR".to_string(), "from-nix".to_string()),
        ]);
        for (index, &key) in HOST_AUTHORITATIVE_VARS.iter().enumerate() {
            if index % 2 == 0 {
                base.insert(key.to_string(), format!("host-{key}"));
            }
            nix.insert(key.to_string(), format!("nix-{key}"));
        }
        let env = EnvironmentService {
            detected_shell: "/bin/sh".to_string(),
            base,
        };
        let merged = env.with_nix(Some(&nix));
        for &key in HOST_AUTHORITATIVE_VARS {
            assert_eq!(merged.get(key), env.base.get(key), "{key}");
        }
        assert_eq!(merged["PATH"], "/nix/bin:/usr/bin");
        assert_eq!(merged["NEW_VAR"], "from-nix");
        assert_eq!(merged["__NIXOS_SET_ENVIRONMENT_DONE"], "1");
        assert_eq!(env.with_nix(None), env.base);
    }

    #[test]
    fn launch_env_undoes_recorded_launcher_edits() {
        let env = restore_originals([
            (
                "XDG_DATA_DIRS".to_string(),
                "/wrapper/share:/usr/share".to_string(),
            ),
            (
                "AXOME_ORIG_XDG_DATA_DIRS".to_string(),
                "/usr/share".to_string(),
            ),
            ("GIO_EXTRA_MODULES".to_string(), "/wrapper/gio".to_string()),
            (
                "AXOME_ORIG_GIO_EXTRA_MODULES".to_string(),
                UNSET_ORIGINAL.to_string(),
            ),
            ("XAUTHORITY".to_string(), "/run/user/1000/xauth".to_string()),
        ]);
        assert_eq!(
            env,
            HashMap::from([
                ("XDG_DATA_DIRS".to_string(), "/usr/share".to_string()),
                ("XAUTHORITY".to_string(), "/run/user/1000/xauth".to_string()),
            ])
        );
    }

    #[test]
    fn login_probe_sees_login_files() {
        let path = std::env::var("PATH").unwrap_or_default();
        let Ok(bash) = which::which_in("bash", Some(&path), ".") else {
            eprintln!("Skipping login probe test: bash is not on PATH");
            return;
        };
        let home = std::env::temp_dir().join(format!("axome-env-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join(".bash_profile"),
            "echo login-noise\nexport AXOME_PROBE_TEST=1\n",
        )
        .unwrap();
        let seed = HashMap::from([
            ("HOME".to_string(), home.to_string_lossy().into_owned()),
            ("PATH".to_string(), path),
        ]);
        let result = probe_login_env(bash.to_str().unwrap(), &seed);
        std::fs::remove_dir_all(&home).unwrap();
        let env = result.unwrap();
        assert_eq!(env["AXOME_PROBE_TEST"], "1");
        assert!(!env.keys().any(|key| key.contains("login-noise")));
    }

    #[test]
    fn test_merge_paths_deduplicates() {
        let result = merge_paths("/usr/bin:/usr/local/bin", "/usr/bin:/opt/bin");
        assert_eq!(result, "/usr/bin:/usr/local/bin:/opt/bin");
    }

    #[test]
    fn test_merge_paths_empty() {
        let result = merge_paths("", "/usr/bin");
        assert_eq!(result, "/usr/bin");
    }

    #[test]
    fn test_merge_paths_primary_only() {
        let result = merge_paths("/a:/b", "");
        assert_eq!(result, "/a:/b");
    }
}
