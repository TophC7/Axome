use parking_lot::RwLock;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, OnceLock};
use sworm_protocol::extensions::*;

use super::settings::SettingsService;
use super::settings_resolution::SettingsCatalogIds;

struct BundledExtension {
    id: &'static str,
    manifest: &'static str,
    payload: &'static [(&'static str, &'static str)],
}

const BUNDLED: &[BundledExtension] = &[
    BundledExtension {
        id: "dev.sworm.biome",
        manifest: include_str!("../../extensions/dev.sworm.biome/extension.json"),
        payload: &[(
            "settings.schema.json",
            include_str!("../../extensions/dev.sworm.biome/settings.schema.json"),
        )],
    },
    BundledExtension {
        id: "dev.sworm.css",
        manifest: include_str!("../../extensions/dev.sworm.css/extension.json"),
        payload: &[
            (
                "settings.schema.json",
                include_str!("../../extensions/dev.sworm.css/settings.schema.json"),
            ),
            (
                "grammars/css.tmLanguage.json",
                include_str!("../../extensions/dev.sworm.css/grammars/css.tmLanguage.json"),
            ),
            (
                "grammars/postcss.tmLanguage.json",
                include_str!("../../extensions/dev.sworm.css/grammars/postcss.tmLanguage.json"),
            ),
        ],
    },
    BundledExtension {
        id: "dev.sworm.html",
        manifest: include_str!("../../extensions/dev.sworm.html/extension.json"),
        payload: &[(
            "settings.schema.json",
            include_str!("../../extensions/dev.sworm.html/settings.schema.json"),
        )],
    },
    BundledExtension {
        id: "dev.sworm.fish",
        manifest: include_str!("../../extensions/dev.sworm.fish/extension.json"),
        payload: &[(
            "grammars/fish.tmLanguage.json",
            include_str!("../../extensions/dev.sworm.fish/grammars/fish.tmLanguage.json"),
        )],
    },
    BundledExtension {
        id: "dev.sworm.go",
        manifest: include_str!("../../extensions/dev.sworm.go/extension.json"),
        payload: &[
            (
                "settings.schema.json",
                include_str!("../../extensions/dev.sworm.go/settings.schema.json"),
            ),
            (
                "grammars/go.tmLanguage.json",
                include_str!("../../extensions/dev.sworm.go/grammars/go.tmLanguage.json"),
            ),
        ],
    },
    BundledExtension {
        id: "dev.sworm.nix",
        manifest: include_str!("../../extensions/dev.sworm.nix/extension.json"),
        payload: &[
            (
                "settings.schema.json",
                include_str!("../../extensions/dev.sworm.nix/settings.schema.json"),
            ),
            (
                "grammars/nix.tmLanguage.json",
                include_str!("../../extensions/dev.sworm.nix/grammars/nix.tmLanguage.json"),
            ),
        ],
    },
    BundledExtension {
        id: "dev.sworm.pyright",
        manifest: include_str!("../../extensions/dev.sworm.pyright/extension.json"),
        payload: &[(
            "settings.schema.json",
            include_str!("../../extensions/dev.sworm.pyright/settings.schema.json"),
        )],
    },
    BundledExtension {
        id: "dev.sworm.rust",
        manifest: include_str!("../../extensions/dev.sworm.rust/extension.json"),
        payload: &[(
            "settings.schema.json",
            include_str!("../../extensions/dev.sworm.rust/settings.schema.json"),
        )],
    },
    BundledExtension {
        id: "dev.sworm.svelte",
        manifest: include_str!("../../extensions/dev.sworm.svelte/extension.json"),
        payload: &[
            (
                "settings.schema.json",
                include_str!("../../extensions/dev.sworm.svelte/settings.schema.json"),
            ),
            (
                "grammars/svelte.tmLanguage.json",
                include_str!("../../extensions/dev.sworm.svelte/grammars/svelte.tmLanguage.json"),
            ),
        ],
    },
    BundledExtension {
        id: "dev.sworm.tailwind",
        manifest: include_str!("../../extensions/dev.sworm.tailwind/extension.json"),
        payload: &[(
            "settings.schema.json",
            include_str!("../../extensions/dev.sworm.tailwind/settings.schema.json"),
        )],
    },
    BundledExtension {
        id: "dev.sworm.vtsls",
        manifest: include_str!("../../extensions/dev.sworm.vtsls/extension.json"),
        payload: &[
            (
                "settings.schema.json",
                include_str!("../../extensions/dev.sworm.vtsls/settings.schema.json"),
            ),
            (
                "grammars/javascript.tmLanguage.json",
                include_str!(
                    "../../extensions/dev.sworm.vtsls/grammars/javascript.tmLanguage.json"
                ),
            ),
            (
                "grammars/typescript.tmLanguage.json",
                include_str!(
                    "../../extensions/dev.sworm.vtsls/grammars/typescript.tmLanguage.json"
                ),
            ),
        ],
    },
    BundledExtension {
        id: "dev.sworm.claude-code",
        manifest: include_str!("../../extensions/dev.sworm.claude-code/extension.json"),
        payload: &[
            (
                "icons/claudecode.svg",
                include_str!("../../extensions/dev.sworm.claude-code/icons/claudecode.svg"),
            ),
            (
                "icons/claudecode-text.svg",
                include_str!("../../extensions/dev.sworm.claude-code/icons/claudecode-text.svg"),
            ),
        ],
    },
    BundledExtension {
        id: "dev.sworm.codex",
        manifest: include_str!("../../extensions/dev.sworm.codex/extension.json"),
        payload: &[
            (
                "icons/codex.svg",
                include_str!("../../extensions/dev.sworm.codex/icons/codex.svg"),
            ),
            (
                "icons/codex-text.svg",
                include_str!("../../extensions/dev.sworm.codex/icons/codex-text.svg"),
            ),
        ],
    },
    BundledExtension {
        id: "dev.sworm.omp",
        manifest: include_str!("../../extensions/dev.sworm.omp/extension.json"),
        payload: &[(
            "icons/omp.svg",
            include_str!("../../extensions/dev.sworm.omp/icons/omp.svg"),
        )],
    },
    BundledExtension {
        id: "dev.sworm.antigravity",
        manifest: include_str!("../../extensions/dev.sworm.antigravity/extension.json"),
        payload: &[
            (
                "icons/antigravity.svg",
                include_str!("../../extensions/dev.sworm.antigravity/icons/antigravity.svg"),
            ),
            (
                "icons/antigravity-text.svg",
                include_str!("../../extensions/dev.sworm.antigravity/icons/antigravity-text.svg"),
            ),
        ],
    },
];

#[derive(Clone, Copy)]
struct PageOverlay {
    id: &'static str,
    kind: SettingsPageKind,
    label: &'static str,
    icon_filename: &'static str,
    language_ids: &'static [&'static str],
}

const PAGE_OVERLAYS: &[PageOverlay] = &[
    PageOverlay {
        id: "language:javascript-typescript",
        kind: SettingsPageKind::Language,
        label: "JS / TypeScript",
        icon_filename: "index.ts",
        language_ids: &["javascript", "typescript"],
    },
    PageOverlay {
        id: "language:json",
        kind: SettingsPageKind::Language,
        label: "JSON",
        icon_filename: "data.json",
        language_ids: &["json"],
    },
    PageOverlay {
        id: "language:css",
        kind: SettingsPageKind::Language,
        label: "CSS",
        icon_filename: "index.css",
        language_ids: &["css", "scss", "less"],
    },
    PageOverlay {
        id: "language:nix",
        kind: SettingsPageKind::Nix,
        label: "Nix",
        icon_filename: "flake.nix",
        language_ids: &["nix"],
    },
    PageOverlay {
        id: "language:go",
        kind: SettingsPageKind::Language,
        label: "Go",
        icon_filename: "main.go",
        language_ids: &["go"],
    },
];

#[derive(Debug)]
pub struct LoadedLspServer {
    pub server_definition_id: String,
    pub extension_id: String,
    pub extension_label: String,
    pub extension_dir: Option<PathBuf>,
    pub requires: Vec<String>,
    pub install_hint: String,
    pub definition: LspServerContribution,
}

#[derive(Debug)]
pub struct LoadedFormatter {
    pub formatter_id: String,
    pub extension_id: String,
    pub extension_label: String,
    pub extension_dir: Option<PathBuf>,
    pub requires: Vec<String>,
    pub install_hint: String,
    pub definition: FormatterContribution,
}

#[derive(Debug)]
pub struct LoadedHarness {
    pub extension_id: String,
    pub extension_label: String,
    pub extension_dir: Option<PathBuf>,
    pub requires: Vec<String>,
    pub install_hint: String,
    pub definition: HarnessContribution,
}

#[derive(Debug)]
pub struct LoadedExtensionCatalog {
    pub catalog: ExtensionCatalog,
    pub lsp_servers: Vec<LoadedLspServer>,
    pub formatters: Vec<LoadedFormatter>,
    pub harnesses: Vec<LoadedHarness>,
    pub grammars: Vec<ExtensionGrammar>,
    pub settings_ids: SettingsCatalogIds,
}

impl LoadedExtensionCatalog {
    pub fn find_lsp_server(&self, id: &str) -> Option<&LoadedLspServer> {
        self.lsp_servers
            .iter()
            .find(|server| server.server_definition_id == id)
    }

    pub fn find_formatter(&self, id: &str) -> Option<&LoadedFormatter> {
        self.formatters
            .iter()
            .find(|formatter| formatter.formatter_id == id)
    }

    pub fn find_harness(&self, id: &str) -> Option<&LoadedHarness> {
        self.harnesses
            .iter()
            .find(|harness| harness.definition.id == id)
    }
}

struct PreparedExtension {
    manifest: ExtensionManifest,
    dir: Option<PathBuf>,
    grammars: Vec<ExtensionGrammar>,
    harnesses: Vec<HarnessDescriptor>,
}

static INSTALLED_DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
static CURRENT: LazyLock<RwLock<Arc<LoadedExtensionCatalog>>> =
    LazyLock::new(|| RwLock::new(Arc::new(load_catalog(installed_dir()))));

pub struct ExtensionService;

impl ExtensionService {
    /// Fix the installed-extensions directory before the first catalog read.
    /// Returns false if already fixed.
    pub fn set_installed_dir(dir: PathBuf) -> bool {
        INSTALLED_DIR.set(Some(dir)).is_ok()
    }

    pub fn current() -> Arc<LoadedExtensionCatalog> {
        CURRENT.read().clone()
    }

    pub fn reload() -> Arc<LoadedExtensionCatalog> {
        let catalog = Arc::new(load_catalog(installed_dir()));
        *CURRENT.write() = catalog.clone();
        catalog
    }

    pub fn server_definition_id(extension_id: &str, contribution_id: &str) -> String {
        format!("{extension_id}::{contribution_id}")
    }
}

fn installed_dir() -> Option<&'static Path> {
    INSTALLED_DIR
        .get_or_init(|| {
            SettingsService::global_data_dir()
                .ok()
                .map(|dir| dir.join("extensions"))
        })
        .as_deref()
}

fn parse_bundled(asset: &BundledExtension) -> Result<PreparedExtension, String> {
    let manifest: ExtensionManifest =
        serde_json::from_str(asset.manifest).map_err(|error| error.to_string())?;
    if manifest.id != asset.id {
        return Err(format!(
            "Extension manifest id {} does not match bundled id {}",
            manifest.id, asset.id
        ));
    }
    prepare(manifest, None, asset.payload)
}

pub fn load_catalog(installed_dir: Option<&Path>) -> LoadedExtensionCatalog {
    // Contribution commands resolve inside the extension dir but spawn with the
    // folder as cwd, so a relative root would point at the wrong place.
    let installed_dir =
        installed_dir.map(|dir| std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf()));
    let installed_dir = installed_dir.as_deref();
    let mut list: Vec<_> = BUNDLED
        .iter()
        .map(|asset| {
            parse_bundled(asset).unwrap_or_else(|error| {
                panic!("bundled extension {} is invalid: {error}", asset.id)
            })
        })
        .collect();
    let mut loaded = build(&list).expect("bundled extensions must build successfully");
    let mut diagnostics = Vec::new();
    let mut installed = Vec::new();
    if let Some(dir) = installed_dir {
        match fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(error) => {
                            diagnostics.push(diagnostic(
                                None,
                                dir,
                                format!("Failed to read extension directory entry: {error}"),
                            ));
                            continue;
                        }
                    };
                    let path = entry.path();
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if name.starts_with('.') || !path.is_dir() {
                        continue;
                    }
                    let manifest_path = path.join(EXTENSION_MANIFEST_FILE);
                    let raw = match fs::read_to_string(&manifest_path) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            diagnostics.push(diagnostic(
                                None,
                                &path,
                                format!(
                                    "Extension folder {} has no extension.json",
                                    path.display()
                                ),
                            ));
                            continue;
                        }
                        other => other,
                    };
                    let manifest = raw.map_err(|error| error.to_string()).and_then(|raw| {
                        serde_json::from_str::<ExtensionManifest>(&raw)
                            .map_err(|error| error.to_string())
                    });
                    let manifest = match manifest {
                        Ok(manifest) => manifest,
                        Err(error) => {
                            diagnostics.push(diagnostic(
                                None,
                                &path,
                                format!(
                                    "Extension {} is invalid: {error}",
                                    manifest_path.display()
                                ),
                            ));
                            continue;
                        }
                    };
                    let id = manifest.id.clone();
                    let error = if manifest.api != EXTENSION_API_VERSION {
                        Some(format!(
                            "Extension {id} targets extension API {}; this Sworm supports API 1",
                            manifest.api
                        ))
                    } else if id != name {
                        Some(format!("Extension folder {name} declares id {id}; the folder must be named after the id"))
                    } else {
                        None
                    };
                    if let Some(error) = error {
                        diagnostics.push(diagnostic(Some(id), &path, error));
                        continue;
                    }
                    match prepare(manifest, Some(path.clone()), &[]) {
                        Ok(extension) => installed.push(extension),
                        Err(error) => diagnostics.push(diagnostic(
                            Some(id.clone()),
                            &path,
                            format!("Extension {id} rejected: {error}"),
                        )),
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => diagnostics.push(diagnostic(
                None,
                dir,
                format!(
                    "Failed to read extensions directory {}: {error}",
                    dir.display()
                ),
            )),
        }
    }
    installed.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
    // Retry rejects until a pass accepts nothing, so an extension may depend on
    // one whose id sorts later; ascending-id passes keep the result deterministic.
    let mut pending = installed;
    let mut rejected = Vec::new();
    loop {
        let mut accepted_any = false;
        for extension in std::mem::take(&mut pending) {
            match try_add(&mut list, extension) {
                Ok(candidate) => {
                    loaded = candidate;
                    accepted_any = true;
                }
                Err(failure) => rejected.push(failure),
            }
        }
        if !accepted_any || rejected.is_empty() {
            break;
        }
        pending = rejected.drain(..).map(|(extension, _)| extension).collect();
    }
    for (extension, error) in rejected {
        let id = extension.manifest.id;
        let path = extension.dir.expect("installed extension has a directory");
        diagnostics.push(diagnostic(
            Some(id.clone()),
            &path,
            format!("Extension {id} rejected: {error}"),
        ));
    }
    for diagnostic in &diagnostics {
        tracing::warn!(extension_id = ?diagnostic.extension_id, path = %diagnostic.path, "{}", diagnostic.message);
    }
    loaded.catalog.diagnostics = diagnostics;
    loaded
}

fn diagnostic(extension_id: Option<String>, path: &Path, message: String) -> ExtensionDiagnostic {
    ExtensionDiagnostic {
        extension_id,
        path: path.to_string_lossy().into_owned(),
        message,
    }
}

fn payload_path(rel: &str) -> Result<&str, String> {
    let rel = rel.strip_prefix("./").unwrap_or(rel);
    if rel.is_empty() || rel.starts_with('/') || rel.split('/').any(|part| part == "..") {
        return Err(format!("invalid payload path {rel}"));
    }
    Ok(rel)
}

fn read_payload<'a>(
    dir: Option<&Path>,
    payload: &'a [(&str, &'a str)],
    relative: &str,
) -> Result<Cow<'a, str>, String> {
    let relative = payload_path(relative)?;
    if let Some(dir) = dir {
        fs::read_to_string(dir.join(relative))
            .map(Cow::Owned)
            .map_err(|error| format!("Failed to read payload {relative}: {error}"))
    } else {
        payload
            .iter()
            .find(|(name, _)| *name == relative)
            .map(|(_, contents)| Cow::Borrowed(*contents))
            .ok_or_else(|| format!("missing payload {relative}"))
    }
}

fn prepare(
    mut manifest: ExtensionManifest,
    dir: Option<PathBuf>,
    payload: &[(&str, &str)],
) -> Result<PreparedExtension, String> {
    normalize_manifest(&mut manifest);
    validate_manifest(&manifest)?;
    if dir.is_none()
        && manifest
            .contributes
            .lsp_servers
            .iter()
            .map(|server| &server.command)
            .chain(
                manifest
                    .contributes
                    .formatters
                    .iter()
                    .map(|formatter| &formatter.command),
            )
            .chain(manifest.contributes.harnesses.iter().flat_map(|harness| {
                std::iter::once(&harness.command)
                    .chain(harness.chat.iter().map(|chat| &chat.command))
            }))
            .any(|command| command.trim().starts_with("./"))
    {
        return Err("bundled extension commands must not be relative".to_string());
    }
    for server in &mut manifest.contributes.lsp_servers {
        let Some(settings) = server.settings.as_mut() else {
            continue;
        };
        if settings.schema.is_some() {
            settings.schema_path = None;
            continue;
        }
        if let Some(relative) = settings.schema_path.take() {
            let raw = read_payload(dir.as_deref(), payload, &relative)?;
            settings.schema = Some(serde_json::from_str(&raw).map_err(|error| {
                format!(
                    "Extension {} server {} has invalid schema {relative}: {error}",
                    manifest.id, server.id
                )
            })?);
        }
    }
    let mut grammars = Vec::new();
    for contribution in &manifest.contributes.grammars {
        let raw = read_payload(dir.as_deref(), payload, &contribution.path)?;
        let grammar: serde_json::Value = serde_json::from_str(&raw).map_err(|_| {
            format!(
                "grammar {} at {} is not a TextMate grammar",
                contribution.language, contribution.path
            )
        })?;
        if !grammar.is_object()
            || grammar
                .get("scopeName")
                .and_then(serde_json::Value::as_str)
                .is_none()
        {
            return Err(format!(
                "grammar {} at {} is not a TextMate grammar",
                contribution.language, contribution.path
            ));
        }
        grammars.push(ExtensionGrammar {
            extension_id: manifest.id.clone(),
            language: contribution.language.clone(),
            embedded_languages: contribution.embedded_languages.clone(),
            grammar,
        });
    }
    let mut harnesses = Vec::new();
    for harness in &manifest.contributes.harnesses {
        let image = |path: &str| -> Result<String, String> {
            let raw = read_payload(dir.as_deref(), payload, path)?;
            if !raw.trim().contains("<svg") {
                return Err(format!("harness {} image {path} is not an SVG", harness.id));
            }
            Ok(raw.into_owned())
        };
        harnesses.push(HarnessDescriptor {
            id: harness.id.clone(),
            label: harness.label.clone(),
            extension_id: manifest.id.clone(),
            icon: image(&harness.brand.icon)?,
            wordmark: harness.brand.wordmark.as_deref().map(image).transpose()?,
            wordmark_aspect: harness.brand.wordmark_aspect,
            wordmark_font: harness.brand.wordmark_font.clone(),
            gradient: harness.brand.gradient.clone(),
            shift_encoding: harness.terminal.shift_encoding,
            title_prefixes: harness.terminal.title_prefixes.clone(),
        });
    }
    Ok(PreparedExtension {
        manifest,
        dir,
        grammars,
        harnesses,
    })
}

/// Adds `extension` (replacing a same-id entry) when the list still builds;
/// otherwise restores `list` and hands the extension back with the error.
fn try_add(
    list: &mut Vec<PreparedExtension>,
    extension: PreparedExtension,
) -> Result<LoadedExtensionCatalog, (PreparedExtension, String)> {
    let index = list
        .iter()
        .position(|item| item.manifest.id == extension.manifest.id);
    let previous = match index {
        Some(index) => Some(std::mem::replace(&mut list[index], extension)),
        None => {
            list.push(extension);
            None
        }
    };
    match build(list) {
        Ok(candidate) => Ok(candidate),
        Err(error) => {
            let extension = match (index, previous) {
                (Some(index), Some(previous)) => std::mem::replace(&mut list[index], previous),
                _ => list.pop().expect("candidate was pushed"),
            };
            Err((extension, error))
        }
    }
}

fn build(list: &[PreparedExtension]) -> Result<LoadedExtensionCatalog, String> {
    let languages = merge_languages(list)?;
    let known_languages: HashSet<_> = languages
        .iter()
        .map(|language| language.id.as_str())
        .collect();
    let mut lsp_servers = Vec::new();
    let mut formatters = Vec::new();
    let mut grammars = Vec::new();
    let mut grammar_owners = BTreeMap::new();
    let mut harnesses = Vec::new();
    let mut harness_descriptors = Vec::new();
    let mut harness_owners: BTreeMap<&str, &str> = BTreeMap::new();
    let mut server_ids = HashSet::new();
    let mut extension_ids = HashSet::new();
    for extension in list {
        let manifest = &extension.manifest;
        if !extension_ids.insert(&manifest.id) {
            return Err(format!("Duplicate extension id {}", manifest.id));
        }
        for definition in &manifest.contributes.lsp_servers {
            for language in &definition.applies_to_languages {
                if !known_languages.contains(language.as_str()) {
                    return Err(format!(
                        "Extension {} server {} targets unknown language {language}",
                        manifest.id, definition.id
                    ));
                }
            }
            validate_document_selectors(
                &manifest.id,
                &definition.id,
                &definition.document_selectors,
                &known_languages,
            )?;
            let server_definition_id =
                ExtensionService::server_definition_id(&manifest.id, &definition.id);
            if !server_ids.insert(server_definition_id.clone()) {
                return Err(format!(
                    "Duplicate server definition id {server_definition_id}"
                ));
            }
            lsp_servers.push(LoadedLspServer {
                server_definition_id,
                extension_id: manifest.id.clone(),
                extension_label: manifest.label.clone(),
                extension_dir: extension.dir.clone(),
                requires: manifest.requires.clone(),
                install_hint: definition
                    .install_hint
                    .as_ref()
                    .or(manifest.install_hint.as_ref())
                    .cloned()
                    .unwrap_or_default(),
                definition: definition.clone(),
            });
        }
        for definition in &manifest.contributes.formatters {
            let formatter_id = ExtensionService::server_definition_id(&manifest.id, &definition.id);
            for language in &definition.languages {
                if !known_languages.contains(language.as_str()) {
                    return Err(format!(
                        "formatter {formatter_id} targets unknown language {language}"
                    ));
                }
            }
            formatters.push(LoadedFormatter {
                formatter_id,
                extension_id: manifest.id.clone(),
                extension_label: manifest.label.clone(),
                extension_dir: extension.dir.clone(),
                requires: manifest.requires.clone(),
                install_hint: definition
                    .install_hint
                    .as_ref()
                    .or(manifest.install_hint.as_ref())
                    .cloned()
                    .unwrap_or_default(),
                definition: definition.clone(),
            });
        }
        for definition in &manifest.contributes.harnesses {
            if let Some(owner) = harness_owners.insert(definition.id.as_str(), manifest.id.as_str())
            {
                return Err(format!(
                    "harness {} is contributed by both {owner} and {}",
                    definition.id, manifest.id
                ));
            }
            harnesses.push(LoadedHarness {
                extension_id: manifest.id.clone(),
                extension_label: manifest.label.clone(),
                extension_dir: extension.dir.clone(),
                requires: manifest.requires.clone(),
                install_hint: definition
                    .install_hint
                    .as_ref()
                    .or(manifest.install_hint.as_ref())
                    .cloned()
                    .unwrap_or_default(),
                definition: definition.clone(),
            });
        }
        harness_descriptors.extend(extension.harnesses.iter().cloned());
        for grammar in &extension.grammars {
            if let Some(owner) =
                grammar_owners.insert(grammar.language.as_str(), manifest.id.as_str())
            {
                return Err(format!(
                    "grammar for {} is contributed by both {owner} and {}",
                    grammar.language, manifest.id
                ));
            }
            grammars.push(grammar.clone());
        }
    }
    for grammar in &grammars {
        for dependency in &grammar.embedded_languages {
            if !grammar_owners.contains_key(dependency.as_str()) {
                return Err(format!(
                    "grammar {} embeds {dependency}, which no extension provides",
                    grammar.language
                ));
            }
        }
    }
    let mut formatter_policies = Vec::new();
    for language in &languages {
        let candidates: Vec<_> = formatters
            .iter()
            .filter(|formatter| formatter.definition.languages.contains(&language.id))
            .collect();
        let Some(first) = candidates.first() else {
            continue;
        };
        let has_lsp = lsp_servers.iter().any(|server| {
            server
                .definition
                .applies_to_languages
                .contains(&language.id)
        });
        let default = candidates
            .iter()
            .find(|formatter| formatter.definition.default)
            .map(|formatter| formatter.formatter_id.as_str())
            .unwrap_or(if has_lsp {
                FORMATTER_LSP
            } else {
                &first.formatter_id
            });
        let default_label = candidates
            .iter()
            .find(|formatter| formatter.formatter_id == default)
            .map(|formatter| formatter.definition.label.as_str())
            .unwrap_or("LSP");
        let mut options = vec![FormatterOption {
            id: default.to_string(),
            label: default_label.to_string(),
        }];
        options.extend(
            candidates
                .iter()
                .filter(|formatter| formatter.formatter_id != default)
                .map(|formatter| FormatterOption {
                    id: formatter.formatter_id.clone(),
                    label: formatter.definition.label.clone(),
                }),
        );
        if has_lsp && default != FORMATTER_LSP {
            options.push(FormatterOption {
                id: FORMATTER_LSP.to_string(),
                label: "LSP".to_string(),
            });
        }
        options.push(FormatterOption {
            id: FORMATTER_DISABLED.to_string(),
            label: "Disabled".to_string(),
        });
        formatter_policies.push(LanguageFormatterPolicy {
            language_id: language.id.clone(),
            language_label: language.label.clone(),
            options,
            default: default.to_string(),
        });
    }
    let pages = build_settings_pages(&languages, &lsp_servers);
    let settings_ids = SettingsCatalogIds {
        harness_ids: harnesses
            .iter()
            .map(|harness| harness.definition.id.clone())
            .collect(),
        lsp_server_ids: lsp_servers
            .iter()
            .map(|server| server.server_definition_id.clone())
            .collect(),
        formatter_ids: formatters
            .iter()
            .map(|formatter| formatter.formatter_id.clone())
            .collect(),
        formatting_defaults: formatter_policies
            .iter()
            .map(|policy| (policy.language_id.clone(), policy.default.clone()))
            .collect(),
    };
    Ok(LoadedExtensionCatalog {
        catalog: ExtensionCatalog {
            runtime: ExtensionRuntimeCatalog {
                languages,
                formatter_policies,
                harnesses: harness_descriptors,
            },
            settings: ExtensionSettingsCatalog { pages },
            diagnostics: Vec::new(),
        },
        lsp_servers,
        formatters,
        harnesses,
        grammars,
        settings_ids,
    })
}

pub fn resolve_contribution_command(
    command: &str,
    extension_dir: Option<&Path>,
    merged_path: &str,
    override_path: Option<&str>,
) -> Result<String, String> {
    let candidate = override_path.unwrap_or(command).trim();
    if candidate.is_empty() {
        return Err("Command path is empty".to_string());
    }
    if override_path.is_none() {
        if let Some(relative) = candidate.strip_prefix("./") {
            let dir = extension_dir.ok_or_else(|| {
                format!("relative command {command} needs an installed extension")
            })?;
            return validate_direct_command_path(dir.join(relative).to_string_lossy().as_ref());
        }
    }
    if candidate.contains('/') {
        return validate_direct_command_path(candidate);
    }
    which::which_in(candidate, Some(merged_path), ".")
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|_| format!("{candidate} is not installed or not on PATH"))
}

pub fn missing_requirement(requires: &[String], merged_path: &str) -> Option<String> {
    requires
        .iter()
        .find(|name| which::which_in(name, Some(merged_path), ".").is_err())
        .map(|name| format!("{name} is not installed or not on PATH"))
}

fn normalize_manifest(manifest: &mut ExtensionManifest) {
    for language in &mut manifest.contributes.languages {
        language.id = normalize_language_id(&language.id);
        language.aliases = normalize_segments(&language.aliases);
        language.extensions = normalize_segments(&language.extensions);
        language.filenames = normalize_segments(&language.filenames);
    }

    for server in &mut manifest.contributes.lsp_servers {
        server.applies_to_languages = server
            .applies_to_languages
            .iter()
            .map(|value| normalize_language_id(value))
            .filter(|value| !value.is_empty())
            .collect();
        for selector in &mut server.document_selectors {
            if let Some(language) = selector.language.as_ref() {
                let normalized = normalize_language_id(language);
                selector.language = (!normalized.is_empty()).then_some(normalized);
            }
            selector.extensions = normalize_segments(&selector.extensions);
            selector.filenames = normalize_segments(&selector.filenames);
        }
    }
    for formatter in &mut manifest.contributes.formatters {
        formatter.languages = formatter
            .languages
            .iter()
            .map(|id| normalize_language_id(id))
            .collect();
    }
    for grammar in &mut manifest.contributes.grammars {
        grammar.language = normalize_language_id(&grammar.language);
        grammar.embedded_languages = grammar
            .embedded_languages
            .iter()
            .map(|id| normalize_language_id(id))
            .collect();
    }
}

fn validate_manifest(manifest: &ExtensionManifest) -> Result<(), String> {
    if manifest.id.trim().is_empty()
        || manifest.label.trim().is_empty()
        || manifest.version.trim().is_empty()
    {
        return Err("extension must have a non-empty id, label, and version".to_string());
    }
    if manifest.api != EXTENSION_API_VERSION {
        return Err(format!("unsupported extension API {}", manifest.api));
    }
    for requirement in &manifest.requires {
        if requirement.is_empty()
            || requirement.contains('/')
            || requirement.chars().any(char::is_whitespace)
        {
            return Err(format!("invalid required command {requirement}"));
        }
    }
    let mut harness_ids = HashSet::new();
    for harness in &manifest.contributes.harnesses {
        let id = &harness.id;
        if !id
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            || !id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_' || byte == b'-'
            })
        {
            return Err(format!("harness id {id} must match [a-z0-9][a-z0-9_-]*"));
        }
        if !harness_ids.insert(id) {
            return Err(format!("duplicate harness {id}"));
        }
        if harness.label.trim().is_empty() || harness.command.trim().is_empty() {
            return Err(format!("harness {id} must define a label and command"));
        }
        for path in std::iter::once(&harness.brand.icon).chain(harness.brand.wordmark.iter()) {
            if payload_path(path).is_err() || !path.ends_with(".svg") {
                return Err(format!(
                    "harness {id} brand images must be SVG payload files"
                ));
            }
        }
        if match (&harness.brand.wordmark, harness.brand.wordmark_aspect) {
            (Some(_), Some(aspect)) => !aspect.is_finite() || aspect <= 0.0,
            (None, None) => false,
            _ => true,
        } {
            return Err(format!(
                "harness {id} wordmark needs a positive wordmark_aspect"
            ));
        }
        if harness.brand.gradient.iter().any(|color| {
            color.len() != 7
                || !color.starts_with('#')
                || !color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        }) {
            return Err(format!("harness {id} gradient colors must be #rrggbb"));
        }
        for (field, args) in [
            ("resume_args", &harness.terminal.resume_args),
            ("new_session_args", &harness.terminal.new_session_args),
        ] {
            if !args.is_empty()
                && (!args.iter().any(|arg| arg.contains(SESSION_ID_VARIABLE))
                    || args.iter().any(|arg| {
                        arg.match_indices("${")
                            .any(|(index, _)| !arg[index..].starts_with(SESSION_ID_VARIABLE))
                    }))
            {
                return Err(format!(
                    "harness {id} {field} must use {SESSION_ID_VARIABLE} and no other variable"
                ));
            }
        }
        for (field, args) in [
            ("terminal.args", harness.terminal.args.as_slice()),
            (
                "chat.args",
                harness
                    .chat
                    .as_ref()
                    .map_or(&[][..], |chat| chat.args.as_slice()),
            ),
            (
                "detect.version_args",
                harness.detect.version_args.as_slice(),
            ),
        ] {
            for arg in args {
                if arg.contains("${") {
                    return Err(format!(
                        "harness {id} {field} uses unknown variable in {arg}"
                    ));
                }
            }
        }
        if !harness.terminal.new_session_args.is_empty() && harness.terminal.resume_args.is_empty()
        {
            return Err(format!(
                "harness {id} mints session ids but has no resume_args"
            ));
        }
        if harness.terminal.title_prefixes.iter().any(String::is_empty) {
            return Err(format!(
                "harness {id} title_prefixes must be non-empty strings"
            ));
        }
        if harness
            .chat
            .as_ref()
            .is_some_and(|chat| chat.command.trim().is_empty())
        {
            return Err(format!("harness {id} chat must define a command"));
        }
    }
    let mut formatter_ids = HashSet::new();
    for formatter in &manifest.contributes.formatters {
        if formatter.id.trim().is_empty() || formatter.label.trim().is_empty() {
            return Err("formatter must have a non-empty id and label".to_string());
        }
        if !formatter_ids.insert(&formatter.id) {
            return Err(format!("duplicate formatter {}", formatter.id));
        }
        if formatter.command.trim().is_empty() || formatter.languages.is_empty() {
            return Err(format!(
                "formatter {} must define a command and languages",
                formatter.id
            ));
        }
        for arg in &formatter.args {
            for (index, _) in arg.match_indices("${") {
                if !arg[index..].starts_with("${relative_file}") {
                    return Err(format!(
                        "formatter {} uses unknown variable in {arg}",
                        formatter.id
                    ));
                }
            }
        }
    }
    let mut grammar_languages = HashSet::new();
    for grammar in &manifest.contributes.grammars {
        if grammar.language.trim().is_empty() || !grammar_languages.insert(&grammar.language) {
            return Err(format!(
                "grammar language {} must be non-empty and unique",
                grammar.language
            ));
        }
        payload_path(&grammar.path)?;
    }
    let mut seen_language_ids = HashSet::new();
    for language in &manifest.contributes.languages {
        if language.id.trim().is_empty() || language.label.trim().is_empty() {
            return Err(format!(
                "Extension {} has a language with an empty id or label",
                manifest.id
            ));
        }
        if !seen_language_ids.insert(language.id.clone()) {
            return Err(format!(
                "Extension {} declares language {} more than once",
                manifest.id, language.id
            ));
        }
    }

    let mut seen_server_ids = HashSet::new();
    for server in &manifest.contributes.lsp_servers {
        if server.id.trim().is_empty() || server.label.trim().is_empty() {
            return Err(format!(
                "Extension {} has an LSP server with an empty id or label",
                manifest.id
            ));
        }
        if !seen_server_ids.insert(server.id.clone()) {
            return Err(format!(
                "Extension {} declares LSP server {} more than once",
                manifest.id, server.id
            ));
        }
        if server.command.trim().is_empty() {
            return Err(format!(
                "Extension {} server {} must define a host command",
                manifest.id, server.id
            ));
        }
        if server.applies_to_languages.is_empty() {
            return Err(format!(
                "Extension {} server {} must declare applies_to_languages",
                manifest.id, server.id
            ));
        }
        if server.document_selectors.is_empty() {
            return Err(format!(
                "Extension {} server {} must define document selectors",
                manifest.id, server.id
            ));
        }

        if let Some(settings) = server.settings.as_ref() {
            if settings.schema.is_none() && settings.schema_path.is_none() {
                return Err(format!(
                    "Extension {} server {} settings must define schema or schema_path",
                    manifest.id, server.id
                ));
            }
        }
    }

    Ok(())
}

fn merge_languages(extensions: &[PreparedExtension]) -> Result<Vec<LanguageContribution>, String> {
    let mut merged = BTreeMap::<String, LanguageContribution>::new();

    for extension in extensions {
        for language in &extension.manifest.contributes.languages {
            if let Some(existing) = merged.get_mut(&language.id) {
                if existing.label != language.label {
                    return Err(format!(
                        "Language {} is declared with conflicting labels: {} vs {}",
                        language.id, existing.label, language.label
                    ));
                }

                append_unique(&mut existing.aliases, &language.aliases);
                append_unique(&mut existing.extensions, &language.extensions);
                append_unique(&mut existing.filenames, &language.filenames);
            } else {
                merged.insert(language.id.clone(), language.clone());
            }
        }
    }

    let mut languages = merged.into_values().collect::<Vec<_>>();
    languages.sort_by(|left, right| left.label.cmp(&right.label).then(left.id.cmp(&right.id)));
    Ok(languages)
}

fn validate_document_selectors(
    extension_id: &str,
    server_id: &str,
    selectors: &[LspDocumentSelector],
    known_language_ids: &HashSet<&str>,
) -> Result<(), String> {
    for selector in selectors {
        if let Some(language_id) = selector.language.as_deref() {
            if !known_language_ids.contains(language_id) {
                return Err(format!(
                    "Extension {} server {} selector references unknown language {}",
                    extension_id, server_id, language_id
                ));
            }
        }
    }

    Ok(())
}

fn build_settings_pages(
    languages: &[LanguageContribution],
    server_definitions: &[LoadedLspServer],
) -> Vec<SettingsPage> {
    let mut pages = Vec::new();
    let mut claimed_language_ids = HashSet::new();

    for overlay in PAGE_OVERLAYS {
        let language_ids = overlay
            .language_ids
            .iter()
            .filter(|value| languages.iter().any(|language| language.id == **value))
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        if language_ids.is_empty() {
            continue;
        }
        claimed_language_ids.extend(language_ids.iter().cloned());

        pages.push(SettingsPage {
            id: overlay.id.to_string(),
            kind: overlay.kind,
            label: overlay.label.to_string(),
            icon_filename: overlay.icon_filename.to_string(),
            server_definition_ids: server_definition_ids_for_languages(
                server_definitions,
                &language_ids,
            ),
            language_ids,
        });
    }

    for language in languages
        .iter()
        .filter(|language| !claimed_language_ids.contains(language.id.as_str()))
    {
        let language_ids = vec![language.id.clone()];
        pages.push(SettingsPage {
            id: format!("language:{}", language.id),
            kind: SettingsPageKind::Language,
            label: language.label.clone(),
            icon_filename: icon_filename_for_language(language),
            server_definition_ids: server_definition_ids_for_languages(
                server_definitions,
                &language_ids,
            ),
            language_ids,
        });
    }

    // Overlay and auto pages share one nav list; order it by label so
    // overlays don't float to the top in declaration order.
    pages.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then_with(|| left.id.cmp(&right.id))
    });
    pages
}

fn server_definition_ids_for_languages(
    server_definitions: &[LoadedLspServer],
    language_ids: &[String],
) -> Vec<String> {
    let page_languages = language_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();

    server_definitions
        .iter()
        .filter(|server| {
            server
                .definition
                .applies_to_languages
                .iter()
                .any(|language_id| page_languages.contains(language_id.as_str()))
        })
        .map(|server| server.server_definition_id.clone())
        .collect()
}

fn append_unique(target: &mut Vec<String>, incoming: &[String]) {
    let mut seen = target.iter().cloned().collect::<HashSet<_>>();
    for value in incoming {
        if seen.insert(value.clone()) {
            target.push(value.clone());
        }
    }
}

fn icon_filename_for_language(language: &LanguageContribution) -> String {
    if let Some(filename) = language.filenames.first() {
        return filename.clone();
    }

    if let Some(extension) = language.extensions.first() {
        return format!("index{}", extension);
    }

    format!("{}.txt", language.id)
}

fn normalize_segments(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| normalize_segment(value))
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_segment(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.starts_with('.') {
        return trimmed.to_ascii_lowercase();
    }
    if trimmed.contains('/') {
        return trimmed.to_string();
    }
    trimmed.to_ascii_lowercase()
}

fn normalize_language_id(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn validate_direct_command_path(candidate: &str) -> Result<String, String> {
    let metadata = fs::metadata(candidate).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => format!("Command not found at {candidate}"),
        _ => format!("Failed to read command metadata {candidate}: {error}"),
    })?;
    if !metadata.is_file() {
        return Err(format!("Command path is not a file: {candidate}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(format!("Command is not executable: {candidate}"));
        }
    }
    Ok(candidate.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_manifests_parse() {
        for extension in BUNDLED {
            let parsed = parse_bundled(extension)
                .unwrap_or_else(|error| panic!("failed to parse {}: {}", extension.id, error));
            assert_eq!(parsed.manifest.id, extension.id);
        }
    }

    #[test]
    fn bundled_schemas_inline_on_parse() {
        for extension in BUNDLED {
            let parsed = parse_bundled(extension)
                .unwrap_or_else(|error| panic!("failed to parse {}: {}", extension.id, error));
            for server in &parsed.manifest.contributes.lsp_servers {
                let Some(settings) = server.settings.as_ref() else {
                    continue;
                };
                assert!(
                    settings.schema_path.is_none(),
                    "schema_path should be consumed for {}::{}",
                    extension.id,
                    server.id
                );
                assert!(
                    settings.schema.is_some(),
                    "schema should be inlined for {}::{}",
                    extension.id,
                    server.id
                );
            }
        }
    }

    #[test]
    fn rejects_missing_applies_to_languages() {
        let raw = r#"
        {
          "id": "dev.sworm.test",
          "label": "Test",
          "version": "1.0.0",
          "api": 1,
          "contributes": {
            "languages": [
              {
                "id": "test",
                "label": "Test",
                "extensions": [".test"]
              }
            ],
            "lsp_servers": [
              {
                "id": "server",
                "label": "Server",
                "command": "test-server",
                "install_hint": "n/a",
                "document_selectors": [
                  {
                    "language": "test",
                    "extensions": [".test"]
                  }
                ]
              }
            ]
          }
        }
        "#;

        let mut manifest = serde_json::from_str::<ExtensionManifest>(raw).unwrap();
        normalize_manifest(&mut manifest);
        assert!(validate_manifest(&manifest).is_err());
    }

    #[test]
    fn builds_expected_settings_pages() {
        let catalog = load_catalog(None);
        let pages = &catalog.catalog.settings.pages;
        let page_ids = pages
            .iter()
            .map(|page| page.id.as_str())
            .collect::<Vec<_>>();

        for expected in [
            "language:javascript-typescript",
            "language:json",
            "language:css",
            "language:svelte",
            "language:html",
            "language:nix",
            "language:python",
            "language:fish",
            "language:rust",
            "language:go",
        ] {
            assert!(page_ids.contains(&expected), "missing page {expected}");
        }
        assert!(
            pages.windows(2).all(|pair| pair[0].label <= pair[1].label),
            "pages not sorted by label: {:?}",
            pages
                .iter()
                .map(|page| page.label.as_str())
                .collect::<Vec<_>>()
        );
    }

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!("sworm-extensions-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn install(root: &Path, manifest: serde_json::Value) {
        let dir = root.join(manifest["id"].as_str().unwrap());
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(EXTENSION_MANIFEST_FILE),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn bundled_harnesses_reproduce_previous_providers() {
        let loaded = load_catalog(None);
        assert_eq!(
            loaded
                .harnesses
                .iter()
                .map(|h| h.definition.id.as_str())
                .collect::<Vec<_>>(),
            ["claude_code", "codex", "omp", "antigravity"]
        );
        assert_eq!(
            loaded
                .harnesses
                .iter()
                .map(|h| h.definition.command.as_str())
                .collect::<Vec<_>>(),
            ["claude", "codex", "omp", "agy"]
        );
        for (harness, resume) in
            loaded
                .harnesses
                .iter()
                .zip(["--resume", "resume", "--resume", "--conversation"])
        {
            let definition = &harness.definition;
            assert_eq!(
                definition.terminal.resume_args,
                [resume, SESSION_ID_VARIABLE]
            );
            if definition.id == "claude_code" {
                assert_eq!(
                    definition.terminal.new_session_args,
                    ["--session-id", SESSION_ID_VARIABLE]
                );
            } else {
                assert!(definition.terminal.new_session_args.is_empty());
            }
            if definition.id == "omp" {
                assert_eq!(
                    definition.detect.version_stream,
                    VersionStream::StdoutOrStderr
                );
                assert_eq!(definition.terminal.title_prefixes, ["π", ">"]);
            } else {
                assert_eq!(definition.detect.version_stream, VersionStream::Stdout);
                assert!(definition.terminal.title_prefixes.is_empty());
            }
            assert_eq!(
                definition.terminal.shift_encoding,
                if definition.id == "antigravity" {
                    ShiftEncoding::Kitty
                } else {
                    ShiftEncoding::Legacy
                }
            );
        }
        assert_eq!(
            loaded
                .catalog
                .runtime
                .harnesses
                .iter()
                .map(|h| h.id.as_str())
                .collect::<Vec<_>>(),
            ["claude_code", "codex", "omp", "antigravity"]
        );
        assert!(loaded
            .catalog
            .runtime
            .harnesses
            .iter()
            .all(|h| h.icon.contains("<svg")));
    }

    #[test]
    fn harness_with_unknown_variable_is_rejected() {
        let root = temp_dir();
        install(
            &root,
            serde_json::json!({
                "id": "dev.example.harness", "label": "Harness", "version": "1.0.0", "api": 1,
                "contributes": { "harnesses": [{
                    "id": "example", "label": "Example", "command": "example",
                    "brand": { "icon": "./icon.svg", "gradient": ["#000000", "#ffffff"] },
                    "terminal": { "resume_args": ["--resume", "${id}"] }
                }] }
            }),
        );
        fs::write(root.join("dev.example.harness/icon.svg"), "<svg></svg>").unwrap();
        let loaded = load_catalog(Some(&root));
        assert_eq!(loaded.catalog.diagnostics.len(), 1);
        assert!(loaded.catalog.diagnostics[0].message.contains("rejected"));
        assert!(loaded.catalog.diagnostics[0]
            .message
            .contains("harness example resume_args must use ${session_id} and no other variable"));
        assert!(loaded.find_harness("example").is_none());
        assert!(!loaded
            .catalog
            .runtime
            .harnesses
            .iter()
            .any(|h| h.id == "example"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bundled_formatter_policies_match_previous_defaults() {
        let loaded = load_catalog(None);
        for (language, expected) in [
            (
                "javascript",
                vec!["dev.sworm.biome::biome", "lsp", "disabled"],
            ),
            (
                "typescript",
                vec!["dev.sworm.biome::biome", "lsp", "disabled"],
            ),
            ("json", vec!["dev.sworm.biome::biome", "lsp", "disabled"]),
            ("nix", vec!["dev.sworm.nix::nixfmt", "lsp", "disabled"]),
            ("go", vec!["lsp", "dev.sworm.go::gofmt", "disabled"]),
        ] {
            let policy = loaded
                .catalog
                .runtime
                .formatter_policies
                .iter()
                .find(|policy| policy.language_id == language)
                .unwrap();
            assert_eq!(
                policy
                    .options
                    .iter()
                    .map(|option| option.id.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(policy.default, expected[0]);
            assert_eq!(
                loaded.settings_ids.formatting_defaults[language],
                expected[0]
            );
        }
        assert_eq!(
            loaded
                .settings_ids
                .formatting_defaults
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["go", "javascript", "json", "nix", "typescript"]
        );
    }

    #[test]
    fn installed_extension_replaces_bundled_by_id() {
        let root = temp_dir();
        let asset = BUNDLED
            .iter()
            .find(|asset| asset.id == "dev.sworm.fish")
            .unwrap();
        let mut manifest: serde_json::Value = serde_json::from_str(asset.manifest).unwrap();
        manifest["label"] = "Fish Override".into();
        install(&root, manifest);
        let dir = root.join(asset.id);
        for (relative, contents) in asset.payload {
            let path = dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
        let loaded = load_catalog(Some(&root));
        assert!(
            loaded.catalog.diagnostics.is_empty(),
            "{:?}",
            loaded.catalog.diagnostics
        );
        let server = loaded
            .lsp_servers
            .iter()
            .find(|server| {
                server
                    .definition
                    .applies_to_languages
                    .iter()
                    .any(|language| language == "fish")
            })
            .unwrap();
        assert_eq!(server.extension_label, "Fish Override");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsupported_api_is_rejected_with_diagnostic() {
        let root = temp_dir();
        install(
            &root,
            serde_json::json!({
                "id": "dev.example.future", "label": "Future", "version": "1.0.0", "api": 2,
                "contributes": { "languages": [{ "id": "future", "label": "Future" }] }
            }),
        );
        let loaded = load_catalog(Some(&root));
        assert_eq!(loaded.catalog.diagnostics.len(), 1);
        assert!(loaded.catalog.diagnostics[0].message.contains("API 2"));
        assert!(!loaded
            .catalog
            .runtime
            .languages
            .iter()
            .any(|language| language.id == "future"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn broken_installed_extension_is_isolated() {
        let root = temp_dir();
        install(
            &root,
            serde_json::json!({
                "id": "dev.example.broken", "label": "Broken", "version": "1.0.0", "api": 1,
                "contributes": { "formatters": [{ "id": "fmt", "label": "Format", "command": "cat", "languages": ["unknown"] }] }
            }),
        );
        let loaded = load_catalog(Some(&root));
        assert_eq!(loaded.catalog.diagnostics.len(), 1);
        assert!(loaded.catalog.diagnostics[0].message.contains("rejected"));
        assert!(loaded
            .catalog
            .settings
            .pages
            .iter()
            .any(|page| page.id == "language:nix"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn relative_installed_dir_yields_absolute_extension_commands() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_dir();
        install(
            &root,
            serde_json::json!({
                "id": "dev.example.relfmt", "label": "Rel", "version": "1.0.0", "api": 1,
                "contributes": {
                    "languages": [{ "id": "relfmt", "label": "Rel", "extensions": [".relfmt"] }],
                    "formatters": [{ "id": "fmt", "label": "Upper", "command": "./bin/fmt", "languages": ["relfmt"] }]
                }
            }),
        );
        let bin = root.join("dev.example.relfmt/bin");
        fs::create_dir(&bin).unwrap();
        let path = bin.join("fmt");
        fs::write(&path, "#!/bin/sh\ntr a-z A-Z\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let cwd = std::env::current_dir().unwrap();
        let relative_root = PathBuf::from("../".repeat(cwd.components().count() - 1))
            .join(root.strip_prefix("/").unwrap());
        let loaded = load_catalog(Some(&relative_root));
        assert!(
            loaded.catalog.diagnostics.is_empty(),
            "{:?}",
            loaded.catalog.diagnostics
        );
        let formatter = loaded.find_formatter("dev.example.relfmt::fmt").unwrap();
        let program = resolve_contribution_command(
            &formatter.definition.command,
            formatter.extension_dir.as_deref(),
            "",
            None,
        )
        .unwrap();
        assert_eq!(
            crate::services::formatting::FormattingService::run(
                &program,
                &[],
                "/",
                "abc",
                &std::collections::HashMap::from([(
                    "PATH".to_string(),
                    std::env::var("PATH").unwrap_or_default(),
                )]),
            ),
            Ok("ABC".to_string())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installed_extension_may_depend_on_later_sorting_extension() {
        let root = temp_dir();
        install(
            &root,
            serde_json::json!({
                "id": "com.acme.fmt", "label": "Format", "version": "1.0.0", "api": 1,
                "contributes": {
                    "formatters": [{ "id": "fmt", "label": "Format", "command": "cat", "languages": ["vue"] }]
                }
            }),
        );
        install(
            &root,
            serde_json::json!({
                "id": "dev.other.vue", "label": "Vue", "version": "1.0.0", "api": 1,
                "contributes": {
                    "languages": [{ "id": "vue", "label": "Vue", "extensions": [".vue"] }]
                }
            }),
        );
        let loaded = load_catalog(Some(&root));
        assert!(
            loaded.catalog.diagnostics.is_empty(),
            "{:?}",
            loaded.catalog.diagnostics
        );
        assert!(loaded.find_formatter("com.acme.fmt::fmt").is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn relative_command_resolves_inside_extension_dir() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir();
        fs::create_dir(root.join("bin")).unwrap();
        let path = root.join("bin/tool");
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            resolve_contribution_command("./bin/tool", Some(&root), "", None).unwrap(),
            path.to_string_lossy()
        );
        assert!(payload_path("../escape").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_direct_command_paths() {
        let missing =
            std::env::temp_dir().join(format!("sworm-lsp-missing-{}", uuid::Uuid::new_v4()));
        let result = validate_direct_command_path(missing.to_string_lossy().as_ref());
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn accepts_executable_direct_command_paths() {
        use std::os::unix::fs::PermissionsExt;

        let path = std::env::temp_dir().join(format!("sworm-lsp-command-{}", uuid::Uuid::new_v4()));
        fs::write(&path, "#!/bin/sh\nexit 0\n").unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions).unwrap();

        let resolved = validate_direct_command_path(path.to_string_lossy().as_ref()).unwrap();
        assert_eq!(resolved, path.to_string_lossy());

        fs::remove_file(path).unwrap();
    }
}
