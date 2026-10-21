use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const EXTENSION_API_VERSION: u32 = 1;
pub const EXTENSION_MANIFEST_FILE: &str = "extension.json";
pub const FORMATTER_LSP: &str = "lsp";
pub const FORMATTER_DISABLED: &str = "disabled";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifest {
    pub id: String,
    pub label: String,
    pub version: String,
    pub api: u32,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default)]
    pub install_hint: Option<String>,
    #[serde(default)]
    pub contributes: ExtensionContributions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ExtensionContributions {
    #[serde(default)]
    pub languages: Vec<LanguageContribution>,
    #[serde(default)]
    pub grammars: Vec<GrammarContribution>,
    #[serde(default)]
    pub lsp_servers: Vec<LspServerContribution>,
    #[serde(default)]
    pub formatters: Vec<FormatterContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageContribution {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub filenames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrammarContribution {
    pub language: String,
    pub path: String,
    #[serde(default)]
    pub embedded_languages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LspServerContribution {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub applies_to_languages: Vec<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub install_hint: Option<String>,
    #[serde(default)]
    pub document_selectors: Vec<LspDocumentSelector>,
    #[serde(default)]
    pub initialization_options: Option<Value>,
    #[serde(default)]
    pub settings: Option<LspServerSettingsDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatterContribution {
    pub id: String,
    pub label: String,
    pub languages: Vec<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub install_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct LspServerSettingsDescriptor {
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub defaults: Option<Value>,
    #[serde(default)]
    pub schema: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LspDocumentSelector {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default)]
    pub filenames: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionCatalog {
    pub runtime: ExtensionRuntimeCatalog,
    pub settings: ExtensionSettingsCatalog,
    pub diagnostics: Vec<ExtensionDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionRuntimeCatalog {
    pub languages: Vec<LanguageContribution>,
    pub formatter_policies: Vec<LanguageFormatterPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionSettingsCatalog {
    pub pages: Vec<SettingsPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPage {
    pub id: String,
    pub kind: SettingsPageKind,
    pub label: String,
    pub icon_filename: String,
    pub language_ids: Vec<String>,
    pub server_definition_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SettingsPageKind {
    Language,
    Nix,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageFormatterPolicy {
    pub language_id: String,
    pub language_label: String,
    pub options: Vec<FormatterOption>,
    pub default: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatterOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionDiagnostic {
    pub extension_id: Option<String>,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionGrammar {
    pub extension_id: String,
    pub language: String,
    pub embedded_languages: Vec<String>,
    pub grammar: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatterStatus {
    pub formatter_id: String,
    pub label: String,
    pub extension_id: String,
    pub extension_label: String,
    pub languages: Vec<String>,
    pub status: FormatterAvailability,
    pub resolved_path: Option<String>,
    pub message: Option<String>,
    pub install_hint: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FormatterAvailability {
    Available,
    Missing,
}
