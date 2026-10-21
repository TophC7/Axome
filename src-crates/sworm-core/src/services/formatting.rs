use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct FormattingService;

impl FormattingService {
    pub fn run(
        program: &str,
        args: &[String],
        cwd: &str,
        content: &str,
        env: &HashMap<String, String>,
    ) -> Result<String, String> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("{program} not found: {error}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(content.as_bytes())
                .map_err(|error| format!("failed to write to {program} stdin: {error}"))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|error| format!("{program} failed: {error}"))?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|error| format!("{program} output not valid UTF-8: {error}"))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let message = if stderr.is_empty() {
                format!("{program} exited with status {}", output.status)
            } else {
                stderr
            };
            Err(format!("{program} error: {message}"))
        }
    }
}

pub(crate) fn relative_stdin_file_path(project_path: &str, file_path: &str) -> String {
    let project = Path::new(project_path);
    let path = Path::new(file_path);

    path.strip_prefix(project)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_project_prefix_for_stdin_file_path() {
        assert_eq!(
            relative_stdin_file_path("/tmp/project", "/tmp/project/src/app.ts"),
            "src/app.ts"
        );
    }

    #[test]
    fn keeps_absolute_path_when_outside_project_root() {
        assert_eq!(
            relative_stdin_file_path("/tmp/project", "/tmp/other/app.ts"),
            "/tmp/other/app.ts"
        );
    }

    #[test]
    fn formats_go_content_with_gofmt() {
        let unformatted = "package main\n\nfunc main(){\nprintln( \"hello\" )\n}\n";
        let env = HashMap::from([(
            "PATH".to_string(),
            std::env::var("PATH").unwrap_or_default(),
        )]);
        let formatted = FormattingService::run("gofmt", &[], "/tmp", unformatted, &env)
            .expect("gofmt formats valid Go code");
        assert_eq!(
            formatted,
            "package main\n\nfunc main() {\n\tprintln(\"hello\")\n}\n"
        );
    }
}
