//! `template.toml` — the manifest of a skeleton version or draft. Reference: docs/manifest.md.

use std::collections::BTreeMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};

pub const MANIFEST_FILE: &str = "template.toml";
pub const SCHEMA_VERSION: u32 = 1;
pub const CATEGORIES: &[&str] = &["web", "desktop", "cli", "library", "mobile", "other"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub stack: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<ForkOrigin>,
    /// Tool name → semver requirement, e.g. `node = ">=24"`.
    #[serde(default)]
    pub requirements: BTreeMap<String, String>,
    #[serde(default)]
    pub features: IndexMap<String, Feature>,
    /// Single-choice options (e.g. how Docker is used); each option can add files, env and
    /// requirements and replace setup steps and commands.
    #[serde(default)]
    pub choices: IndexMap<String, Choice>,
    /// Written to `.env` and `.env.example`; values are minijinja templates.
    #[serde(default)]
    pub env: IndexMap<String, String>,
    /// Env keys that get a unique free port per project, starting at the value.
    #[serde(default)]
    pub ports: IndexMap<String, u16>,
    /// Env keys that get `N` random bytes (hex) in `.env` and stay empty in `.env.example`.
    #[serde(default)]
    pub secrets: IndexMap<String, usize>,
    #[serde(default)]
    pub set: Vec<SetEdit>,
    #[serde(default)]
    pub setup: Vec<SetupStep>,
    #[serde(default)]
    pub commands: IndexMap<String, Command>,
    #[serde(default)]
    pub verify: Verify,
    /// How to preview the skeleton without creating a project.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<Preview>,
    /// UI translations of the manifest texts: language → English text → translation.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updates: Option<Updates>,
}

/// `[preview]`: a throwaway copy of the skeleton is set up once and this command runs in it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Preview {
    /// Command key to run (a long-running server or a build). Default: the primary command.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// What to open; `${VAR}` comes from the preview's `.env`. Default: the command's `url`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// File or folder to show after the command, relative to the preview project (e.g. a build output).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// What to do next, shown after the command started or finished.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
    /// Shell command run in the preview copy before it is deleted, for what the preview leaves
    /// outside it (e.g. the `~/Library` folders of a desktop app). Options add their `cleanup`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<String>,
}

fn default_category() -> String {
    "other".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ForkOrigin {
    pub id: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Feature {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub default: bool,
    /// Paths or globs (relative to `files/`) copied only when the feature is enabled.
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub requirements: BTreeMap<String, String>,
    #[serde(default)]
    pub env: IndexMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Choice {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    /// Key of the option selected by default.
    pub default: String,
    /// Shown only when earlier choices have one of the given values, e.g. `{ docker = ["run", "full"] }`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub when: BTreeMap<String, OneOrMany>,
    pub options: IndexMap<String, ChoiceOption>,
}

/// A string or a list of strings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    pub fn values(&self) -> Vec<&str> {
        match self {
            Self::One(value) => vec![value.as_str()],
            Self::Many(values) => values.iter().map(String::as_str).collect(),
        }
    }

    pub fn contains(&self, value: &str) -> bool {
        self.values().contains(&value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    /// Paths or globs copied only when this option is selected.
    #[serde(default)]
    pub files: Vec<String>,
    /// Extra tool requirements when selected.
    #[serde(default)]
    pub requirements: BTreeMap<String, String>,
    /// Base requirements that do not apply when selected (e.g. no local Node when everything runs in Docker).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drop_requirements: Vec<String>,
    #[serde(default)]
    pub env: IndexMap<String, String>,
    /// Extra `[ports]` allocated only with this option (e.g. a database server's forwarded port).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub ports: IndexMap<String, u16>,
    /// Replaces the base `[[setup]]` steps when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup: Option<Vec<SetupStep>>,
    /// Overrides fields of base commands with the same key, or adds commands.
    #[serde(default)]
    pub commands: IndexMap<String, CommandOverride>,
    /// Run when a throwaway copy with this option is removed — after a verification, when a
    /// preview is deleted (e.g. remove Docker containers, volumes and images).
    #[serde(default, alias = "verify_cleanup", skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<String>,
    /// Overrides fields of `[preview]` when selected (e.g. another message in Docker mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<PreviewOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct PreviewOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Partial command: set fields replace those of the base command.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct CommandOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub long: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inputs: Option<Vec<CommandInput>>,
}

/// Structured edit of a JSON or TOML file after copying (e.g. `package.json` → `name`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SetEdit {
    pub file: String,
    /// Keys; `name[field=value]` selects an element of an array (e.g. `package[name=app]`).
    pub path: Vec<String>,
    pub value: String,
    /// `json` or `toml`; inferred from the extension and well-known lockfiles.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Apply only when this feature is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SetupStep {
    pub label: String,
    pub run: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Command {
    #[serde(default)]
    pub label: String,
    pub run: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Long-running process (dev server) with a Stop button.
    #[serde(default, skip_serializing_if = "is_false")]
    pub long: bool,
    /// URL to open; `${VAR}` is read from the project's `.env`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub primary: bool,
    /// Shown only when this feature is enabled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feature: Option<String>,
    /// Values asked for before each run (a form in the app, `inputs` in MCP), passed to the
    /// command as environment variables: `run = 'npm run new -- "$NAME"'`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<CommandInput>,
}

/// One value a command asks for before it runs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CommandInput {
    /// Environment variable name (`NAME`, `EXTENSION_NAME`).
    pub name: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub placeholder: String,
    /// Initial value of the field.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default: String,
    /// May be left empty (by default every input is required).
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional: bool,
}

impl Command {
    /// Checks the values given for [`Command::inputs`]: known names, required ones not empty.
    pub fn check_inputs(&self, key: &str, values: &BTreeMap<String, String>) -> Result<()> {
        for name in values.keys() {
            if !self.inputs.iter().any(|i| &i.name == name) {
                return Err(Error::invalid(format!("command \"{key}\" has no input \"{name}\"")));
            }
        }
        for input in self.inputs.iter().filter(|i| !i.optional) {
            if values.get(&input.name).is_none_or(|v| v.trim().is_empty()) {
                return Err(Error::invalid(format!("command \"{key}\" needs input {} ({})", input.name, input.label)));
            }
        }
        Ok(())
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(deny_unknown_fields)]
pub struct Verify {
    /// Command keys run after setup on a throwaway project.
    #[serde(default)]
    pub commands: Vec<String>,
    /// Extra choice combinations to verify besides the defaults, e.g. `[{ docker = "full" }]`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Updates {
    pub ecosystem: String,
    /// Packages held within a version range (e.g. `typescript = "<6.1"` until typescript-eslint
    /// supports newer releases): newer versions are reported as held and skipped by level updates.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub hold: BTreeMap<String, String>,
}

impl Manifest {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = dir.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path).at(&path)?;
        Self::parse(&text).map_err(|err| Error::validation(format!("{}: {}", path.display(), err.message)))
    }

    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).map_err(|err| Error::validation(err.to_string().trim().to_string()))
    }

    /// Features enabled by default.
    pub fn default_features(&self) -> BTreeMap<String, bool> {
        self.features.iter().map(|(name, f)| (name.clone(), f.default)).collect()
    }

    /// Normalises a requested feature map: unknown names are errors, missing ones use defaults.
    pub fn resolve_features(&self, requested: &BTreeMap<String, bool>) -> Result<BTreeMap<String, bool>> {
        for name in requested.keys() {
            if !self.features.contains_key(name) {
                return Err(Error::invalid(format!(
                    "unknown feature \"{name}\" (available: {})",
                    self.features.keys().cloned().collect::<Vec<_>>().join(", ")
                )));
            }
        }
        Ok(self
            .features
            .iter()
            .map(|(name, f)| (name.clone(), requested.get(name).copied().unwrap_or(f.default)))
            .collect())
    }

    /// Normalises requested choices: unknown names or values are errors, missing ones use defaults,
    /// choices whose `when` is not met get an empty value.
    pub fn resolve_choices(&self, requested: &BTreeMap<String, String>) -> Result<BTreeMap<String, String>> {
        for (name, value) in requested {
            let choice = self.choices.get(name).ok_or_else(|| {
                Error::invalid(format!(
                    "unknown choice \"{name}\" (available: {})",
                    self.choices.keys().cloned().collect::<Vec<_>>().join(", ")
                ))
            })?;
            if !value.is_empty() && !choice.options.contains_key(value) {
                return Err(Error::invalid(format!(
                    "\"{value}\" is not an option of {name} (available: {})",
                    choice.options.keys().cloned().collect::<Vec<_>>().join(", ")
                )));
            }
        }
        let mut resolved = BTreeMap::new();
        for (name, choice) in &self.choices {
            let active = choice
                .when
                .iter()
                .all(|(other, allowed)| resolved.get(other).is_some_and(|value: &String| allowed.contains(value)));
            let value = if active {
                requested.get(name).filter(|v| !v.is_empty()).cloned().unwrap_or_else(|| choice.default.clone())
            } else {
                String::new()
            };
            resolved.insert(name.clone(), value);
        }
        Ok(resolved)
    }

    pub fn select(&self, features: &BTreeMap<String, bool>, choices: &BTreeMap<String, String>) -> Result<Selection> {
        Ok(Selection { features: self.resolve_features(features)?, choices: self.resolve_choices(choices)? })
    }

    /// Default selection; `all_features` turns every feature on (verification).
    pub fn default_selection(&self, all_features: bool) -> Selection {
        Selection {
            features: self.features.iter().map(|(k, f)| (k.clone(), all_features || f.default)).collect(),
            choices: self.resolve_choices(&BTreeMap::new()).unwrap_or_default(),
        }
    }

    /// Selected options in declaration order.
    pub fn selected_options<'a>(&'a self, selection: &Selection) -> Vec<(&'a str, &'a ChoiceOption)> {
        self.choices
            .iter()
            .filter_map(|(name, choice)| {
                let value = selection.choices.get(name)?;
                choice.options.get(value).map(|option| (name.as_str(), option))
            })
            .collect()
    }

    /// Requirements, env, files, setup and commands for a selection.
    pub fn resolve(&self, selection: &Selection) -> Resolved {
        let feature_on =
            |name: &Option<String>| name.as_ref().is_none_or(|f| selection.features.get(f).copied().unwrap_or(false));
        let options = self.selected_options(selection);

        let mut requirements = self.requirements.clone();
        for (_, option) in &options {
            for tool in &option.drop_requirements {
                requirements.remove(tool);
            }
        }
        let mut env = self.env.clone();
        let mut ports = self.ports.clone();
        let mut include = Vec::new();
        let mut exclude = Vec::new();
        for (name, feature) in &self.features {
            if selection.features.get(name).copied().unwrap_or(false) {
                requirements.extend(feature.requirements.clone());
                env.extend(feature.env.clone());
                include.extend(feature.files.clone());
            } else {
                exclude.extend(feature.files.clone());
            }
        }
        for (name, choice) in &self.choices {
            let selected = selection.choices.get(name).map(String::as_str).unwrap_or_default();
            for (key, option) in &choice.options {
                if key == selected {
                    requirements.extend(option.requirements.clone());
                    env.extend(option.env.clone());
                    ports.extend(option.ports.clone());
                    include.extend(option.files.clone());
                } else {
                    exclude.extend(option.files.clone());
                }
            }
        }

        let mut setup: Vec<SetupStep> = self.setup.clone();
        let mut commands = self.commands.clone();
        let mut cleanup = Vec::new();
        let mut preview = self.preview.clone().unwrap_or_default();
        for (_, option) in &options {
            if let Some(steps) = &option.setup {
                setup = steps.clone();
            }
            for (key, patch) in &option.commands {
                let command = commands.entry(key.clone()).or_insert_with(|| Command {
                    label: key.clone(),
                    run: String::new(),
                    description: String::new(),
                    long: false,
                    url: None,
                    primary: false,
                    feature: None,
                    inputs: Vec::new(),
                });
                patch.apply(command);
            }
            cleanup.extend(option.cleanup.clone());
            if let Some(patch) = &option.preview {
                if let Some(v) = &patch.command {
                    preview.command = Some(v.clone());
                }
                if let Some(v) = &patch.url {
                    preview.url = Some(v.clone()).filter(|u| !u.is_empty());
                }
                if let Some(v) = &patch.path {
                    preview.path = Some(v.clone()).filter(|p| !p.is_empty());
                }
                if let Some(v) = &patch.message {
                    preview.message = v.clone();
                }
            }
        }
        setup.retain(|step| feature_on(&step.feature));
        commands.retain(|_, command| feature_on(&command.feature) && !command.run.trim().is_empty());

        Resolved { requirements, env, include, exclude, setup, commands, cleanup, preview, ports }
    }
}

impl CommandOverride {
    fn apply(&self, command: &mut Command) {
        if let Some(v) = &self.label {
            command.label = v.clone();
        }
        if let Some(v) = &self.run {
            command.run = v.clone();
        }
        if let Some(v) = &self.description {
            command.description = v.clone();
        }
        if let Some(v) = self.long {
            command.long = v;
        }
        if let Some(v) = &self.url {
            command.url = Some(v.clone()).filter(|u| !u.is_empty());
        }
        if let Some(v) = self.primary {
            command.primary = v;
        }
        if let Some(v) = &self.feature {
            command.feature = Some(v.clone()).filter(|f| !f.is_empty());
        }
        if let Some(v) = &self.inputs {
            command.inputs = v.clone();
        }
    }
}

impl Resolved {
    /// Whether a missing `tool` blocks setup: base requirements that still apply, and tools named
    /// in the setup steps or in `extra` commands. Other tools (e.g. docker for "Docker up") only
    /// matter later and produce warnings.
    pub fn needs_tool(&self, manifest: &Manifest, tool: &str, extra: &[&str]) -> bool {
        if manifest.requirements.contains_key(tool) && self.requirements.contains_key(tool) {
            return true;
        }
        self.setup.iter().map(|s| s.run.as_str()).chain(extra.iter().copied()).any(|script| {
            script.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_')).any(|word| word == tool)
        })
    }
}

impl Manifest {
    /// Every user-facing text of the manifest (translated through `[translations.<lang>]`).
    pub fn texts(&self) -> Vec<String> {
        let mut texts = vec![self.name.clone(), self.description.clone()];
        for feature in self.features.values() {
            texts.extend([feature.label.clone(), feature.description.clone()]);
        }
        for choice in self.choices.values() {
            texts.extend([choice.label.clone(), choice.description.clone()]);
            for option in choice.options.values() {
                texts.extend([option.label.clone(), option.description.clone()]);
                for command in option.commands.values() {
                    texts.extend(command.label.clone());
                    texts.extend(command.description.clone());
                    for input in command.inputs.iter().flatten() {
                        texts.extend([input.label.clone(), input.placeholder.clone()]);
                    }
                }
                texts.extend(option.setup.iter().flatten().map(|s| s.label.clone()));
                texts.extend(option.preview.as_ref().and_then(|p| p.message.clone()));
            }
        }
        for command in self.commands.values() {
            texts.extend([command.label.clone(), command.description.clone()]);
            texts.extend(command.inputs.iter().flat_map(|i| [i.label.clone(), i.placeholder.clone()]));
        }
        texts.extend(self.setup.iter().map(|s| s.label.clone()));
        texts.extend(self.preview.as_ref().map(|p| p.message.clone()));
        let mut seen = std::collections::BTreeSet::new();
        texts.retain(|t| !t.trim().is_empty() && seen.insert(t.clone()));
        texts
    }

    /// Translations of the given texts only (e.g. name and description for the registry).
    pub fn translations_of(&self, texts: &[&str]) -> BTreeMap<String, BTreeMap<String, String>> {
        self.translations
            .iter()
            .map(|(lang, map)| {
                let subset =
                    map.iter().filter(|(k, _)| texts.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone()));
                (lang.clone(), subset.collect::<BTreeMap<_, _>>())
            })
            .filter(|(_, map)| !map.is_empty())
            .collect()
    }
}

/// Selected features and choices of a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Selection {
    pub features: BTreeMap<String, bool>,
    /// Choice → selected option key; empty for choices whose `when` is not met.
    pub choices: BTreeMap<String, String>,
}

impl Manifest {
    /// Command key used by the preview: `[preview] command` (with option overrides), else the
    /// primary, else the first long-running command.
    pub fn preview_command(&self, resolved: &Resolved) -> Option<String> {
        resolved
            .preview
            .command
            .clone()
            .or_else(|| resolved.commands.iter().find(|(_, c)| c.primary).map(|(k, _)| k.clone()))
            .or_else(|| resolved.commands.iter().find(|(_, c)| c.long).map(|(k, _)| k.clone()))
    }
}

/// The manifest applied to a selection.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub requirements: BTreeMap<String, String>,
    /// Env templates (base, enabled features, selected options).
    pub env: IndexMap<String, String>,
    /// File patterns that must be copied (enabled features, selected options).
    pub include: Vec<String>,
    /// File patterns to skip (disabled features, unselected options), unless also included.
    pub exclude: Vec<String>,
    pub setup: Vec<SetupStep>,
    pub commands: IndexMap<String, Command>,
    /// Scripts that remove what a throwaway copy created (see `ChoiceOption::cleanup`).
    pub cleanup: Vec<String>,
    /// `[preview]` with the selected options' overrides.
    pub preview: Preview,
    /// `[ports]` plus the selected options' ports.
    pub ports: IndexMap<String, u16>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
schema = 1
id = "node-vue"
name = "Node + Vue"
category = "web"
languages = ["node"]

[requirements]
node = ">=24"

[features.docker]
label = "Docker"
files = ["Dockerfile"]
requirements = { docker = ">=24" }
env = { COMPOSE_PROJECT_NAME = "{{ project.slug }}" }

[env]
APP_NAME = "{{ project.name }}"

[ports]
APP_PORT = 3000

[[set]]
file = "package.json"
path = ["name"]
value = "{{ project.slug }}"

[[setup]]
label = "Install dependencies"
run = "npm install"

[commands.dev]
label = "Dev"
run = "npm run dev"
long = true
url = "http://localhost:${APP_PORT}"
primary = true

[commands.new]
label = "New page"
run = 'npm run new -- "$NAME" "$TITLE"'
inputs = [
  { name = "NAME", label = "Page name", placeholder = "about" },
  { name = "TITLE", label = "Title", default = "About", optional = true },
]

[verify]
commands = ["dev"]

[updates]
ecosystem = "npm"
hold = { typescript = "<6.1" }
"#;

    #[test]
    fn parses_sample() {
        let m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(m.id, "node-vue");
        assert_eq!(m.ports["APP_PORT"], 3000);
        assert!(m.commands["dev"].long);
        assert_eq!(m.features["docker"].env["COMPOSE_PROJECT_NAME"], "{{ project.slug }}");
        let selection = m.select(&BTreeMap::from([("docker".into(), true)]), &BTreeMap::new()).unwrap();
        assert_eq!(m.resolve(&selection).requirements.len(), 2);
    }

    #[test]
    fn checks_command_inputs() {
        let m = Manifest::parse(SAMPLE).unwrap();
        let new = &m.commands["new"];
        assert_eq!(new.inputs.len(), 2);
        let values = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
        };
        assert!(new.check_inputs("new", &values(&[("NAME", "about")])).is_ok());
        assert!(new.check_inputs("new", &values(&[("NAME", "  ")])).is_err(), "required input is empty");
        assert!(new.check_inputs("new", &values(&[("NAME", "a"), ("OTHER", "b")])).is_err(), "unknown input");
        assert!(m.texts().iter().any(|t| t == "Page name"));
    }

    #[test]
    fn rejects_unknown_keys() {
        let err = Manifest::parse(&SAMPLE.replace("category = \"web\"", "categroy = \"web\"")).unwrap_err();
        assert!(err.message.contains("categroy"), "{}", err.message);
    }

    const CHOICES: &str = r#"
schema = 1
id = "demo"
name = "Demo"

[requirements]
node = ">=24"

[env]
APP_NAME = "{{ project.name }}"

[[setup]]
label = "Install dependencies"
run = "npm install"

[commands.dev]
label = "Dev"
run = "npm run dev"
long = true
primary = true

[commands.test]
label = "Test"
run = "npm test"

[choices.docker]
label = "Docker"
default = "run"

[choices.docker.options.none]
label = "No Docker"

[choices.docker.options.run]
label = "Docker to run"
files = ["Dockerfile"]
requirements = { docker = ">=24" }
env = { COMPOSE_PROJECT_NAME = "{{ project.slug }}" }
commands.docker-up = { label = "Docker up", run = "docker compose up", long = true }

[choices.docker.options.full]
label = "Everything in Docker"
files = ["Dockerfile", "dev"]
requirements = { docker = ">=24" }
drop_requirements = ["node"]
setup = [{ label = "Install dependencies", run = "docker compose run --rm dev npm install" }]
commands.dev = { run = "docker compose up dev" }
commands.test = { run = "docker compose run --rm dev npm test" }
cleanup = "docker compose down -v"
preview = { message = "Runs in Docker." }
ports = { DB_PORT = 5432 }

[choices.data]
label = "Data"
default = "volume"
when = { docker = ["run", "full"] }

[choices.data.options.volume]
label = "Docker volume"
env = { DOCKER_DATA = "data" }

[choices.data.options.local]
label = "Project folder"
env = { DOCKER_DATA = "./data" }

[verify]
commands = ["test"]
variants = [{ docker = "full", data = "local" }]

[preview]
message = "Open the page."
cleanup = "rm -rf cache"

[[set]]
file = "config/app.cfg"
path = ["app[id=main]", "name"]
value = "{{ project.slug }}"
format = "toml"

[translations.ru]
"Docker" = "Docker"
"Everything in Docker" = "Всё в Docker"
"Open the page." = "Откройте страницу."
"#;

    #[test]
    fn resolves_choices() {
        let m = Manifest::parse(CHOICES).unwrap();
        let none = |pairs: &[(&str, &str)]| -> BTreeMap<String, String> {
            pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
        };
        // Defaults; `data` applies because docker = run.
        let defaults = m.resolve_choices(&BTreeMap::new()).unwrap();
        assert_eq!(defaults, none(&[("data", "volume"), ("docker", "run")]));
        // `data` is inactive without Docker, even when requested.
        let off = m.resolve_choices(&none(&[("docker", "none"), ("data", "local")])).unwrap();
        assert_eq!(off["data"], "");
        assert!(m.resolve_choices(&none(&[("docker", "k8s")])).is_err());
        assert!(m.resolve_choices(&none(&[("colour", "red")])).is_err());

        let run = m.resolve(&m.select(&BTreeMap::new(), &BTreeMap::new()).unwrap());
        assert_eq!(run.requirements.keys().collect::<Vec<_>>(), ["docker", "node"]);
        assert_eq!(run.env["DOCKER_DATA"], "data");
        assert!(run.commands.contains_key("docker-up"));
        assert_eq!(run.setup[0].run, "npm install");
        assert!(run.exclude.contains(&"dev".to_string()));

        let full = m.resolve(&m.select(&BTreeMap::new(), &none(&[("docker", "full"), ("data", "local")])).unwrap());
        assert_eq!(full.requirements.keys().collect::<Vec<_>>(), ["docker"]);
        assert_eq!(full.env["DOCKER_DATA"], "./data");
        assert_eq!(full.commands["dev"].run, "docker compose up dev");
        assert!(full.commands["dev"].long && full.commands["dev"].primary, "overrides keep other fields");
        assert!(!full.commands.contains_key("docker-up"));
        assert_eq!(full.setup[0].run, "docker compose run --rm dev npm install");
        assert_eq!(full.cleanup, ["docker compose down -v"]);
        assert_eq!(full.preview.message, "Runs in Docker.");
        assert_eq!(full.ports["DB_PORT"], 5432);
        assert!(full.include.contains(&"dev".to_string()));

        let local = m.resolve(&m.select(&BTreeMap::new(), &none(&[("docker", "none")])).unwrap());
        assert_eq!(local.preview.message, "Open the page.");
        assert!(!local.ports.contains_key("DB_PORT"));
        assert!(!local.env.contains_key("DOCKER_DATA"));

        let ru = &m.translations_of(&["Everything in Docker", "Unknown"])["ru"];
        assert_eq!(ru.len(), 1);
        assert_eq!(ru["Everything in Docker"], "Всё в Docker");
        assert!(m.texts().iter().any(|t| t == "Open the page."));
        assert!(local.exclude.contains(&"Dockerfile".to_string()));
    }

    /// Keeps schemas/template.schema.json in sync with the Rust model.
    #[test]
    fn json_schema_covers_every_key() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../schemas/template.schema.json")).unwrap();
        let mut m = Manifest::parse(SAMPLE).unwrap();
        m.forked_from = Some(ForkOrigin { id: "a".into(), version: "1.0.0".into() });
        m.set[0].feature = Some("docker".into());
        let value = serde_json::to_value(&m).unwrap();
        let props = &schema["properties"];
        for key in value.as_object().unwrap().keys() {
            assert!(props.get(key).is_some(), "schema lacks top-level key {key}");
        }
        let nested = [
            ("features", &value["features"]["docker"], &props["features"]["additionalProperties"]["properties"]),
            ("commands", &value["commands"]["dev"], &props["commands"]["additionalProperties"]["properties"]),
            ("commands", &value["commands"]["new"], &props["commands"]["additionalProperties"]["properties"]),
            ("inputs", &value["commands"]["new"]["inputs"][1], &schema["$defs"]["commandInput"]["properties"]),
            ("set", &value["set"][0], &props["set"]["items"]["properties"]),
            ("setup", &value["setup"][0], &props["setup"]["items"]["properties"]),
        ];
        for (section, item, schema_props) in nested {
            for key in item.as_object().unwrap().keys() {
                assert!(schema_props.get(key).is_some(), "schema lacks {section}.{key}");
            }
        }

        let with_choices = serde_json::to_value(Manifest::parse(CHOICES).unwrap()).unwrap();
        for key in with_choices.as_object().unwrap().keys() {
            assert!(props.get(key).is_some(), "schema lacks top-level key {key}");
        }
        for key in with_choices["set"][0].as_object().unwrap().keys() {
            assert!(props["set"]["items"]["properties"].get(key).is_some(), "schema lacks set.{key}");
        }
        let choice_props = &props["choices"]["additionalProperties"]["properties"];
        let option_props = &schema["$defs"]["choiceOption"]["properties"];
        for key in with_choices["choices"]["data"].as_object().unwrap().keys() {
            assert!(choice_props.get(key).is_some(), "schema lacks choices.*.{key}");
        }
        for key in with_choices["choices"]["docker"]["options"]["full"].as_object().unwrap().keys() {
            assert!(option_props.get(key).is_some(), "schema lacks choice option key {key}");
        }
        for key in with_choices["preview"].as_object().unwrap().keys() {
            assert!(props["preview"]["properties"].get(key).is_some(), "schema lacks preview.{key}");
        }
        for key in value["updates"].as_object().unwrap().keys() {
            assert!(props["updates"]["properties"].get(key).is_some(), "schema lacks updates.{key}");
        }
        for key in with_choices["verify"].as_object().unwrap().keys() {
            assert!(props["verify"]["properties"].get(key).is_some(), "schema lacks verify.{key}");
        }
    }

    #[test]
    fn rejects_unknown_features() {
        let m = Manifest::parse(SAMPLE).unwrap();
        assert!(m.resolve_features(&BTreeMap::from([("k8s".into(), true)])).is_err());
    }
}
