//! The skeleton library: every skeleton is its own git repository whose `vX.Y.Z` tags are the
//! published versions. Registries (`index.json`) list the repositories; fastDev keeps a mirror
//! of each repository it uses (downloaded on demand) and exports versions from it. Skeletons
//! being authored live as working clones in the workspace folder; their uncommitted state is
//! the draft.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, IoContext, Result};
use crate::git;
use crate::index::{self, IndexEntry, RegistrySource};
use crate::manifest::{CATEGORIES, ForkOrigin, MANIFEST_FILE, Manifest, SCHEMA_VERSION};
use crate::render::{self, TEMPLATE_SUFFIX};
use crate::util::{self, FORBIDDEN_DIRS};

pub const FILES_DIR: &str = "files";
/// Target label of the draft (the workspace clone).
pub const DRAFT_DIR: &str = "draft";
/// Draft state in the workspace clone (git-ignored).
pub const DRAFT_META: &str = ".fastdev-draft.toml";
pub const CHANGELOG_FILE: &str = "CHANGELOG.md";
pub const AGENTS_TEMPLATE: &str = "AGENTS.md.tmpl";
pub const SPEC_TEMPLATE: &str = "SPEC.md.tmpl";
pub const GITIGNORE_SOURCE: &str = "_gitignore";
pub const SUPPORTED_ECOSYSTEMS: &[&str] = &["npm"];
/// Interface languages besides English; manifests translate their texts into these.
pub const UI_LANGUAGES: &[&str] = &["ru"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DraftKind {
    New,
    Edit,
    Fork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    pub hash: String,
    pub passed: bool,
    pub at: String,
    #[serde(default)]
    pub job_id: String,
    /// Selections not verified because their tools are missing here (e.g. "docker=none: PHP >=8.4").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<String>,
}

/// Content of `.fastdev-draft.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftMeta {
    pub kind: DraftKind,
    /// edit: version of the same skeleton; fork: version of `source`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<Verification>,
}

impl DraftMeta {
    pub fn load(draft_dir: &Path) -> Option<Self> {
        let text = fs::read_to_string(draft_dir.join(DRAFT_META)).ok()?;
        toml::from_str(&text).ok()
    }

    pub fn save(&self, draft_dir: &Path) -> Result<()> {
        let text = toml::to_string_pretty(self).map_err(|err| Error::internal(err.to_string()))?;
        util::write_atomic(&draft_dir.join(DRAFT_META), text.as_bytes())
    }
}

/// A draft or a published version of a skeleton.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Draft,
    Version(semver::Version),
}

impl Target {
    pub fn label(&self) -> String {
        match self {
            Self::Draft => DRAFT_DIR.to_string(),
            Self::Version(v) => v.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftInfo {
    pub kind: DraftKind,
    pub based_on: Option<String>,
    pub source: Option<String>,
    pub created_at: String,
    pub path: String,
    pub verification: Option<Verification>,
}

/// Where a skeleton comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkeletonSource {
    /// Listed in a registry.
    Registry,
    /// Added by repository URL.
    Custom,
    /// Only in the workspace (not published to a registry yet).
    Workspace,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkeletonSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub languages: Vec<String>,
    pub tags: Vec<String>,
    pub stack: Vec<String>,
    /// Downloaded versions, newest first.
    pub versions: Vec<String>,
    pub latest: Option<String>,
    pub draft: Option<DraftInfo>,
    pub forked_from: Option<ForkOrigin>,
    pub source: SkeletonSource,
    /// Git URL or local path of the repository.
    pub repo: Option<String>,
    /// The repository has been downloaded (mirror exists).
    pub downloaded: bool,
    /// Working clone in the workspace, when there is one.
    pub workspace_path: Option<String>,
    /// Translations of name and description: language → English text → translation.
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
    /// Set when the metadata cannot be read.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangelogEntry {
    pub version: String,
    pub date: String,
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub level: IssueLevel,
    pub file: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub id: String,
    pub target: String,
    pub valid: bool,
    pub errors: Vec<Issue>,
    pub warnings: Vec<Issue>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncItem {
    pub name: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    pub registries: Vec<SyncItem>,
    pub skeletons: Vec<SyncItem>,
}

/// A skeleton known from a registry or a custom source.
#[derive(Debug, Clone)]
struct KnownSkeleton {
    entry: IndexEntry,
    source: SkeletonSource,
}

#[derive(Debug, Clone)]
pub struct Library {
    data_dir: PathBuf,
    workspace: PathBuf,
    registries: Vec<String>,
    sources: Vec<String>,
}

impl Library {
    pub fn new(
        data_dir: impl Into<PathBuf>,
        workspace: impl Into<PathBuf>,
        registries: Vec<String>,
        sources: Vec<String>,
    ) -> Self {
        Self { data_dir: data_dir.into(), workspace: workspace.into(), registries, sources }
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn registries(&self) -> &[String] {
        &self.registries
    }

    fn cache_dir(&self, id: &str) -> PathBuf {
        self.data_dir.join("skeletons").join(id)
    }

    pub fn mirror_dir(&self, id: &str) -> PathBuf {
        self.cache_dir(id).join("repo.git")
    }

    /// The workspace clone of a skeleton (the draft area).
    pub fn draft_dir(&self, id: &str) -> PathBuf {
        self.workspace.join(id)
    }

    pub fn registry_sources(&self) -> Vec<RegistrySource> {
        self.registries.iter().map(|url| index::source(url, &self.data_dir)).collect()
    }

    /// First registry that is a local git repository (updated on publish).
    pub fn writable_registry(&self) -> Option<RegistrySource> {
        self.registry_sources().into_iter().find(|s| s.writable && s.dir.join(index::INDEX_FILE).is_file())
    }

    /// Skeletons from registries (first registry wins) and custom sources, without network access.
    fn known(&self) -> Vec<KnownSkeleton> {
        let mut known: Vec<KnownSkeleton> = Vec::new();
        for source in self.registry_sources() {
            let Ok(index) = index::load(&source) else { continue };
            for entry in index.skeletons {
                // Ids name cache folders; an entry like `../x` from a foreign index is ignored.
                if util::is_valid_id(&entry.id) && !known.iter().any(|k| k.entry.id == entry.id) {
                    known.push(KnownSkeleton { entry, source: SkeletonSource::Registry });
                }
            }
        }
        for url in &self.sources {
            if let Some(entry) = self.custom_entry(url)
                && util::is_valid_id(&entry.id)
                && !known.iter().any(|k| k.entry.id == entry.id)
            {
                known.push(KnownSkeleton { entry, source: SkeletonSource::Custom });
            }
        }
        known
    }

    /// Entry for a custom source, read from its (already downloaded) mirror.
    fn custom_entry(&self, url: &str) -> Option<IndexEntry> {
        let skeletons = self.data_dir.join("skeletons");
        for dir in fs::read_dir(&skeletons).ok()?.flatten() {
            let mirror = dir.path().join("repo.git");
            if git::remote_url(&mirror, "origin").as_deref() == Some(url) {
                let tags = git::version_tags(&mirror);
                let rev = tags.first().map(|(v, _)| format!("{}{v}", git::TAG_PREFIX)).unwrap_or_else(|| "HEAD".into());
                let manifest = Manifest::parse(&git::show(&mirror, &rev, MANIFEST_FILE).ok()?).ok()?;
                let latest = tags.first().map(|(v, _)| v.to_string()).unwrap_or_default();
                return Some(IndexEntry::from_manifest(&manifest, url, &latest));
            }
        }
        None
    }

    fn workspace_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = fs::read_dir(&self.workspace)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.path().join(MANIFEST_FILE).is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| util::is_valid_id(name))
            .collect();
        ids.sort();
        ids
    }

    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.known().into_iter().map(|k| k.entry.id).collect();
        for id in self.workspace_ids() {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids
    }

    pub fn exists(&self, id: &str) -> bool {
        self.known().iter().any(|k| k.entry.id == id) || self.draft_dir(id).join(MANIFEST_FILE).is_file()
    }

    /// Repository of a skeleton: the registry/source URL, or the workspace clone.
    pub fn repo_url(&self, id: &str) -> Option<String> {
        if let Some(known) = self.known().into_iter().find(|k| k.entry.id == id) {
            return Some(known.entry.repo);
        }
        let workspace = self.draft_dir(id);
        git::is_work_tree(&workspace)
            .then(|| git::remote_url(&workspace, "origin").unwrap_or_else(|| workspace.to_string_lossy().into_owned()))
    }

    pub fn is_downloaded(&self, id: &str) -> bool {
        git::is_bare(&self.mirror_dir(id))
    }

    /// Clones the repository into the cache when it is not there yet ("download on demand").
    pub fn ensure_mirror(&self, id: &str) -> Result<PathBuf> {
        let mirror = self.mirror_dir(id);
        if git::is_bare(&mirror) {
            return Ok(mirror);
        }
        let url =
            self.repo_url(id).ok_or_else(|| Error::not_found(format!("skeleton \"{id}\" is not in any registry")))?;
        git::clone_mirror(&url, &mirror)
            .map_err(|err| Error::command(format!("cannot download skeleton \"{id}\" from {url}: {}", err.message)))?;
        Ok(mirror)
    }

    /// Downloads new versions of a skeleton.
    pub fn refresh(&self, id: &str) -> Result<()> {
        let mirror = self.ensure_mirror(id)?;
        let url = self.repo_url(id).ok_or_else(|| Error::not_found(format!("skeleton \"{id}\" has no repository")))?;
        git::fetch_mirror(&mirror, &url)?;
        // A downloaded version stays fixed by its commit (SPEC §5.2): when the tag of a remote
        // repository moves, fastDev keeps the export and validation warns. Only a local repository
        // may re-publish a version in place (development), and then its export is refreshed.
        if index::local_path(&url).is_none() {
            return Ok(());
        }
        for (version, commit) in git::version_tags(&mirror) {
            if self.version_commit(id, &version).is_some_and(|exported| exported != commit) {
                self.forget_version(id, &version)?;
            }
        }
        Ok(())
    }

    /// Pulls registries and fetches every downloaded skeleton.
    pub fn sync(&self) -> SyncReport {
        let item = |name: String, result: Result<()>| SyncItem {
            name,
            ok: result.is_ok(),
            error: result.err().map(|e| e.message),
        };
        let registries = self.registry_sources().iter().map(|s| item(s.url.clone(), index::sync(s))).collect();
        let skeletons = self.ids().into_iter().filter(|id| self.is_downloaded(id)).map(|id| {
            let result = self.refresh(&id);
            item(id, result)
        });
        SyncReport { registries, skeletons: skeletons.collect() }
    }

    /// Downloaded versions (no network), newest first.
    pub fn cached_versions(&self, id: &str) -> Vec<semver::Version> {
        git::version_tags(&self.mirror_dir(id)).into_iter().map(|(v, _)| v).collect()
    }

    /// Published versions, downloading the repository when needed, newest first.
    pub fn versions(&self, id: &str) -> Result<Vec<semver::Version>> {
        self.ensure_mirror(id)?;
        Ok(self.cached_versions(id))
    }

    pub fn latest(&self, id: &str) -> Option<semver::Version> {
        self.cached_versions(id).into_iter().next()
    }

    /// Whether the workspace clone differs from the latest release (or was opened as a draft).
    pub fn has_draft(&self, id: &str) -> bool {
        let dir = self.draft_dir(id);
        if !dir.join(MANIFEST_FILE).is_file() {
            return false;
        }
        if dir.join(DRAFT_META).is_file() || !git::is_work_tree(&dir) || !git::is_clean(&dir) {
            return true;
        }
        match git::head(&dir) {
            Some(head) => !git::version_tags(&dir).iter().any(|(_, commit)| *commit == head),
            None => true,
        }
    }

    /// `None` → draft when present, otherwise the latest version.
    pub fn resolve_target(&self, id: &str, target: Option<&str>) -> Result<Target> {
        if !self.exists(id) {
            return Err(Error::not_found(format!("skeleton \"{id}\" is not in the library; try sync_library")));
        }
        match target.map(str::trim).filter(|t| !t.is_empty()) {
            Some(DRAFT_DIR) => {
                if self.has_draft(id) {
                    Ok(Target::Draft)
                } else {
                    Err(Error::not_found(format!("skeleton \"{id}\" has no draft")))
                }
            }
            Some("latest") => self.resolve_published(id, None),
            Some(version) => self.resolve_published(id, Some(version)),
            None if self.has_draft(id) => Ok(Target::Draft),
            None => self.resolve_published(id, None),
        }
    }

    /// Only published versions (for creating projects); `None` → latest.
    pub fn resolve_published(&self, id: &str, version: Option<&str>) -> Result<Target> {
        let versions = self.versions(id)?;
        match version.map(str::trim).filter(|v| !v.is_empty() && *v != "latest") {
            Some(DRAFT_DIR) => Err(Error::invalid("projects are created from published versions, not drafts")),
            Some(text) => {
                let wanted = semver::Version::parse(text.trim_start_matches(git::TAG_PREFIX))
                    .map_err(|_| Error::invalid(format!("\"{text}\" is not a version or \"draft\"")))?;
                versions
                    .into_iter()
                    .find(|v| *v == wanted)
                    .map(Target::Version)
                    .ok_or_else(|| Error::not_found(format!("skeleton \"{id}\" has no version {wanted}")))
            }
            None => versions
                .into_iter()
                .next()
                .map(Target::Version)
                .ok_or_else(|| Error::not_found(format!("skeleton \"{id}\" has no published version yet"))),
        }
    }

    /// Folder with `template.toml` and `files/`: the workspace clone for the draft, an exported
    /// copy of the tag for versions (exported once, then reused).
    pub fn target_dir(&self, id: &str, target: &Target) -> Result<PathBuf> {
        match target {
            Target::Draft => Ok(self.draft_dir(id)),
            Target::Version(version) => {
                let dir = self.cache_dir(id).join("versions").join(version.to_string());
                if dir.join(MANIFEST_FILE).is_file() {
                    return Ok(dir);
                }
                let mirror = self.ensure_mirror(id)?;
                let (_, commit) = git::version_tags(&mirror)
                    .into_iter()
                    .find(|(v, _)| v == version)
                    .ok_or_else(|| Error::not_found(format!("skeleton \"{id}\" has no version {version}")))?;
                let tmp = self.cache_dir(id).join("versions").join(format!(".{version}-{}", util::short_id()));
                git::export(&mirror, &commit, &tmp)?;
                if dir.exists() {
                    fs::remove_dir_all(&dir).at(&dir)?;
                }
                fs::rename(&tmp, &dir).at(&dir)?;
                util::write_atomic(&self.commit_file(id, version), commit.as_bytes())?;
                Ok(dir)
            }
        }
    }

    fn commit_file(&self, id: &str, version: &semver::Version) -> PathBuf {
        self.cache_dir(id).join("versions").join(format!("{version}.commit"))
    }

    /// Drops the exported copy of a version (it is exported again from the tag on next use).
    pub fn forget_version(&self, id: &str, version: &semver::Version) -> Result<()> {
        let dir = self.cache_dir(id).join("versions").join(version.to_string());
        if dir.exists() {
            fs::remove_dir_all(&dir).at(&dir)?;
        }
        let commit = self.commit_file(id, version);
        if commit.exists() {
            fs::remove_file(&commit).at(&commit)?;
        }
        Ok(())
    }

    /// Commit a downloaded version was exported from.
    pub fn version_commit(&self, id: &str, version: &semver::Version) -> Option<String> {
        fs::read_to_string(self.commit_file(id, version)).ok().map(|s| s.trim().to_string())
    }

    pub fn load_manifest(&self, id: &str, target: &Target) -> Result<Manifest> {
        Manifest::load(&self.target_dir(id, target)?)
    }

    pub fn draft_info(&self, id: &str) -> Option<DraftInfo> {
        if !self.has_draft(id) {
            return None;
        }
        let dir = self.draft_dir(id);
        let tags = git::version_tags(&dir);
        let meta = DraftMeta::load(&dir).unwrap_or(DraftMeta {
            kind: if tags.is_empty() { DraftKind::New } else { DraftKind::Edit },
            based_on: tags.first().map(|(v, _)| v.to_string()),
            source: None,
            created_at: String::new(),
            verification: None,
        });
        Some(DraftInfo {
            kind: meta.kind,
            based_on: meta.based_on,
            source: meta.source,
            created_at: meta.created_at,
            path: dir.to_string_lossy().into_owned(),
            verification: meta.verification,
        })
    }

    pub fn summary(&self, id: &str) -> SkeletonSummary {
        let known = self.known().into_iter().find(|k| k.entry.id == id);
        let versions = self.cached_versions(id);
        let draft = self.draft_info(id);
        let workspace = self.draft_dir(id);
        // Metadata: latest downloaded version, else the registry entry, else the draft.
        let from_version = versions.first().and_then(|v| {
            let rev = format!("{}{v}", git::TAG_PREFIX);
            git::show(&self.mirror_dir(id), &rev, MANIFEST_FILE).ok().and_then(|text| Manifest::parse(&text).ok())
        });
        let from_draft = || Manifest::load(&workspace);
        let (meta, error) = match (from_version, &known) {
            (Some(m), _) => (Some(IndexEntry::from_manifest(&m, "", "")), None),
            (None, Some(k)) => (Some(k.entry.clone()), None),
            (None, None) => match from_draft() {
                Ok(m) => (Some(IndexEntry::from_manifest(&m, "", "")), None),
                Err(err) => (None, Some(err.message)),
            },
        };
        let meta = meta.unwrap_or_else(|| IndexEntry {
            id: id.to_string(),
            repo: String::new(),
            name: id.to_string(),
            description: String::new(),
            category: "other".into(),
            languages: vec![],
            tags: vec![],
            stack: vec![],
            latest: None,
            forked_from: None,
            translations: BTreeMap::new(),
        });
        let latest =
            versions.first().map(ToString::to_string).or_else(|| known.as_ref().and_then(|k| k.entry.latest.clone()));
        SkeletonSummary {
            id: id.to_string(),
            name: meta.name,
            description: meta.description,
            category: meta.category,
            languages: meta.languages,
            tags: meta.tags,
            stack: meta.stack,
            versions: versions.iter().map(ToString::to_string).collect(),
            latest,
            draft,
            forked_from: meta.forked_from,
            source: known.as_ref().map_or(SkeletonSource::Workspace, |k| k.source),
            repo: self.repo_url(id),
            downloaded: self.is_downloaded(id),
            workspace_path: workspace.join(MANIFEST_FILE).is_file().then(|| workspace.to_string_lossy().into_owned()),
            translations: meta.translations,
            error,
        }
    }

    pub fn list(&self) -> Vec<SkeletonSummary> {
        self.ids().iter().map(|id| self.summary(id)).collect()
    }

    /// Changelog of the latest downloaded version, or of the workspace clone.
    pub fn changelog(&self, id: &str) -> Vec<ChangelogEntry> {
        let text = self
            .latest(id)
            .and_then(|v| git::show(&self.mirror_dir(id), &format!("{}{v}", git::TAG_PREFIX), CHANGELOG_FILE).ok())
            .or_else(|| fs::read_to_string(self.draft_dir(id).join(CHANGELOG_FILE)).ok())
            .unwrap_or_default();
        parse_changelog(&text)
    }

    pub fn validate(&self, id: &str, target: &Target) -> ValidationReport {
        let mut v = Validator::default();
        match self.target_dir(id, target) {
            Ok(dir) => v.run(self, id, target, &dir),
            Err(err) => v.error(None, err.message),
        }
        ValidationReport {
            id: id.to_string(),
            target: target.label(),
            valid: v.errors.is_empty(),
            errors: v.errors,
            warnings: v.warnings,
        }
    }

    /// Adds an existing skeleton repository by URL: downloads it and returns its id.
    pub fn download_source(&self, url: &str) -> Result<String> {
        let tmp = self.data_dir.join("skeletons").join(format!(".incoming-{}", util::short_id()));
        git::clone_mirror(url, &tmp)
            .map_err(|err| Error::command(format!("cannot download {url}: {}", err.message)))?;
        let outcome = (|| {
            let tags = git::version_tags(&tmp);
            let rev = tags.first().map(|(v, _)| format!("{}{v}", git::TAG_PREFIX)).unwrap_or_else(|| "HEAD".into());
            let manifest = git::show(&tmp, &rev, MANIFEST_FILE)
                .map_err(|_| Error::validation(format!("{url} is not a fastDev skeleton (no {MANIFEST_FILE})")))
                .and_then(|text| Manifest::parse(&text))?;
            if tags.is_empty() {
                return Err(Error::validation(format!("{url} has no published version (vX.Y.Z tag)")));
            }
            let id = manifest.id.clone();
            if !util::is_valid_id(&id) {
                return Err(Error::validation(format!("{url}: invalid skeleton id \"{id}\"")));
            }
            let mirror = self.mirror_dir(&id);
            if git::is_bare(&mirror) {
                if git::remote_url(&mirror, "origin").as_deref() == Some(url) {
                    return Ok(id);
                }
                return Err(Error::conflict(format!("a skeleton with id \"{id}\" already exists")));
            }
            fs::create_dir_all(self.cache_dir(&id)).at(&self.cache_dir(&id))?;
            fs::rename(&tmp, &mirror).at(&mirror)?;
            Ok(id)
        })();
        if tmp.exists() {
            let _ = fs::remove_dir_all(&tmp);
        }
        outcome
    }
}

/// A TOML key for a translation table: bare when possible, else a basic string.
fn toml_key(text: &str) -> String {
    toml_edit::Key::new(text).display_repr().into_owned()
}

fn parse_changelog(text: &str) -> Vec<ChangelogEntry> {
    let mut entries: Vec<ChangelogEntry> = Vec::new();
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            let (version, date) = header
                .split_once(" — ")
                .or_else(|| header.split_once(" - "))
                .map(|(v, d)| (v.trim().to_string(), d.trim().to_string()))
                .unwrap_or_else(|| (header.trim().to_string(), String::new()));
            entries.push(ChangelogEntry { version, date, body: String::new() });
        } else if let Some(entry) = entries.last_mut() {
            entry.body.push_str(line);
            entry.body.push('\n');
        }
    }
    for entry in &mut entries {
        entry.body = entry.body.trim().to_string();
    }
    entries
}

/// Removes the section of `version` from `CHANGELOG.md` in `dir` (used when re-publishing).
pub fn remove_changelog_entry(dir: &Path, version: &str) -> Result<()> {
    let path = dir.join(CHANGELOG_FILE);
    let Ok(text) = fs::read_to_string(&path) else { return Ok(()) };
    let mut out = Vec::new();
    let mut skipping = false;
    for line in text.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            skipping = header.split_whitespace().next() == Some(version);
        }
        if !skipping {
            out.push(line);
        }
    }
    util::write_atomic(&path, format!("{}\n", out.join("\n").trim_end()).as_bytes())
}

/// Prepends a section to `CHANGELOG.md` in `dir`.
pub fn add_changelog_entry(dir: &Path, version: &str, changes: &[String], note: Option<&str>) -> Result<()> {
    let path = dir.join(CHANGELOG_FILE);
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let body = existing.strip_prefix("# Changelog").map(str::trim_start).unwrap_or(existing.trim_start());
    let mut section = format!("## {version} — {}\n\n", util::today());
    if let Some(note) = note {
        section.push_str(&format!("_{note}_\n\n"));
    }
    for change in changes {
        section.push_str(&format!("- {}\n", change.trim()));
    }
    let text = format!("# Changelog\n\n{section}\n{body}");
    util::write_atomic(&path, format!("{}\n", text.trim_end()).as_bytes())
}

#[derive(Default)]
struct Validator {
    errors: Vec<Issue>,
    warnings: Vec<Issue>,
}

impl Validator {
    fn error(&mut self, file: Option<&str>, message: impl Into<String>) {
        self.errors.push(Issue { level: IssueLevel::Error, file: file.map(str::to_string), message: message.into() });
    }

    fn warn(&mut self, file: Option<&str>, message: impl Into<String>) {
        self.warnings.push(Issue {
            level: IssueLevel::Warning,
            file: file.map(str::to_string),
            message: message.into(),
        });
    }

    fn run(&mut self, library: &Library, id: &str, target: &Target, dir: &Path) {
        let manifest = match Manifest::load(dir) {
            Ok(m) => m,
            Err(err) => {
                self.error(Some(MANIFEST_FILE), err.message);
                return;
            }
        };
        self.check_manifest(library, id, &manifest);
        if let Ok(text) = fs::read_to_string(dir.join(MANIFEST_FILE)) {
            self.check_deprecated(&text);
        }

        let files_dir = dir.join(FILES_DIR);
        if !files_dir.is_dir() {
            self.error(Some(FILES_DIR), "the files/ folder is missing");
            return;
        }
        let files = match all_files(&files_dir) {
            Ok(files) => files,
            Err(err) => {
                self.error(Some(FILES_DIR), err.message);
                return;
            }
        };
        self.check_files(&manifest, &files);
        self.check_templates(&manifest, &files_dir, &files);

        // Published versions are fixed by their commit; warn when the tag was moved since download.
        if let Target::Version(version) = target {
            let current = git::version_tags(&library.mirror_dir(id)).into_iter().find(|(v, _)| v == version);
            if let (Some((_, now)), Some(downloaded)) = (current, library.version_commit(id, version))
                && now != downloaded
            {
                self.warn(
                    None,
                    format!("tag v{version} was moved to another commit after it was downloaded; fastDev keeps using {downloaded}"),
                );
            }
        }
    }

    fn check_manifest(&mut self, library: &Library, id: &str, m: &Manifest) {
        let file = Some(MANIFEST_FILE);
        if m.schema != SCHEMA_VERSION {
            self.error(file, format!("schema must be {SCHEMA_VERSION}"));
        }
        if m.id != id {
            self.error(file, format!("id \"{}\" does not match the folder name \"{id}\"", m.id));
        }
        if !util::is_valid_id(&m.id) {
            self.error(file, "id must use lowercase letters, digits and single dashes, starting with a letter");
        }
        if m.name.trim().is_empty() {
            self.error(file, "name is empty");
        }
        if m.description.trim().is_empty() {
            self.warn(file, "description is empty");
        }
        if !CATEGORIES.contains(&m.category.as_str()) {
            self.error(file, format!("category must be one of: {}", CATEGORIES.join(", ")));
        }
        if m.languages.is_empty() {
            self.warn(file, "languages is empty; the skeleton will not appear in language filters");
        }
        let mut requirements: Vec<(&String, &String)> = m.requirements.iter().collect();
        for feature in m.features.values() {
            requirements.extend(feature.requirements.iter());
        }
        for (tool, req) in requirements {
            if semver::VersionReq::parse(req).is_err() {
                self.error(file, format!("requirement {tool} = \"{req}\" is not a valid semver requirement"));
            }
        }
        for (key, cmd) in &m.commands {
            if cmd.run.trim().is_empty() {
                self.error(file, format!("commands.{key}.run is empty"));
            }
            if cmd.label.trim().is_empty() {
                self.warn(file, format!("commands.{key}.label is empty"));
            }
            if let Some(feature) = &cmd.feature
                && !m.features.contains_key(feature)
            {
                self.error(file, format!("commands.{key}.feature \"{feature}\" is not defined in [features]"));
            }
        }
        if m.commands.values().filter(|c| c.primary).count() > 1 {
            self.warn(file, "more than one command is marked primary");
        }
        let option_inputs = m.choices.iter().flat_map(|(name, choice)| {
            choice.options.iter().flat_map(move |(option, o)| {
                o.commands.iter().filter_map(move |(key, c)| {
                    Some((format!("choices.{name}.options.{option}.commands.{key}"), c.run.clone(), c.inputs.clone()?))
                })
            })
        });
        let all_inputs: Vec<(String, Option<String>, Vec<crate::manifest::CommandInput>)> = m
            .commands
            .iter()
            .map(|(key, c)| (format!("commands.{key}"), Some(c.run.clone()), c.inputs.clone()))
            .chain(option_inputs)
            .collect();
        for (at, run, inputs) in &all_inputs {
            let mut seen = std::collections::BTreeSet::new();
            for input in inputs {
                let valid = input.name.starts_with(|c: char| c.is_ascii_uppercase())
                    && input.name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
                if !valid {
                    self.error(file, format!("{at}.inputs: \"{}\" must be an UPPER_CASE variable name", input.name));
                }
                if !seen.insert(&input.name) {
                    self.error(file, format!("{at}.inputs: \"{}\" is listed twice", input.name));
                }
                if input.label.trim().is_empty() {
                    self.error(file, format!("{at}.inputs.{}: label is empty", input.name));
                }
                if let Some(run) = run
                    && !run.contains(&format!("${}", input.name))
                    && !run.contains(&format!("${{{}", input.name))
                {
                    self.warn(
                        file,
                        format!("{at}: input {} is not used in `run` (write \"${}\")", input.name, input.name),
                    );
                }
            }
        }
        for (key, cmd) in &m.commands {
            if cmd.primary && !cmd.inputs.is_empty() {
                self.error(file, format!("commands.{key}: the primary command cannot ask for inputs"));
            }
        }
        for step in &m.setup {
            if let Some(feature) = &step.feature
                && !m.features.contains_key(feature)
            {
                self.error(file, format!("setup step \"{}\" uses unknown feature \"{feature}\"", step.label));
            }
        }
        let option_commands = || m.choices.values().flat_map(|c| c.options.values()).flat_map(|o| o.commands.iter());
        let known = |key: &str| m.commands.contains_key(key) || option_commands().any(|(k, _)| k == key);
        for key in &m.verify.commands {
            let long = m.commands.get(key).is_some_and(|c| c.long)
                || option_commands().any(|(k, c)| k == key && c.long == Some(true));
            let asks = m.commands.get(key).is_some_and(|c| !c.inputs.is_empty());
            if !known(key) {
                self.error(file, format!("verify.commands references unknown command \"{key}\""));
            } else if asks {
                self.error(file, format!("verify.commands cannot run \"{key}\": it asks for inputs"));
            } else if long {
                self.error(file, format!("verify.commands must not contain long-running command \"{key}\""));
            }
        }
        let preview_commands = m.preview.iter().filter_map(|p| p.command.clone()).chain(
            m.choices.values().flat_map(|c| c.options.values()).filter_map(|o| o.preview.as_ref()?.command.clone()),
        );
        for command in preview_commands.collect::<Vec<_>>() {
            if !known(&command) {
                self.error(file, format!("preview command \"{command}\" is not defined"));
            }
        }
        self.check_translations(m);
        if m.preview.is_none() && !m.commands.values().any(|c| c.primary || c.long) {
            self.warn(file, "no [preview] and no primary or long-running command: the skeleton cannot be previewed");
        }
        if m.verify.commands.is_empty() {
            self.warn(file, "verify.commands is empty; verification only runs setup");
        }
        if let Some(updates) = &m.updates
            && !SUPPORTED_ECOSYSTEMS.contains(&updates.ecosystem.as_str())
        {
            self.warn(file, format!("updates.ecosystem \"{}\" is not supported yet", updates.ecosystem));
        }
        for (package, range) in m.updates.iter().flat_map(|u| &u.hold) {
            if semver::VersionReq::parse(range).is_err() {
                self.error(file, format!("updates.hold.{package}: \"{range}\" is not a version range (e.g. \"<6.1\")"));
            }
        }
        let option_ports = m.choices.iter().flat_map(|(name, choice)| {
            choice.options.iter().flat_map(move |(key, option)| {
                option
                    .ports
                    .iter()
                    .map(move |(port, value)| (format!("choices.{name}.options.{key}.ports"), port, value))
            })
        });
        let all_ports: Vec<(String, &String, &u16)> =
            m.ports.iter().map(|(k, v)| ("ports".to_string(), k, v)).chain(option_ports).collect();
        let option_env: Vec<&String> =
            m.choices.values().flat_map(|c| c.options.values()).flat_map(|o| o.env.keys()).collect();
        for (at, key, value) in &all_ports {
            if m.env.contains_key(*key) || option_env.contains(key) {
                self.error(file, format!("{key} is defined both as env and as a port ({at})"));
            }
            if **value == 0 {
                self.error(file, format!("{at}.{key} must be greater than 0"));
            }
        }
        for (key, bytes) in &m.secrets {
            if *bytes == 0 || *bytes > 256 {
                self.error(file, format!("secrets.{key} must be between 1 and 256 bytes"));
            }
        }
        for edit in &m.set {
            if let Some(feature) = &edit.feature
                && !m.features.contains_key(feature)
            {
                self.error(file, format!("[[set]] for {} uses unknown feature \"{feature}\"", edit.file));
            }
        }
        self.check_choices(m);
        if let Some(origin) = &m.forked_from
            && !library.exists(&origin.id)
        {
            self.warn(file, format!("forked_from skeleton \"{}\" is not in this library", origin.id));
        }
    }

    fn check_choices(&mut self, m: &Manifest) {
        let file = Some(MANIFEST_FILE);
        let names: Vec<&String> = m.choices.keys().collect();
        for (index, (name, choice)) in m.choices.iter().enumerate() {
            if m.features.contains_key(name) {
                self.error(file, format!("\"{name}\" is both a feature and a choice"));
            }
            if choice.options.is_empty() {
                self.error(file, format!("choices.{name} has no options"));
            }
            if !choice.options.contains_key(&choice.default) {
                self.error(file, format!("choices.{name}.default \"{}\" is not one of its options", choice.default));
            }
            for (other, values) in &choice.when {
                // `when` may only look at choices declared before this one.
                match m.choices.get(other) {
                    Some(target) if names[..index].contains(&other) => {
                        for value in values.values() {
                            if !target.options.contains_key(value) {
                                self.error(
                                    file,
                                    format!("choices.{name}.when: \"{value}\" is not an option of {other}"),
                                );
                            }
                        }
                    }
                    _ => self.error(
                        file,
                        format!("choices.{name}.when must refer to a choice declared before it, not \"{other}\""),
                    ),
                }
            }
            for (option_name, option) in &choice.options {
                let at = format!("choices.{name}.options.{option_name}");
                if option.label.trim().is_empty() {
                    self.warn(file, format!("{at}.label is empty"));
                }
                for (tool, req) in &option.requirements {
                    if semver::VersionReq::parse(req).is_err() {
                        self.error(
                            file,
                            format!("{at}: requirement {tool} = \"{req}\" is not a valid semver requirement"),
                        );
                    }
                }
                for tool in &option.drop_requirements {
                    if !m.requirements.contains_key(tool) {
                        self.warn(file, format!("{at}.drop_requirements: \"{tool}\" is not in [requirements]"));
                    }
                }
                for (key, command) in &option.commands {
                    if !m.commands.contains_key(key) && command.run.as_deref().is_none_or(|r| r.trim().is_empty()) {
                        self.error(file, format!("{at}.commands.{key} adds a command, so it needs `run`"));
                    }
                    if let Some(feature) = command.feature.as_deref().filter(|f| !f.is_empty())
                        && !m.features.contains_key(feature)
                    {
                        self.error(
                            file,
                            format!("{at}.commands.{key}.feature \"{feature}\" is not defined in [features]"),
                        );
                    }
                }
                for step in option.setup.iter().flatten() {
                    if let Some(feature) = &step.feature
                        && !m.features.contains_key(feature)
                    {
                        self.error(
                            file,
                            format!("{at}: setup step \"{}\" uses unknown feature \"{feature}\"", step.label),
                        );
                    }
                }
            }
        }
        let mut verified: Vec<BTreeMap<String, String>> = m.resolve_choices(&BTreeMap::new()).into_iter().collect();
        for (index, variant) in m.verify.variants.iter().enumerate() {
            match m.resolve_choices(variant) {
                Ok(choices) => verified.push(choices),
                Err(err) => self.error(file, format!("verify.variants[{index}]: {}", err.message)),
            }
        }
        // Every option must be exercised by the default selection or a variant.
        for (name, choice) in &m.choices {
            for key in choice.options.keys() {
                if !verified.iter().any(|choices| choices.get(name) == Some(key)) {
                    self.warn(
                        file,
                        format!("choices.{name}.options.{key} is never verified: add it to a [verify] variants entry"),
                    );
                }
            }
        }
    }

    fn check_files(&mut self, m: &Manifest, files: &[String]) {
        for rel in files {
            if let Some(bad) = rel.split('/').find(|part| FORBIDDEN_DIRS.contains(part)) {
                self.error(Some(rel), format!("\"{bad}\" folders must not be part of a skeleton"));
            }
        }
        for rel in files {
            let output = crate::generator::output_path(&crate::generator::parse_variant(rel).dest);
            if output == ".env" || output.ends_with("/.env") {
                self.error(Some(rel), ".env files must not be shipped (they hold local settings); ship .env.example");
            }
        }
        if files.iter().any(|f| f == ".gitignore" || f.ends_with("/.gitignore")) {
            self.warn(Some(".gitignore"), "name it _gitignore; a real .gitignore affects the library repository");
        }
        // Final project paths (conditions and `.tmpl` removed, `_gitignore` renamed).
        let outputs: Vec<String> =
            files.iter().map(|f| crate::generator::output_path(&crate::generator::parse_variant(f).dest)).collect();
        let has_output = |name: &str| outputs.iter().any(|o| o == name);
        let needs_env = !m.env.is_empty() || !m.ports.is_empty() || !m.secrets.is_empty();
        if needs_env && !has_output(".env.example") {
            self.error(
                Some(".env.example"),
                ".env.example (or .env.example.tmpl) is required when [env], [ports] or [secrets] are used",
            );
        }
        for (output, name) in [("AGENTS.md", AGENTS_TEMPLATE), ("SPEC.md", SPEC_TEMPLATE)] {
            if !files.iter().any(|f| crate::generator::parse_variant(f).dest == name) {
                self.error(Some(name), format!("{name} is missing; every project gets {output}"));
            }
        }
        self.check_variants(m, files);
        for (feature_name, feature) in &m.features {
            if feature.files.is_empty() && feature.env.is_empty() {
                self.warn(Some(MANIFEST_FILE), format!("feature \"{feature_name}\" has no files and no env"));
            }
            for pattern in &feature.files {
                match crate::generator::pattern_matcher(std::slice::from_ref(pattern)) {
                    Ok(matcher) => {
                        if !files.iter().any(|f| {
                            crate::generator::matches(&matcher, f)
                                || crate::generator::matches(&matcher, &crate::generator::parse_variant(f).dest)
                        }) {
                            self.error(
                                Some(MANIFEST_FILE),
                                format!("features.{feature_name}.files: \"{pattern}\" matches no file"),
                            );
                        }
                    }
                    Err(err) => self.error(Some(MANIFEST_FILE), err.message),
                }
            }
        }
        for (choice_name, choice) in &m.choices {
            for (option_name, option) in &choice.options {
                for pattern in &option.files {
                    match crate::generator::pattern_matcher(std::slice::from_ref(pattern)) {
                        Ok(matcher) => {
                            if !files.iter().any(|f| {
                                crate::generator::matches(&matcher, f)
                                    || crate::generator::matches(&matcher, &crate::generator::parse_variant(f).dest)
                            }) {
                                self.error(
                                    Some(MANIFEST_FILE),
                                    format!("choices.{choice_name}.options.{option_name}.files: \"{pattern}\" matches no file"),
                                );
                            }
                        }
                        Err(err) => self.error(Some(MANIFEST_FILE), err.message),
                    }
                }
            }
        }
        for edit in &m.set {
            let target = edit.file.as_str();
            if !has_output(target) {
                self.error(Some(MANIFEST_FILE), format!("[[set]] file \"{target}\" does not exist in files/"));
            }
            if crate::edits::format_of(target, edit.format.as_deref()).is_none() {
                self.error(
                    Some(MANIFEST_FILE),
                    format!("[[set]] file \"{target}\": add format = \"json\" or \"toml\" (unknown file type)"),
                );
            }
        }
    }

    /// `@key=value` file conditions: known keys and values, and no two files that end up at the
    /// same project path for any selection.
    fn check_variants(&mut self, m: &Manifest, files: &[String]) {
        let variants: Vec<(String, crate::generator::FileVariant)> =
            files.iter().map(|f| (f.clone(), crate::generator::parse_variant(f))).collect();
        let mut dimensions: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (rel, variant) in &variants {
            for (key, values) in &variant.conditions {
                let allowed: Vec<String> = if let Some(choice) = m.choices.get(key) {
                    let mut all: Vec<String> = choice.options.keys().cloned().collect();
                    if !choice.when.is_empty() {
                        all.push(String::new());
                    }
                    all
                } else if m.features.contains_key(key) {
                    vec!["on".into(), "off".into()]
                } else {
                    self.error(Some(rel), format!("condition @{key}=…: \"{key}\" is not a choice or feature"));
                    continue;
                };
                for value in values {
                    if !allowed.contains(value) {
                        self.error(
                            Some(rel),
                            format!("condition @{key}={value}: \"{value}\" is not an option of {key}"),
                        );
                    }
                }
                dimensions.insert(key.clone(), allowed);
            }
        }
        if dimensions.is_empty() {
            return;
        }
        let combinations: usize = dimensions.values().map(Vec::len).product();
        if combinations > 4096 {
            self.warn(None, format!("{combinations} option combinations: file collisions were not checked"));
            return;
        }
        let keys: Vec<&String> = dimensions.keys().collect();
        let mut reported = std::collections::BTreeSet::new();
        for index in 0..combinations {
            let mut rest = index;
            let mut selection = crate::manifest::Selection::default();
            for key in &keys {
                let values = &dimensions[*key];
                let value = values[rest % values.len()].clone();
                rest /= values.len();
                if m.features.contains_key(*key) {
                    selection.features.insert((*key).clone(), value == "on");
                } else {
                    selection.choices.insert((*key).clone(), value);
                }
            }
            let mut seen: BTreeMap<String, &String> = BTreeMap::new();
            for (rel, variant) in &variants {
                if !variant.applies(&selection) {
                    continue;
                }
                let output = crate::generator::output_path(&variant.dest);
                if let Some(other) = seen.insert(output.clone(), rel)
                    && reported.insert(output.clone())
                {
                    self.error(
                        Some(rel),
                        format!("\"{rel}\" and \"{other}\" both produce {output} for the same options; make their conditions exclusive"),
                    );
                }
            }
        }
    }

    /// Old key names that still parse (serde aliases) but should be renamed.
    fn check_deprecated(&mut self, text: &str) {
        let Ok(table) = text.parse::<toml::Table>() else { return };
        let choices = table.get("choices").and_then(|c| c.as_table()).into_iter().flatten();
        for (name, choice) in choices {
            let options = choice.get("options").and_then(|o| o.as_table()).into_iter().flatten();
            for (key, option) in options {
                if option.get("verify_cleanup").is_some() {
                    self.warn(
                        Some(MANIFEST_FILE),
                        format!("choices.{name}.options.{key}: rename verify_cleanup to cleanup"),
                    );
                }
            }
        }
    }

    /// Every user-facing manifest text should have a translation for the app's UI languages.
    fn check_translations(&mut self, m: &Manifest) {
        let file = Some(MANIFEST_FILE);
        let texts = m.texts();
        for lang in UI_LANGUAGES {
            let table = m.translations.get(*lang);
            let missing: Vec<&String> =
                texts.iter().filter(|t| !table.is_some_and(|map| map.contains_key(t.as_str()))).collect();
            // One warning per text, with the exact key to add.
            for text in missing {
                self.warn(file, format!("[translations.{lang}] lacks {}", toml_key(text)));
            }
            if let Some(map) = table {
                for key in map.keys().filter(|k| !texts.contains(k)) {
                    self.warn(file, format!("[translations.{lang}] \"{key}\" is not a text of this manifest"));
                }
            }
        }
        for lang in m.translations.keys().filter(|l| !UI_LANGUAGES.contains(&l.as_str())) {
            self.warn(file, format!("[translations.{lang}]: the app has no \"{lang}\" interface"));
        }
    }

    fn check_templates(&mut self, m: &Manifest, files_dir: &Path, files: &[String]) {
        let env = render::environment();
        let ctx = sample_context(m);
        let option_env = m.choices.values().flat_map(|c| c.options.values()).flat_map(|o| o.env.iter());
        for (key, value) in m.env.iter().chain(m.features.values().flat_map(|f| f.env.iter())).chain(option_env) {
            if let Err(err) = render::render_str(&env, &format!("env.{key}"), value, &ctx) {
                self.error(Some(MANIFEST_FILE), err.message);
            }
        }
        for edit in &m.set {
            if let Err(err) = render::render_str(&env, &format!("set {}", edit.file), &edit.value, &ctx) {
                self.error(Some(MANIFEST_FILE), err.message);
            }
        }
        for rel in files.iter().filter(|f| f.ends_with(TEMPLATE_SUFFIX)) {
            let source = match fs::read_to_string(files_dir.join(rel)) {
                Ok(source) => source,
                Err(_) => {
                    self.error(Some(rel), "template is not valid UTF-8");
                    continue;
                }
            };
            // Render with all features on and off, and with every option of every choice,
            // so all branches are checked.
            let mut contexts = Vec::new();
            for flag in [true, false] {
                let mut variant = ctx.clone();
                variant.features.values_mut().for_each(|v| *v = flag);
                contexts.push(variant);
            }
            for (name, choice) in &m.choices {
                for option in choice.options.keys().map(String::as_str).chain([""]) {
                    let mut variant = ctx.clone();
                    variant.choices.insert(name.clone(), option.to_string());
                    contexts.push(variant);
                }
            }
            for variant in &contexts {
                if let Err(err) = render::render_str(&env, rel, &source, variant) {
                    self.error(Some(rel), err.message);
                    break;
                }
            }
        }
    }
}

fn all_files(root: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.map_err(|err| Error::internal(err.to_string()))?;
        if entry.file_type().is_file() && entry.file_name() != ".DS_Store" {
            files.push(util::relative(root, entry.path()));
        }
    }
    files.sort();
    Ok(files)
}

/// Context with placeholder values, used to dry-run templates during validation.
pub fn sample_context(m: &Manifest) -> render::TemplateContext {
    // Base ports plus every option's ports, so templates of any selection can be rendered.
    let mut ports: indexmap::IndexMap<String, u16> = m.ports.clone();
    for option in m.choices.values().flat_map(|c| c.options.values()) {
        ports.extend(option.ports.clone());
    }
    let mut env: indexmap::IndexMap<String, String> = m.env.keys().map(|k| (k.clone(), "value".to_string())).collect();
    for (k, v) in &ports {
        env.insert(k.clone(), v.to_string());
    }
    for feature in m.features.values() {
        for k in feature.env.keys() {
            env.insert(k.clone(), "value".to_string());
        }
    }
    for option in m.choices.values().flat_map(|c| c.options.values()) {
        for k in option.env.keys() {
            env.insert(k.clone(), "value".to_string());
        }
    }
    for k in m.secrets.keys() {
        env.insert(k.clone(), String::new());
    }
    render::TemplateContext {
        project: render::ProjectContext {
            name: "Sample Project".into(),
            slug: "sample-project".into(),
            brief: "Sample brief.".into(),
            created_at: util::today(),
        },
        skeleton: render::SkeletonContext { id: m.id.clone(), name: m.name.clone(), version: "0.0.0".into() },
        features: m.features.keys().map(|k| (k.clone(), true)).collect(),
        choices: m.resolve_choices(&std::collections::BTreeMap::new()).unwrap_or_default(),
        options: render::OptionsContext { git: true, install: true, agents_md: true, spec_md: true, claude_md: true },
        env,
        ports,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIGURABLE: &str = r#"
schema = 1
id = "demo"
name = "Demo"
description = "A demo."

[choices.frontend]
label = "Frontend"
default = "vue"

[choices.frontend.options.vue]
label = "Vue"

[choices.frontend.options.react]
label = "React"

[translations.ru]
"Demo" = "Демо"
"Frontend" = "Фронтенд"
"Old text" = "Старый текст"
"#;

    fn messages(issues: &[Issue]) -> String {
        issues.iter().map(|i| i.message.clone()).collect::<Vec<_>>().join("\n")
    }

    #[test]
    fn checks_file_variants() {
        let m = Manifest::parse(CONFIGURABLE).unwrap();
        let files = |list: &[&str]| list.iter().map(ToString::to_string).collect::<Vec<_>>();

        let mut v = Validator::default();
        v.check_variants(&m, &files(&["app@frontend=vue.ts", "app@frontend=react.ts", "shared.ts"]));
        assert!(v.errors.is_empty(), "{}", messages(&v.errors));

        let mut v = Validator::default();
        v.check_variants(&m, &files(&["web@frontend=vue|react/main.ts", "web/main.ts@frontend=react"]));
        assert!(messages(&v.errors).contains("both produce web/main.ts"), "{}", messages(&v.errors));

        let mut v = Validator::default();
        v.check_variants(&m, &files(&["x.ts@frontend=svelte", "y.ts@colour=red"]));
        let text = messages(&v.errors);
        assert!(text.contains("\"svelte\" is not an option") && text.contains("\"colour\" is not a choice"), "{text}");
    }

    #[test]
    fn warns_about_missing_and_stale_translations() {
        let m = Manifest::parse(CONFIGURABLE).unwrap();
        let mut v = Validator::default();
        v.check_translations(&m);
        let text = messages(&v.warnings);
        assert!(
            text.contains("lacks \"A demo.\"") && text.contains("lacks Vue") && text.contains("lacks React"),
            "{text}"
        );
        assert!(text.contains("\"Old text\" is not a text"), "{text}");
    }

    #[test]
    fn warns_about_unverified_options() {
        let m = Manifest::parse(CONFIGURABLE).unwrap();
        let mut v = Validator::default();
        v.check_choices(&m);
        assert!(messages(&v.warnings).contains("choices.frontend.options.react is never verified"));

        let m =
            Manifest::parse(&format!("{CONFIGURABLE}\n[verify]\nvariants = [{{ frontend = \"react\" }}]\n")).unwrap();
        let mut v = Validator::default();
        v.check_choices(&m);
        assert!(!messages(&v.warnings).contains("never verified"), "{}", messages(&v.warnings));
    }

    #[test]
    fn warns_about_deprecated_keys() {
        let mut v = Validator::default();
        v.check_deprecated("[choices.docker.options.full]\nverify_cleanup = \"docker compose down\"\n");
        assert!(messages(&v.warnings).contains("rename verify_cleanup to cleanup"));
    }

    #[test]
    fn parses_changelog() {
        let entries = parse_changelog(
            "# Changelog\n\n## 1.1.0 — 2026-10-01\n\n- Updated Vue\n\n## 1.0.0 — 2026-09-30\n\n- Initial\n",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, "1.1.0");
        assert_eq!(entries[0].date, "2026-10-01");
        assert_eq!(entries[1].body, "- Initial");
    }
}
