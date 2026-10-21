use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const EXTENSION_API_VERSION: u32 = 1;
pub const EXTENSION_MANIFEST_FILE: &str = "extension.json";
pub const FORMATTER_LSP: &str = "lsp";
pub const FORMATTER_DISABLED: &str = "disabled";
/// The only argument variable; valid only in resume and new-session arguments.
pub const SESSION_ID_VARIABLE: &str = "${session_id}";

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
    #[serde(default)]
    pub harnesses: Vec<HarnessContribution>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessContribution {
    /// Stable harness identifier.
    pub id: String,
    /// Human-readable harness name.
    pub label: String,
    /// Command used for both the version probe and the terminal CLI.
    pub command: String,
    /// Installation guidance, falling back to the extension's hint when absent.
    #[serde(default)]
    pub install_hint: Option<String>,
    /// Brand assets and colors.
    pub brand: HarnessBrand,
    /// Version probe configuration.
    #[serde(default)]
    pub detect: HarnessDetect,
    /// Terminal launch and presentation configuration.
    #[serde(default)]
    pub terminal: HarnessTerminal,
    /// ACP agent for the Chat view; parsed and validated, not yet launched.
    #[serde(default)]
    pub chat: Option<HarnessChat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessBrand {
    /// Relative SVG payload path for the icon.
    pub icon: String,
    /// Optional relative SVG payload path for the wordmark.
    #[serde(default)]
    pub wordmark: Option<String>,
    /// Positive width-to-height ratio, required when a wordmark is supplied.
    #[serde(default)]
    pub wordmark_aspect: Option<f64>,
    /// Optional font family for a text wordmark.
    #[serde(default)]
    pub wordmark_font: Option<String>,
    /// Gradient endpoints as #rrggbb colors.
    pub gradient: [String; 2],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessDetect {
    /// Version probe arguments; variables are not allowed.
    #[serde(default = "default_version_args")]
    pub version_args: Vec<String>,
    /// Output stream used to read the version.
    #[serde(default)]
    pub version_stream: VersionStream,
}

fn default_version_args() -> Vec<String> {
    vec!["--version".to_string()]
}

impl Default for HarnessDetect {
    fn default() -> Self {
        Self {
            version_args: default_version_args(),
            version_stream: VersionStream::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VersionStream {
    #[default]
    Stdout,
    StdoutOrStderr,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct HarnessTerminal {
    /// Base terminal arguments; variables are not allowed.
    #[serde(default)]
    pub args: Vec<String>,
    /// Arguments inserted when the tab has a session id; only `${session_id}` is allowed.
    #[serde(default)]
    pub resume_args: Vec<String>,
    /// Non-empty arguments mint a UUID v4 session id up front; only `${session_id}` is allowed.
    #[serde(default)]
    pub new_session_args: Vec<String>,
    /// Shift-key encoding used by the terminal.
    #[serde(default)]
    pub shift_encoding: ShiftEncoding,
    /// Markers stripped from the title in order, trimming following whitespace;
    /// stripping stops at the first absent marker.
    #[serde(default)]
    pub title_prefixes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ShiftEncoding {
    #[default]
    Legacy,
    Kitty,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessChat {
    /// ACP agent command.
    pub command: String,
    /// ACP agent arguments; variables are not allowed.
    #[serde(default)]
    pub args: Vec<String>,
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
    pub harnesses: Vec<HarnessDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarnessDescriptor {
    /// Stable harness identifier.
    pub id: String,
    /// Human-readable harness name.
    pub label: String,
    /// Identifier of the contributing extension.
    pub extension_id: String,
    /// SVG markup.
    pub icon: String,
    /// Optional wordmark SVG markup.
    pub wordmark: Option<String>,
    /// Wordmark width-to-height ratio.
    pub wordmark_aspect: Option<f64>,
    /// Optional font family for a text wordmark.
    pub wordmark_font: Option<String>,
    /// Gradient endpoints as #rrggbb colors.
    pub gradient: [String; 2],
    /// Shift-key encoding used by the terminal.
    pub shift_encoding: ShiftEncoding,
    /// Markers stripped in order, trimming following whitespace and stopping at the first absent marker.
    pub title_prefixes: Vec<String>,
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
