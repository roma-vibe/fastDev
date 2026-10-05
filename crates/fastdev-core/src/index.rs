//! Skeleton registries: a git repository (or a local folder) with an `index.json` that lists
//! skeleton repositories. The catalog is built from registries without downloading skeletons.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};
use crate::git;
use crate::manifest::{ForkOrigin, Manifest};
use crate::util::{self, expand_home};

pub const INDEX_FILE: &str = "index.json";
pub const INDEX_SCHEMA: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryIndex {
    pub schema: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub skeletons: Vec<IndexEntry>,
}

/// One skeleton of a registry. Metadata mirrors the manifest of the latest version so the
/// catalog can be shown before the repository is downloaded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IndexEntry {
    pub id: String,
    /// Git URL or local path of the skeleton repository.
    pub repo: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "other")]
    pub category: String,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub stack: Vec<String>,
    /// Latest published version (a hint; the tags of the repository are authoritative).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<ForkOrigin>,
    /// Translations of `name` and `description`: language → English text → translation.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
}

fn other() -> String {
    "other".into()
}

impl IndexEntry {
    pub fn from_manifest(manifest: &Manifest, repo: &str, latest: &str) -> Self {
        Self {
            id: manifest.id.clone(),
            repo: repo.to_string(),
            name: manifest.name.clone(),
            description: manifest.description.clone(),
            category: manifest.category.clone(),
            languages: manifest.languages.clone(),
            tags: manifest.tags.clone(),
            stack: manifest.stack.clone(),
            latest: Some(latest.to_string()),
            forked_from: manifest.forked_from.clone(),
            translations: manifest.translations_of(&[&manifest.name, &manifest.description]),
        }
    }
}

/// Where a registry lives on disk.
#[derive(Debug, Clone)]
pub struct RegistrySource {
    pub url: String,
    pub dir: PathBuf,
    /// A local git work tree: publishing updates its `index.json` and commits.
    pub writable: bool,
}

/// Local folders are used in place; remote URLs are cloned into the data folder.
pub fn local_path(url: &str) -> Option<PathBuf> {
    let url = url.trim();
    if let Some(path) = url.strip_prefix("file://") {
        return Some(PathBuf::from(path));
    }
    (url.starts_with('/') || url.starts_with('~')).then(|| expand_home(url))
}

pub fn source(url: &str, data_dir: &Path) -> RegistrySource {
    match local_path(url) {
        Some(dir) => {
            let writable = git::is_work_tree(&dir);
            RegistrySource { url: url.to_string(), dir, writable }
        }
        None => {
            let hash = util::short_hash(url);
            RegistrySource { url: url.to_string(), dir: data_dir.join("registries").join(hash), writable: false }
        }
    }
}

/// Pulls a registry: clones remote registries into the cache, fast-forwards local ones with a remote.
pub fn sync(source: &RegistrySource) -> Result<()> {
    if local_path(&source.url).is_some() {
        if !source.dir.join(INDEX_FILE).is_file() {
            return Err(Error::not_found(format!("{} has no {INDEX_FILE}", source.dir.display())));
        }
        if source.writable && git::remote_url(&source.dir, "origin").is_some() {
            git::run(&source.dir, &["pull", "--ff-only", "--quiet"])?;
        }
        return Ok(());
    }
    if git::is_work_tree(&source.dir) {
        git::run(&source.dir, &["pull", "--ff-only", "--quiet"])?;
    } else {
        git::clone(&source.url, &source.dir)?;
    }
    Ok(())
}

pub fn load(source: &RegistrySource) -> Result<RegistryIndex> {
    let path = source.dir.join(INDEX_FILE);
    let text = std::fs::read_to_string(&path).at(&path)?;
    let index: RegistryIndex =
        serde_json::from_str(&text).map_err(|err| Error::validation(format!("{}: {err}", path.display())))?;
    if index.schema != INDEX_SCHEMA {
        return Err(Error::validation(format!("{}: unsupported schema {}", path.display(), index.schema)));
    }
    Ok(index)
}

/// Cross-process lock of a registry: publishes from parallel agents (app, CLI) update
/// `index.json` and commit one after another.
struct RegistryLock(PathBuf);

impl RegistryLock {
    fn acquire(dir: &Path) -> Result<Self> {
        let path = dir.join(".git").join("fastdev-registry.lock");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Self(path)),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    // A lock older than 5 minutes belongs to a crashed process.
                    let stale = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .is_ok_and(|t| t.elapsed().unwrap_or_default() > std::time::Duration::from_secs(300));
                    if stale {
                        let _ = std::fs::remove_file(&path);
                    } else if std::time::Instant::now() > deadline {
                        return Err(Error::conflict(format!(
                            "registry is locked by another publish ({})",
                            path.display()
                        )));
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(200));
                    }
                }
                Err(err) => return Err(Error::io(&path, err)),
            }
        }
    }
}

impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Adds or replaces an entry of a writable registry and commits the change.
/// Returns whether anything changed.
pub fn upsert(source: &RegistrySource, entry: IndexEntry, push: bool, warnings: &mut Vec<String>) -> Result<bool> {
    if !source.writable {
        return Err(Error::not_allowed(format!("registry {} is not a local git repository", source.url)));
    }
    let _lock = RegistryLock::acquire(&source.dir)?;
    let mut index = load(source)?;
    match index.skeletons.iter_mut().find(|e| e.id == entry.id) {
        Some(existing) if *existing == entry => return Ok(false),
        Some(existing) => *existing = entry.clone(),
        None => index.skeletons.push(entry.clone()),
    }
    index.skeletons.sort_by(|a, b| a.id.cmp(&b.id));
    let text = serde_json::to_string_pretty(&index)?;
    util::write_atomic(&source.dir.join(INDEX_FILE), format!("{text}\n").as_bytes())?;
    git::run(&source.dir, &["add", INDEX_FILE])?;
    let message = format!("{}: {}", entry.id, entry.latest.as_deref().unwrap_or("update"));
    git::run(&source.dir, &["commit", "--quiet", "-m", &message])?;
    if push
        && git::remote_url(&source.dir, "origin").is_some()
        && let Err(err) = git::run(&source.dir, &["push", "--quiet", "origin", "HEAD"])
    {
        warnings.push(format!("registry push failed: {}", err.message));
    }
    Ok(true)
}

/// Creates an empty registry repository (used to bootstrap a local library).
pub fn init(dir: &Path, name: &str) -> Result<()> {
    std::fs::create_dir_all(dir).at(dir)?;
    if dir.join(INDEX_FILE).exists() {
        return Err(Error::conflict(format!("{} already has {INDEX_FILE}", dir.display())));
    }
    let index = RegistryIndex { schema: INDEX_SCHEMA, name: name.to_string(), skeletons: Vec::new() };
    util::write_atomic(&dir.join(INDEX_FILE), format!("{}\n", serde_json::to_string_pretty(&index)?).as_bytes())?;
    util::write_atomic(&dir.join("README.md"), REGISTRY_README.replace("__NAME__", name).as_bytes())?;
    if !git::is_work_tree(dir) {
        git::run(dir, &["init", "--quiet", "-b", "main"])?;
    }
    git::run(dir, &["add", "-A"])?;
    git::run(dir, &["commit", "--quiet", "-m", "Create registry"])?;
    Ok(())
}

const REGISTRY_README: &str = "# __NAME__

A registry of [fastDev](https://github.com/roma-vibe/fastDev) project skeletons. `index.json` lists skeleton
repositories; fastDev shows them in its library and downloads a skeleton (git clone) when it is
first used. Versions are the `vX.Y.Z` tags of each skeleton repository.

fastDev updates this file when a skeleton is published. To add a skeleton by hand, append an entry:

```json
{ \"id\": \"node-vue\", \"repo\": \"https://github.com/<owner>/fastdev-node-vue.git\", \"name\": \"Node + Vue\" }
```
";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_updates_a_local_registry() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("registry");
        std::fs::create_dir_all(&dir).unwrap();
        crate::git::init_repo(&dir);
        init(&dir, "Test registry").unwrap();
        let src = source(&dir.to_string_lossy(), tmp.path());
        assert!(src.writable);
        let entry = IndexEntry {
            id: "demo".into(),
            repo: "/tmp/demo".into(),
            name: "Demo".into(),
            description: String::new(),
            category: "web".into(),
            languages: vec!["node".into()],
            tags: vec![],
            stack: vec![],
            latest: Some("1.0.0".into()),
            forked_from: None,
            translations: BTreeMap::new(),
        };
        let mut warnings = Vec::new();
        assert!(upsert(&src, entry.clone(), true, &mut warnings).unwrap());
        assert!(!upsert(&src, entry, true, &mut warnings).unwrap());
        assert_eq!(load(&src).unwrap().skeletons.len(), 1);
        assert!(crate::git::is_clean(&dir));
        assert!(sync(&src).is_ok());
    }

    #[test]
    fn parallel_upserts_keep_every_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("registry");
        crate::git::init_repo(&dir);
        init(&dir, "Test").unwrap();
        let handles: Vec<_> = (0..6)
            .map(|i| {
                let src = source(&dir.to_string_lossy(), tmp.path());
                std::thread::spawn(move || {
                    let entry = IndexEntry {
                        id: format!("s{i}"),
                        repo: format!("/tmp/s{i}"),
                        name: format!("S{i}"),
                        description: String::new(),
                        category: "web".into(),
                        languages: vec![],
                        tags: vec![],
                        stack: vec![],
                        latest: Some("1.0.0".into()),
                        forked_from: None,
                        translations: BTreeMap::new(),
                    };
                    upsert(&src, entry, false, &mut Vec::new()).unwrap();
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        assert_eq!(load(&source(&dir.to_string_lossy(), tmp.path())).unwrap().skeletons.len(), 6);
        assert!(crate::git::is_clean(&dir));
    }

    #[test]
    fn remote_registries_are_cached() {
        let src = source("https://example.com/registry.git", Path::new("/data"));
        assert!(!src.writable);
        assert!(src.dir.starts_with("/data/registries"));
    }
}
