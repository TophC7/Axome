use crate::services::{
    extensions::ExtensionService,
    settings::{SettingsJsoncLayer, SettingsService},
};
use jsonschema::{error::ValidationErrorKind, Validator};
use schemars::schema_for;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
};
use axome_protocol::extensions::{FORMATTER_DISABLED, FORMATTER_LSP};
use axome_protocol::settings::{
    EffectiveSettings, HarnessConfigRecord, HarnessSettings, LspServerConfigRecord,
    LspServerSettings, SettingsDiagnostic, SettingsDiagnosticCode, SettingsDiagnosticSeverity,
    SettingsLayerKind, SettingsOrigin,
};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsCatalogIds {
    pub lsp_server_ids: Vec<String>,
    pub formatter_ids: Vec<String>,
    pub formatting_defaults: BTreeMap<String, String>,
    pub harness_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsLayerLoad {
    Loaded(SettingsJsoncLayer),
    Invalid {
        layer: SettingsLayerKind,
        path: PathBuf,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedSettings {
    pub settings: EffectiveSettings,
    pub diagnostics: Vec<SettingsDiagnostic>,
}

pub fn resolve_effective_settings_for_folder_path(
    folder_path: Option<&Path>,
) -> Result<ResolvedSettings, String> {
    let catalog = ExtensionService::current();
    Ok(SettingsResolver::load(&catalog.settings_ids)?.resolve_folder_path(folder_path))
}

pub fn lsp_config_record(settings: &EffectiveSettings, server_id: &str) -> LspServerConfigRecord {
    let config = settings
        .lsp
        .servers
        .get(server_id)
        .cloned()
        .unwrap_or_else(LspServerSettings::default);
    LspServerConfigRecord {
        server_definition_id: server_id.to_string(),
        enabled: config.enabled,
        binary_path_override: config.binary_path_override,
        extra_args: config.extra_args,
        trace: config.trace,
        settings: config.settings,
    }
}

pub fn harness_config_record(
    settings: &EffectiveSettings,
    harness_id: &str,
) -> HarnessConfigRecord {
    let config = settings
        .harnesses
        .get(harness_id)
        .cloned()
        .unwrap_or_else(HarnessSettings::default);
    HarnessConfigRecord {
        harness_id: harness_id.to_string(),
        enabled: config.enabled,
        binary_path_override: config.binary_path_override,
        extra_args: config.extra_args,
    }
}

pub(crate) fn load_settings_layer(layer: SettingsLayerKind, path: PathBuf) -> SettingsLayerLoad {
    match SettingsService::read_jsonc_layer_or_empty(&path) {
        Ok(layer) => SettingsLayerLoad::Loaded(layer),
        Err(message) => SettingsLayerLoad::Invalid {
            layer,
            path,
            message,
        },
    }
}

pub fn parse_error_diagnostic(
    layer: SettingsLayerKind,
    path: &Path,
    message: String,
) -> SettingsDiagnostic {
    SettingsDiagnostic {
        layer,
        // Resolved by this process; a remote workspace's payload is retagged
        // `Host` when the desktop merges it.
        origin: SettingsOrigin::Desktop,
        path: path.to_string_lossy().into_owned(),
        pointer: String::new(),
        code: SettingsDiagnosticCode::ParseError,
        severity: SettingsDiagnosticSeverity::Error,
        message,
    }
}

/// Resolves defaults <- global <- folder. Each layer is validated against the
/// schema derived from `EffectiveSettings`; rejected nodes are stripped and
/// reported, so one bad field never poisons its siblings or the other layer.
/// Defaults <- global is validated once; folder layers then resolve on top without
/// re-reading global settings or rebuilding the global schema.
pub struct SettingsResolver<'a> {
    ids: &'a SettingsCatalogIds,
    base: Value,
    diagnostics: Vec<SettingsDiagnostic>,
    folder_validator: std::cell::OnceCell<Validator>,
}

impl<'a> SettingsResolver<'a> {
    pub fn new(global: SettingsLayerLoad, ids: &'a SettingsCatalogIds) -> Self {
        let global_schema = settings_layer_schema(SettingsLayerKind::Global, ids);
        let global_validator =
            jsonschema::validator_for(&global_schema).expect("global model schema is valid");
        let mut diagnostics = Vec::new();
        let mut base = serde_json::to_value(EffectiveSettings::with_catalog_defaults(
            &ids.lsp_server_ids,
            &ids.formatting_defaults,
            &ids.harness_ids,
        ))
        .expect("default settings serialize");
        if let Some(value) = validate_layer(
            &global_validator,
            SettingsLayerKind::Global,
            global,
            &mut diagnostics,
        ) {
            merge_layer(&mut base, value);
        }
        Self {
            ids,
            base,
            diagnostics,
            folder_validator: std::cell::OnceCell::new(),
        }
    }

    pub fn load(ids: &'a SettingsCatalogIds) -> Result<Self, String> {
        let global_path = SettingsService::global_settings_path()?;
        Ok(Self::new(
            load_settings_layer(SettingsLayerKind::Global, global_path),
            ids,
        ))
    }

    pub fn resolve(&self, folder: Option<SettingsLayerLoad>) -> ResolvedSettings {
        let ids = self.ids;
        let mut merged = self.base.clone();
        let mut diagnostics = self.diagnostics.clone();
        if let Some(folder_load) = folder {
            let folder_validator = self.folder_validator.get_or_init(|| {
                jsonschema::validator_for(&settings_layer_schema(SettingsLayerKind::Folder, ids))
                    .expect("folder model schema is valid")
            });
            if let Some(value) = validate_layer(
                folder_validator,
                SettingsLayerKind::Folder,
                folder_load,
                &mut diagnostics,
            ) {
                merge_layer(&mut merged, value);
            }
        }

        let settings = serde_json::from_value(merged).unwrap_or_else(|error| {
            // Bug guard: validation already stripped everything the model rejects.
            diagnostics.push(parse_error_diagnostic(
                SettingsLayerKind::Global,
                Path::new(""),
                format!("Settings model rejected validated layers: {error}"),
            ));
            EffectiveSettings::with_catalog_defaults(
                &ids.lsp_server_ids,
                &ids.formatting_defaults,
                &ids.harness_ids,
            )
        });

        ResolvedSettings {
            settings,
            diagnostics,
        }
    }

    pub fn resolve_folder_path(&self, folder_path: Option<&Path>) -> ResolvedSettings {
        self.resolve(folder_path.map(|folder_path| {
            load_settings_layer(
                SettingsLayerKind::Folder,
                SettingsService::folder_settings_path(folder_path),
            )
        }))
    }
}

/// Validates one layer, appending a diagnostic per rejected node and removing
/// those nodes from the returned value. `None` means the layer contributes
/// nothing (missing file, parse failure, or a non-object root).
fn validate_layer(
    validator: &Validator,
    kind: SettingsLayerKind,
    load: SettingsLayerLoad,
    diagnostics: &mut Vec<SettingsDiagnostic>,
) -> Option<Value> {
    let layer = match load {
        SettingsLayerLoad::Invalid {
            layer,
            path,
            message,
        } => {
            diagnostics.push(parse_error_diagnostic(layer, &path, message));
            return None;
        }
        SettingsLayerLoad::Loaded(layer) if layer.loaded => layer,
        SettingsLayerLoad::Loaded(_) => return None,
    };

    let mut rejected: Vec<(String, SettingsDiagnosticCode, String)> = Vec::new();
    for error in validator.iter_errors(&layer.value) {
        let at = error.instance_path().as_str();
        match error.kind() {
            ValidationErrorKind::AdditionalProperties { unexpected } => {
                rejected.extend(unexpected.iter().map(|key| {
                    let pointer = format!("{at}{}", json_pointer(&[key]));
                    let message = if kind == SettingsLayerKind::Folder
                        && is_global_only_pointer(&pointer)
                    {
                        format!(
                            "Setting `{key}` is window-scoped and can only be configured in global settings"
                        )
                    } else {
                        format!("Unknown setting `{key}`")
                    };
                    (pointer, SettingsDiagnosticCode::UnknownKey, message)
                }));
            }
            // Harnesses, lsp.servers, and formatting use catalog ids; presets
            // use a name pattern, all pinned in the schema as `propertyNames`.
            ValidationErrorKind::PropertyNames { error: inner } => {
                let key = inner.instance().as_str().unwrap_or_default().to_string();
                if at.ends_with("/presets") {
                    rejected.push((
                        format!("{at}{}", json_pointer(&[&key])),
                        SettingsDiagnosticCode::InvalidValue,
                        format!("Invalid preset name `{key}`: use letters, digits, '-' and '_'"),
                    ));
                    continue;
                }
                let noun = if at == "/harnesses" {
                    "harness"
                } else if at == "/formatting" {
                    "formatting language"
                } else {
                    "LSP server"
                };
                rejected.push((
                    format!("{at}{}", json_pointer(&[&key])),
                    SettingsDiagnosticCode::UnknownKey,
                    format!("Unknown {noun} `{key}`"),
                ));
            }
            _ => rejected.push((
                at.to_string(),
                SettingsDiagnosticCode::InvalidValue,
                error.to_string(),
            )),
        }
    }

    let mut seen = HashSet::new();
    rejected.retain(|(pointer, code, _)| seen.insert((pointer.clone(), *code)));

    let path = layer.path.to_string_lossy().into_owned();
    let root_rejected = rejected.iter().any(|(pointer, ..)| pointer.is_empty());
    let mut value = layer.value;
    for (pointer, code, message) in rejected {
        remove_at(&mut value, &pointer);
        diagnostics.push(SettingsDiagnostic {
            layer: kind,
            origin: SettingsOrigin::Desktop,
            path: path.clone(),
            severity: if pointer.is_empty() {
                SettingsDiagnosticSeverity::Error
            } else {
                SettingsDiagnosticSeverity::Warning
            },
            pointer,
            code,
            message,
        });
    }

    (!root_rejected).then_some(value)
}

/// Drops the node at `pointer`. An invalid element invalidates the list that
/// holds it, so the walk stops at the deepest object ancestor: an error at
/// `/harnesses/codex/extra_args/1` removes `extra_args`.
fn remove_at(value: &mut Value, pointer: &str) {
    let segments = pointer
        .split('/')
        .skip(1)
        .map(|segment| segment.replace("~1", "/").replace("~0", "~"))
        .collect::<Vec<_>>();
    let mut node = value;
    for (index, segment) in segments.iter().enumerate() {
        let Some(object) = node.as_object_mut() else {
            return;
        };
        if index + 1 == segments.len() || !object.get(segment).is_some_and(Value::is_object) {
            object.remove(segment);
            return;
        }
        node = object.get_mut(segment).expect("key checked above");
    }
}

/// Object + object merges key by key; anything else replaces, so arrays and
/// explicit `null`s override the layer below.
fn merge_layer(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(existing) => merge_layer(existing, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

fn json_pointer(parts: &[&str]) -> String {
    let mut pointer = String::new();
    for part in parts {
        pointer.push('/');
        pointer.push_str(&part.replace('~', "~0").replace('/', "~1"));
    }
    pointer
}

/// JSON Pointer prefixes of settings that are strictly `GlobalOnly` and cannot be
/// configured in project folder settings.
pub(crate) const GLOBAL_ONLY_POINTERS: &[&str] = &["/window", "/typography", "/remotes"];

pub(crate) fn is_global_only_pointer(pointer: &str) -> bool {
    GLOBAL_ONLY_POINTERS.iter().any(|prefix| {
        pointer == *prefix
            || (pointer.starts_with(prefix) && pointer[prefix.len()..].starts_with('/'))
    })
}

/// Runtime + editor JSON Schema for a settings layer file. Derived from the
/// model; catalog-driven map keys and formatter ids are injected because they
/// come from runtime catalogs, not types.
///
/// When generated for `SettingsLayerKind::Folder`, settings with global-only scope
/// (defined in `GLOBAL_ONLY_POINTERS`) are pruned from the schema so that editors omit
/// them from autocomplete and layer validation rejects them.
pub(crate) fn settings_layer_schema(layer: SettingsLayerKind, ids: &SettingsCatalogIds) -> Value {
    let mut schema =
        serde_json::to_value(schema_for!(EffectiveSettings)).expect("settings schema serializes");
    restrict_map_keys(&mut schema, &["harnesses"], ids.harness_ids.clone());
    for presets_pointer in [
        "/definitions/HarnessSettings/properties/presets",
        "/definitions/ShellSettings/properties/presets",
    ] {
        schema
            .pointer_mut(presets_pointer)
            .and_then(Value::as_object_mut)
            .unwrap_or_else(|| panic!("settings schema exposes {presets_pointer}"))
            .insert(
                "propertyNames".to_string(),
                json!({ "pattern": "^[A-Za-z0-9_-]+$" }),
            );
    }
    restrict_map_keys(&mut schema, &["lsp", "servers"], ids.lsp_server_ids.clone());
    restrict_map_keys(
        &mut schema,
        &["formatting"],
        ids.formatting_defaults.keys().cloned().collect(),
    );
    let formatter_pointer = "/definitions/FormattingLanguageSettings/properties/formatter";
    let formatter_ids: Vec<&str> = ids
        .formatter_ids
        .iter()
        .map(String::as_str)
        .chain([FORMATTER_LSP, FORMATTER_DISABLED])
        .collect();
    schema
        .pointer_mut(formatter_pointer)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("settings schema exposes {formatter_pointer}"))
        .insert("enum".to_string(), json!(formatter_ids));

    if layer == SettingsLayerKind::Folder {
        for pointer in GLOBAL_ONLY_POINTERS {
            let segments: Vec<&str> = pointer.trim_start_matches('/').split('/').collect();
            if segments.len() == 1 {
                if let Some(props) = schema
                    .pointer_mut("/properties")
                    .and_then(Value::as_object_mut)
                {
                    props.remove(segments[0]);
                }
            } else {
                let field_path = &segments[..segments.len() - 1];
                let prop_name = segments.last().unwrap();
                let p = resolve_schema_pointer(&schema, field_path);
                if let Some(props) = schema
                    .pointer_mut(&format!("{p}/properties"))
                    .and_then(Value::as_object_mut)
                {
                    props.remove(*prop_name);
                }
            }
        }
    }

    schema
}

/// Pins the accepted key set of a map-valued setting. Struct-typed fields are
/// `$ref`s into `definitions` (wrapped in `allOf` when schemars attaches field
/// metadata), so each step dereferences before descending.
pub(crate) fn restrict_map_keys(schema: &mut Value, field_path: &[&str], keys: Vec<String>) {
    let pointer = resolve_schema_pointer(schema, field_path);
    schema
        .pointer_mut(&pointer)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("settings schema exposes {pointer}"))
        .insert("propertyNames".to_string(), json!({ "enum": keys }));
}

pub(crate) fn resolve_schema_pointer(schema: &Value, field_path: &[&str]) -> String {
    let mut pointer = String::new();
    for field in field_path {
        while let Some(reference) = schema.pointer(&pointer).and_then(field_ref) {
            pointer = reference.trim_start_matches('#').to_string();
        }
        pointer.push_str("/properties/");
        pointer.push_str(field);
    }
    while let Some(reference) = schema.pointer(&pointer).and_then(field_ref) {
        pointer = reference.trim_start_matches('#').to_string();
    }
    pointer
}

pub(crate) fn field_ref(node: &Value) -> Option<&str> {
    node.get("$ref")
        .or_else(|| node.pointer("/allOf/0/$ref"))?
        .as_str()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use axome_protocol::settings::TabBeamPosition;
    #[test]
    fn global_schema_includes_global_only_settings() {
        let schema =
            settings_layer_schema(SettingsLayerKind::Global, &SettingsCatalogIds::default());
        assert!(schema.pointer("/properties/window").is_some());
        assert!(schema.pointer("/properties/remotes").is_some());
    }

    #[test]
    fn folder_schema_prunes_global_only_settings() {
        let schema =
            settings_layer_schema(SettingsLayerKind::Folder, &SettingsCatalogIds::default());
        assert!(schema.pointer("/properties/window").is_none());
        assert!(schema.pointer("/properties/typography").is_none());
        assert!(schema.pointer("/properties/remotes").is_none());
        assert!(schema.pointer("/properties/nix").is_some());
    }

    fn loaded(layer: SettingsLayerKind, name: &str, value: Value) -> SettingsLayerLoad {
        SettingsLayerLoad::Loaded(SettingsJsoncLayer {
            path: PathBuf::from(format!("/{layer:?}/{name}.jsonc")),
            loaded: true,
            value,
        })
    }

    fn missing(layer: SettingsLayerKind, name: &str) -> SettingsLayerLoad {
        SettingsLayerLoad::Loaded(SettingsJsoncLayer {
            path: PathBuf::from(format!("/{layer:?}/{name}.jsonc")),
            loaded: false,
            value: json!({}),
        })
    }

    fn invalid(layer: SettingsLayerKind, name: &str) -> SettingsLayerLoad {
        SettingsLayerLoad::Invalid {
            layer,
            path: PathBuf::from(format!("/{layer:?}/{name}.jsonc")),
            message: "parse exploded".to_string(),
        }
    }

    fn resolve(global: SettingsLayerLoad, project: Option<SettingsLayerLoad>) -> ResolvedSettings {
        let ids = SettingsCatalogIds {
            lsp_server_ids: vec!["dev.axome.vtsls::vtsls".to_string()],
            harness_ids: vec!["claude_code".into()],
            formatter_ids: vec![
                "dev.axome.biome::biome".to_string(),
                "dev.axome.nix::nixfmt".to_string(),
            ],
            formatting_defaults: BTreeMap::from([
                ("json".to_string(), "dev.axome.biome::biome".to_string()),
                ("nix".to_string(), "dev.axome.nix::nixfmt".to_string()),
            ]),
        };
        SettingsResolver::new(global, &ids).resolve(project)
    }

    #[test]
    fn missing_layers_resolve_defaults() {
        let resolved = resolve(missing(SettingsLayerKind::Global, "global"), None);

        assert_eq!(resolved.settings.typography.code_font_size, 13);
        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn valid_global_overrides_defaults() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({ "typography": { "code_font_size": 15 } }),
            ),
            None,
        );

        assert_eq!(resolved.settings.typography.code_font_size, 15);
        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn valid_folder_overrides_global() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({ "nix": { "eval_timeout_secs": 900 } }),
            ),
            Some(loaded(
                SettingsLayerKind::Folder,
                "folder",
                json!({ "nix": { "eval_timeout_secs": 1200 } }),
            )),
        );

        assert_eq!(resolved.settings.nix.eval_timeout_secs, 1200);
    }

    #[test]
    fn window_tab_beam_position_round_trips_in_global() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({ "window": { "tab_beam_position": "bottom" } }),
            ),
            None,
        );

        assert_eq!(
            resolved.settings.window.tab_beam_position,
            TabBeamPosition::Bottom
        );
        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn folder_layer_rejects_global_only_settings_with_window_scoped_diagnostics() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({
                    "window": { "tab_beam_position": "bottom" },
                    "typography": { "code_font_size": 15 },
                    "remotes": {
                        "home": {
                            "address": "global.example:7420",
                            "fingerprint": "SHA256:00"
                        }
                    }
                }),
            ),
            Some(loaded(
                SettingsLayerKind::Folder,
                "folder",
                json!({
                    "window": { "tab_beam_position": "top" },
                    "typography": { "code_font_size": 20 },
                    "remotes": {
                        "home": {
                            "address": "folder.example:7420",
                            "fingerprint": "SHA256:11"
                        }
                    }
                }),
            )),
        );

        assert_eq!(
            resolved.settings.window.tab_beam_position,
            TabBeamPosition::Bottom
        );
        assert_eq!(resolved.settings.typography.code_font_size, 15);
        let remote = &resolved.settings.remotes["home"];
        assert_eq!(remote.address, "global.example:7420");
        assert_eq!(remote.fingerprint, "SHA256:00");
        assert_eq!(resolved.diagnostics.len(), 3);
        for pointer in ["/window", "/typography", "/remotes"] {
            assert!(resolved.diagnostics.iter().any(|diagnostic| {
                diagnostic.code == SettingsDiagnosticCode::UnknownKey
                    && diagnostic.pointer == pointer
                    && diagnostic.message.contains("window-scoped")
            }));
        }
    }

    #[test]
    fn invalid_global_fails_only_global_layer() {
        let resolved = resolve(
            invalid(SettingsLayerKind::Global, "global"),
            Some(loaded(
                SettingsLayerKind::Folder,
                "folder",
                json!({ "nix": { "eval_timeout_secs": 1200 } }),
            )),
        );

        assert_eq!(resolved.settings.nix.eval_timeout_secs, 1200);
        assert_eq!(
            resolved.diagnostics[0].code,
            SettingsDiagnosticCode::ParseError
        );
        assert_eq!(resolved.diagnostics[0].layer, SettingsLayerKind::Global);
    }

    #[test]
    fn invalid_folder_fails_only_folder_layer() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({ "typography": { "code_font_size": 16 } }),
            ),
            Some(invalid(SettingsLayerKind::Folder, "folder")),
        );

        assert_eq!(resolved.settings.typography.code_font_size, 16);
        assert_eq!(
            resolved.diagnostics[0].code,
            SettingsDiagnosticCode::ParseError
        );
        assert_eq!(resolved.diagnostics[0].layer, SettingsLayerKind::Folder);
    }

    #[test]
    fn object_settings_merge_recursively_and_arrays_replace() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({
                    "harnesses": {
                        "claude_code": { "extra_args": ["--global"] }
                    },
                    "lsp": {
                        "servers": {
                            "dev.axome.vtsls::vtsls": {
                                "settings": {
                                    "typescript": { "preferences": { "importModuleSpecifier": "relative" } },
                                    "array": ["global"]
                                }
                            }
                        }
                    }
                }),
            ),
            Some(loaded(
                SettingsLayerKind::Folder,
                "folder",
                json!({
                    "harnesses": {
                        "claude_code": { "extra_args": ["--project"] }
                    },
                    "lsp": {
                        "servers": {
                            "dev.axome.vtsls::vtsls": {
                                "settings": {
                                    "typescript": { "preferences": { "quoteStyle": "single" } },
                                    "array": ["project"]
                                }
                            }
                        }
                    }
                }),
            )),
        );

        assert_eq!(
            resolved.settings.harnesses["claude_code"].extra_args,
            vec!["--project"]
        );
        let lsp_settings = resolved.settings.lsp.servers["dev.axome.vtsls::vtsls"]
            .settings
            .as_ref()
            .expect("lsp settings merged");
        assert_eq!(
            lsp_settings["typescript"]["preferences"]["importModuleSpecifier"],
            json!("relative")
        );
        assert_eq!(
            lsp_settings["typescript"]["preferences"]["quoteStyle"],
            json!("single")
        );
        assert_eq!(lsp_settings["array"], json!(["project"]));
        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn nullable_null_applies_and_invalid_non_nullable_null_is_ignored() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({
                    "harnesses": {
                        "claude_code": {
                            "enabled": null,
                            "binary_path_override": "/bin/claude"
                        }
                    }
                }),
            ),
            Some(loaded(
                SettingsLayerKind::Folder,
                "folder",
                json!({
                    "harnesses": {
                        "claude_code": {
                            "binary_path_override": null
                        }
                    }
                }),
            )),
        );

        let harness = &resolved.settings.harnesses["claude_code"];
        assert!(harness.enabled);
        assert_eq!(harness.binary_path_override, None);
        assert!(resolved.diagnostics.iter().any(|diagnostic| diagnostic.code
            == SettingsDiagnosticCode::InvalidValue
            && diagnostic.pointer == "/harnesses/claude_code/enabled"));
    }

    #[test]
    fn unknown_harness_and_lsp_ids_emit_diagnostics_and_are_ignored() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({
                    "harnesses": { "missing_harness": { "enabled": false } },
                    "lsp": { "servers": { "missing::server": { "enabled": false } } }
                }),
            ),
            None,
        );

        assert!(resolved.settings.harnesses["claude_code"].enabled);
        assert!(resolved.settings.lsp.servers["dev.axome.vtsls::vtsls"].enabled);
        assert!(!resolved.settings.harnesses.contains_key("missing_harness"));
        assert!(!resolved
            .settings
            .lsp
            .servers
            .contains_key("missing::server"));
        for pointer in ["/harnesses/missing_harness", "/lsp/servers/missing::server"] {
            assert!(
                resolved.diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == SettingsDiagnosticCode::UnknownKey
                        && diagnostic.pointer == pointer
                }),
                "diagnostic for {pointer}"
            );
        }
    }

    #[test]
    fn semantic_errors_ignore_bad_field_and_apply_rest_of_layer() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({
                    "typography": {
                        "code_font_size": "big",
                        "code_font_family": "Iosevka"
                    },
                    "formatting": {
                        "json": { "formatter": "weird" },
                        "nix": { "formatter": "disabled" }
                    }
                }),
            ),
            None,
        );

        assert_eq!(resolved.settings.typography.code_font_size, 13);
        assert_eq!(resolved.settings.typography.code_font_family, "Iosevka");
        assert_eq!(
            resolved.settings.formatting["json"].formatter,
            "dev.axome.biome::biome"
        );
        assert_eq!(resolved.settings.formatting["nix"].formatter, "disabled");
        for pointer in ["/typography/code_font_size", "/formatting/json/formatter"] {
            assert!(
                resolved.diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == SettingsDiagnosticCode::InvalidValue
                        && diagnostic.pointer == pointer
                }),
                "diagnostic for {pointer}"
            );
        }
    }

    #[test]
    fn explorer_exclude_merges_with_defaults() {
        let resolved = resolve(
            loaded(SettingsLayerKind::Global, "global", json!({})),
            Some(loaded(
                SettingsLayerKind::Folder,
                "folder",
                json!({
                    "explorer": {
                        "exclude": { "**/vendor": true, "**/.git": false },
                        "exclude_gitignore": true,
                        "compact_folders": false
                    }
                }),
            )),
        );

        let exclude = &resolved.settings.explorer.exclude;
        assert!(exclude["**/vendor"]);
        assert!(!exclude["**/.git"]);
        assert!(exclude["**/.DS_Store"]);
        assert!(resolved.settings.explorer.exclude_gitignore);
        assert!(!resolved.settings.explorer.compact_folders);
        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn worktree_copy_layers_preserve_overlapping_vetoes() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({
                    "git": { "worktree_copy": { ".local": true, ".local/private": true, "**/*.env": true } }
                }),
            ),
            Some(loaded(
                SettingsLayerKind::Folder,
                "folder",
                json!({
                    "git": { "worktree_copy": { ".local/private": false, ".env": true } }
                }),
            )),
        );
        assert_eq!(
            resolved.settings.git.worktree_copy,
            std::collections::BTreeMap::from([
                (".local".to_owned(), true),
                (".local/private".to_owned(), false),
                ("**/*.env".to_owned(), true),
                (".env".to_owned(), true),
            ])
        );
        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn explorer_unknown_key_emits_diagnostic() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({ "explorer": { "excludeGitIgnore": true } }),
            ),
            None,
        );

        assert!(!resolved.settings.explorer.exclude_gitignore);
        assert!(resolved.diagnostics.iter().any(|diagnostic| diagnostic.code
            == SettingsDiagnosticCode::UnknownKey
            && diagnostic.pointer == "/explorer/excludeGitIgnore"));
    }

    #[test]
    fn explorer_non_bool_exclude_value_is_ignored_with_diagnostic() {
        let resolved = resolve(
            loaded(
                SettingsLayerKind::Global,
                "global",
                json!({
                    "explorer": {
                        "exclude": { "**/vendor": "yes", "**/gen": true },
                        "compact_folders": false
                    }
                }),
            ),
            None,
        );

        let exclude = &resolved.settings.explorer.exclude;
        assert!(exclude["**/gen"]);
        assert!(!exclude.contains_key("**/vendor"));
        assert!(!resolved.settings.explorer.compact_folders);
        assert!(resolved.diagnostics.iter().any(|diagnostic| diagnostic.code
            == SettingsDiagnosticCode::InvalidValue
            && diagnostic.pointer == "/explorer/exclude/**~1vendor"));
    }
    #[test]
    fn invalid_preset_name_is_rejected() {
        let ids = SettingsCatalogIds {
            harness_ids: vec!["omp".into()],
            ..Default::default()
        };
        let resolved = SettingsResolver::new(missing(SettingsLayerKind::Global, "global"), &ids)
            .resolve(Some(loaded(
                SettingsLayerKind::Folder,
                "folder",
                json!({ "harnesses": { "omp": { "presets": {
                    "bad name": {},
                    "web": { "args": ["--profile", "web"] }
                } } } }),
            )));
        assert_eq!(resolved.diagnostics.len(), 1);
        let diagnostic = &resolved.diagnostics[0];
        assert_eq!(diagnostic.code, SettingsDiagnosticCode::InvalidValue);
        assert_eq!(
            diagnostic.pointer,
            json_pointer(&["harnesses", "omp", "presets", "bad name"])
        );
        assert_eq!(
            diagnostic.message,
            "Invalid preset name `bad name`: use letters, digits, '-' and '_'"
        );
        let presets = &resolved.settings.harnesses["omp"].presets;
        assert!(!presets.contains_key("bad name"));
        assert_eq!(presets["web"].args, vec!["--profile", "web"]);
    }
}
