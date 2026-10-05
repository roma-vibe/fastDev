//! App settings stored as `settings.json` in the data folder.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::util::{home_dir, write_atomic};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    System,
    En,
    Ru,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Editor {
    #[default]
    Vscode,
    Cursor,
    Zed,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Terminal {
    #[default]
    Terminal,
    Iterm,
    Warp,
    Ghostty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    pub language: Language,
    /// Skeleton registries: git URLs or local folders with an `index.json`.
    pub registries: Vec<String>,
    /// Extra skeleton repositories added by URL (not listed in a registry).
    pub skeleton_sources: Vec<String>,
    /// Working clones of skeletons being authored: one git repository per skeleton.
    pub workspace_path: PathBuf,
    pub projects_dir: PathBuf,
    pub editor: Editor,
    /// Used when `editor` is `custom`; `{path}` is replaced with the project path.
    pub editor_command: String,
    pub terminal: Terminal,
    /// Closing the window hides it; the app keeps running in the menu bar.
    pub keep_in_menu_bar: bool,
    /// Push the skeleton repository (and the registry) to `origin` after publishing.
    pub push_on_publish: bool,
    pub favorites: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: Language::System,
            registries: default_registries(),
            skeleton_sources: Vec::new(),
            workspace_path: default_library_dir().join("skeletons"),
            projects_dir: home_dir().join("Documents"),
            editor: Editor::Vscode,
            editor_command: String::new(),
            terminal: Terminal::Terminal,
            keep_in_menu_bar: true,
            push_on_publish: true,
            favorites: Vec::new(),
        }
    }
}

/// The public registry of fastDev skeletons.
pub const PUBLIC_REGISTRY: &str = "https://github.com/roma-vibe/fastdev-registry.git";

/// Library layout of skeleton authors: `~/Documents/fastDev-library/{registry,skeletons/<id>}`,
/// with the registry repository cloned locally so publishing can update it.
pub fn default_library_dir() -> PathBuf {
    home_dir().join("Documents/fastDev-library")
}

/// The local clone of the registry when there is one (authors), otherwise the public registry.
fn default_registries() -> Vec<String> {
    let local = default_library_dir().join("registry");
    let registry = if local.join("index.json").is_file() {
        local.to_string_lossy().into_owned()
    } else {
        PUBLIC_REGISTRY.to_string()
    };
    vec![registry]
}

impl Settings {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        write_atomic(path, text.as_bytes())
    }
}
