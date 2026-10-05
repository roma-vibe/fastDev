//! `.fastdev.toml` — project metadata committed inside every created project (SPEC §6.4).

use std::collections::BTreeMap;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};
use crate::manifest::{Command, Manifest, Resolved, Selection, SetupStep};
use crate::util;

pub const META_FILE: &str = ".fastdev.toml";
pub const LOCK_FILE: &str = ".fastdev.lock";

const HEADER: &str = "# fastDev project metadata: where the project came from and the commands shown\n\
# as buttons on its page in fastDev. Edit the commands freely.\n\n";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaProject {
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaSkeleton {
    pub id: String,
    pub version: String,
    /// Repository the version came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    /// Commit of the version tag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub project: MetaProject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skeleton: Option<MetaSkeleton>,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub choices: BTreeMap<String, String>,
    #[serde(default)]
    pub setup: Vec<SetupStep>,
    #[serde(default)]
    pub commands: IndexMap<String, Command>,
    /// Translations of the command and setup labels shown in the app.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
}

impl ProjectMeta {
    /// Metadata with the setup steps and commands resolved for the selection.
    pub fn new(
        name: &str,
        slug: &str,
        manifest: &Manifest,
        version: &str,
        selection: &Selection,
        resolved: &Resolved,
    ) -> Self {
        Self {
            project: MetaProject { name: name.to_string(), slug: slug.to_string(), created_at: util::now_rfc3339() },
            skeleton: Some(MetaSkeleton {
                id: manifest.id.clone(),
                version: version.to_string(),
                repo: None,
                commit: None,
            }),
            features: selection.features.clone(),
            choices: selection
                .choices
                .iter()
                .filter(|(_, v)| !v.is_empty())
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            setup: resolved.setup.clone(),
            commands: resolved.commands.clone(),
            translations: {
                let texts: Vec<&str> = resolved
                    .commands
                    .values()
                    .flat_map(|c| {
                        [c.label.as_str(), c.description.as_str()]
                            .into_iter()
                            .chain(c.inputs.iter().flat_map(|i| [i.label.as_str(), i.placeholder.as_str()]))
                    })
                    .chain(resolved.setup.iter().map(|s| s.label.as_str()))
                    .collect();
                manifest.translations_of(&texts)
            },
        }
    }

    pub fn load(project_dir: &Path) -> Result<Option<Self>> {
        let path = project_dir.join(META_FILE);
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).at(&path)?;
        toml::from_str(&text)
            .map(Some)
            .map_err(|err| Error::validation(format!("{}: {}", path.display(), err.to_string().trim())))
    }

    pub fn save(&self, project_dir: &Path) -> Result<()> {
        let body = toml::to_string_pretty(self).map_err(|err| Error::internal(err.to_string()))?;
        util::write_atomic(&project_dir.join(META_FILE), format!("{HEADER}{body}").as_bytes())
    }

    /// Commands visible with the enabled features.
    pub fn active_commands(&self) -> IndexMap<String, Command> {
        self.commands
            .iter()
            .filter(|(_, cmd)| cmd.feature.as_ref().is_none_or(|f| self.features.get(f).copied().unwrap_or(false)))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn active_setup(&self) -> Vec<SetupStep> {
        self.setup
            .iter()
            .filter(|s| s.feature.as_ref().is_none_or(|f| self.features.get(f).copied().unwrap_or(false)))
            .cloned()
            .collect()
    }

    /// Metadata for a folder without `.fastdev.toml`: commands from `package.json` scripts.
    pub fn infer(project_dir: &Path) -> Self {
        let name = project_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut commands = IndexMap::new();
        let mut setup = Vec::new();
        let package = project_dir.join("package.json");
        if let Ok(text) = std::fs::read_to_string(&package)
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&text)
        {
            setup.push(SetupStep { label: "Install dependencies".into(), run: "npm install".into(), feature: None });
            if let Some(scripts) = json.get("scripts").and_then(|s| s.as_object()) {
                for (key, label, long) in
                    [("dev", "Dev", true), ("start", "Start", true), ("build", "Build", false), ("test", "Test", false)]
                {
                    if scripts.contains_key(key) {
                        commands.insert(
                            key.to_string(),
                            Command {
                                label: label.into(),
                                run: format!("npm run {key}"),
                                description: String::new(),
                                long,
                                url: None,
                                primary: key == "dev",
                                feature: None,
                                inputs: Vec::new(),
                            },
                        );
                    }
                }
            }
        }
        Self {
            project: MetaProject { slug: util::slugify(&name), name, created_at: util::now_rfc3339() },
            skeleton: None,
            features: BTreeMap::new(),
            choices: BTreeMap::new(),
            setup,
            commands,
            translations: BTreeMap::new(),
        }
    }
}
