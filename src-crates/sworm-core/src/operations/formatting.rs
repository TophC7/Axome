use crate::{
    errors::ApiError,
    services::{
        extensions::{missing_requirement, resolve_contribution_command, ExtensionService},
        folders::resolve_folder,
        formatting::{relative_stdin_file_path, FormattingService},
    },
    Host,
};
use std::collections::HashMap;
use sworm_protocol::extensions::{FormatterAvailability, FormatterStatus};

impl Host {
    pub fn formatting_format(
        &self,
        folder_path: String,
        file_path: String,
        formatter_id: String,
        content: String,
    ) -> Result<String, ApiError> {
        let extensions = ExtensionService::current();
        let formatter = extensions
            .find_formatter(&formatter_id)
            .ok_or_else(|| ApiError::NotFound(format!("Unknown formatter {formatter_id}")))?;
        let (folder, env) = self.formatter_context(&folder_path)?;
        let path = env.get("PATH").map(String::as_str).unwrap_or("");
        if let Some(message) = missing_requirement(&formatter.requires, path) {
            return Err(ApiError::Internal(message));
        }
        let program = resolve_contribution_command(
            &formatter.definition.command,
            formatter.extension_dir.as_deref(),
            path,
            None,
        )
        .map_err(ApiError::Internal)?;
        let relative_file = relative_stdin_file_path(&folder, &file_path);
        let args: Vec<String> = formatter
            .definition
            .args
            .iter()
            .map(|arg| arg.replace("${relative_file}", &relative_file))
            .collect();
        FormattingService::run(&program, &args, &folder, &content, &env).map_err(ApiError::Internal)
    }

    pub fn formatting_list_formatters(
        &self,
        folder_path: Option<String>,
    ) -> Result<Vec<FormatterStatus>, ApiError> {
        let env = if let Some(folder_path) = folder_path.as_deref() {
            let folder = resolve_folder(folder_path)?;
            self.folder_env(&folder)
        } else {
            self.env.with_nix(None)
        };
        let path = env.get("PATH").map(String::as_str).unwrap_or("");
        let extensions = ExtensionService::current();
        Ok(extensions
            .formatters
            .iter()
            .map(|formatter| {
                let resolved = match missing_requirement(&formatter.requires, path) {
                    Some(message) => Err(message),
                    None => resolve_contribution_command(
                        &formatter.definition.command,
                        formatter.extension_dir.as_deref(),
                        path,
                        None,
                    ),
                };
                let (status, resolved_path, message) = match resolved {
                    Ok(program) => (FormatterAvailability::Available, Some(program), None),
                    Err(message) => (FormatterAvailability::Missing, None, Some(message)),
                };
                FormatterStatus {
                    formatter_id: formatter.formatter_id.clone(),
                    label: formatter.definition.label.clone(),
                    extension_id: formatter.extension_id.clone(),
                    extension_label: formatter.extension_label.clone(),
                    languages: formatter.definition.languages.clone(),
                    status,
                    resolved_path,
                    message,
                    install_hint: formatter.install_hint.clone(),
                }
            })
            .collect())
    }

    /// Canonical folder path and child env, overlaid on the formatter's inherited env.
    fn formatter_context(
        &self,
        folder_path: &str,
    ) -> Result<(String, HashMap<String, String>), ApiError> {
        let folder = resolve_folder(folder_path)?;
        let env = self.folder_env(&folder);
        Ok((folder.to_string_lossy().into_owned(), env))
    }
}
