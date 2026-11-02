// Central registry of JSON Schemas for Axome config files.
//
// Each entry pairs a schema with the glob patterns that identify the
// file. The frontend fetches this list per host when Monaco first loads that host's folder
// and hands each entry to Monaco so that opening a matching file gets autocomplete, validation,
// and hover docs for free. Settings schemas use the current extension catalog
// for language, LSP server, and formatter ids.
//
// Adding a new config file is a three-step change:
//   <> define/extend the canonical model under axome-protocol (schema is derived)
//   <> append a `ConfigSchemaEntry` below
//   <> ship it; the frontend picks it up automatically on next load

use schemars::schema_for;
use serde_json::json;
use axome_protocol::config_schemas::ConfigSchemaEntry;

use crate::services::{extensions::ExtensionService, settings_resolution::settings_layer_schema};
use axome_protocol::{server_config::ServerConfig, settings::SettingsLayerKind, task::TasksFile};

pub fn all_config_schemas() -> Vec<ConfigSchemaEntry> {
    let extensions = ExtensionService::current();
    let ids = &extensions.settings_ids;

    vec![
        ConfigSchemaEntry {
            id: "axome.tasks".into(),
            file_match: vec![
                "**/.axome/tasks.jsonc".into(),
                "**/.axome/tasks.json".into(),
            ],
            schema: serde_json::to_value(schema_for!(TasksFile)).expect("tasks schema serializes"),
        },
        ConfigSchemaEntry {
            id: "axome.settings.global".into(),
            file_match: vec!["**/axome/settings.jsonc".into()],
            schema: settings_layer_schema(SettingsLayerKind::Global, ids),
        },
        ConfigSchemaEntry {
            id: "axome.settings.folder".into(),
            file_match: vec!["**/.axome/settings.jsonc".into()],
            schema: settings_layer_schema(SettingsLayerKind::Folder, ids),
        },
        ConfigSchemaEntry {
            id: "axome.shortcuts".into(),
            file_match: vec!["**/shortcuts.jsonc".into()],
            schema: shortcuts_file_schema(),
        },
        ConfigSchemaEntry {
            id: "axome.server".into(),
            file_match: vec!["**/axome/server.jsonc".into()],
            schema: serde_json::to_value(schema_for!(ServerConfig))
                .expect("server config schema serializes"),
        },
    ]
}

fn shortcuts_file_schema() -> serde_json::Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Axome shortcuts",
        "description": "Global keyboard shortcut overrides. Defaults live in Axome's command registry; this file stores only user changes.",
        "type": "object",
        "additionalProperties": true,
        "properties": {
            "version": {
                "type": "integer",
                "const": 1,
                "default": 1
            },
            "bindings": {
                "type": "array",
                "default": [],
                "items": {
                    "type": "object",
                    "additionalProperties": true,
                    "required": ["command", "key"],
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Stable command ID, such as toggle-command-palette or editor:editor.action.formatDocument."
                        },
                        "key": {
                            "type": "string",
                            "description": "Shortcut chord, such as Ctrl+P or Ctrl+Shift+P."
                        }
                    }
                }
            },
            "unboundCommands": {
                "type": "array",
                "default": [],
                "items": { "type": "string" },
                "description": "Command IDs whose default shortcuts are explicitly disabled."
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::settings_resolution::SettingsCatalogIds;

    #[test]
    fn registers_settings_jsonc_schemas() {
        let schemas = all_config_schemas();
        let folder_settings = schemas
            .iter()
            .find(|entry| entry.id == "axome.settings.folder")
            .expect("folder settings schema registered");

        assert_eq!(folder_settings.file_match, vec!["**/.axome/settings.jsonc"]);

        let global_settings = schemas
            .iter()
            .find(|entry| entry.id == "axome.settings.global")
            .expect("global settings schema registered");

        assert_eq!(global_settings.file_match, vec!["**/axome/settings.jsonc"]);
    }

    #[test]
    fn registers_shortcuts_jsonc_schema() {
        let schemas = all_config_schemas();
        let shortcuts = schemas
            .iter()
            .find(|entry| entry.id == "axome.shortcuts")
            .expect("shortcuts schema registered");

        assert_eq!(shortcuts.file_match, vec!["**/shortcuts.jsonc"]);
        assert!(shortcuts.schema.to_string().contains("unboundCommands"));
    }

    #[test]
    fn registers_tasks_jsonc_schema() {
        let schemas = all_config_schemas();
        let tasks = schemas
            .iter()
            .find(|entry| entry.id == "axome.tasks")
            .expect("tasks schema registered");

        assert_eq!(
            tasks.file_match,
            vec!["**/.axome/tasks.jsonc", "**/.axome/tasks.json"]
        );
    }

    #[test]
    fn settings_schema_contains_canonical_sections_and_ids() {
        let rendered = settings_layer_schema(
            SettingsLayerKind::Global,
            &SettingsCatalogIds {
                lsp_server_ids: vec!["dev.axome.vtsls::vtsls".to_string()],
                harness_ids: vec!["omp".into()],
                ..SettingsCatalogIds::default()
            },
        )
        .to_string();

        for section in [
            "window",
            "typography",
            "nix",
            "explorer",
            "exclude_gitignore",
            "compact_folders",
            "formatting",
            "harnesses",
            "sessions",
            "shell",
            "lsp",
            "remotes",
        ] {
            assert!(rendered.contains(section), "schema includes {section}");
        }
        assert!(rendered.contains("\"omp\""));
        assert!(rendered.contains("dev.axome.vtsls::vtsls"));
    }

    #[test]
    fn settings_schema_allows_null_only_on_nullable_fields() {
        let schema = settings_layer_schema(
            SettingsLayerKind::Global,
            &SettingsCatalogIds {
                lsp_server_ids: vec!["dev.axome.vtsls::vtsls".to_string()],
                harness_ids: vec!["omp".into()],
                ..SettingsCatalogIds::default()
            },
        );
        let rendered = schema.to_string();

        assert!(rendered.contains("binary_path_override"));
        assert!(rendered.contains("\"string\",\"null\""));
        let typography = &schema["definitions"]["TypographySettings"]["properties"];
        assert_eq!(typography["code_font_size"]["type"], json!("integer"));
        assert_eq!(
            typography["terminal_font_size"]["type"],
            json!(["integer", "null"])
        );
    }
}
