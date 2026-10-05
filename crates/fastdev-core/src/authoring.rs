//! Skeleton lifecycle: drafts (new / edit / fork), verification and publishing (SPEC §5.7–5.8).

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::{Error, IoContext, Result};
use crate::generator::{self, CreateOptions, Source};
use crate::index::{self, IndexEntry};
use crate::jobs::JobCtx;
use crate::library::{
    AGENTS_TEMPLATE, CHANGELOG_FILE, DRAFT_META, DraftInfo, DraftKind, DraftMeta, FILES_DIR, GITIGNORE_SOURCE, Library,
    SPEC_TEMPLATE, Target, Verification, add_changelog_entry, remove_changelog_entry,
};
use crate::manifest::{MANIFEST_FILE, Manifest, Resolved, Selection, SetupStep};
use crate::{git, shell, toolchain, util};

#[derive(Debug, Clone, Deserialize)]
pub struct DraftRequest {
    pub mode: DraftKind,
    pub id: String,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub from_version: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

pub fn create_draft(lib: &Library, req: &DraftRequest) -> Result<DraftInfo> {
    if !util::is_valid_id(&req.id) {
        return Err(Error::invalid(format!(
            "\"{}\" is not a valid id: use lowercase letters, digits and single dashes, starting with a letter",
            req.id
        )));
    }
    let draft = lib.draft_dir(&req.id);
    if lib.has_draft(&req.id) {
        return Err(Error::conflict(format!(
            "skeleton \"{}\" already has a draft at {}; continue editing it or discard it first",
            req.id,
            draft.display()
        )));
    }
    fs::create_dir_all(lib.workspace()).at(lib.workspace())?;
    let meta = match req.mode {
        DraftKind::New => {
            if lib.exists(&req.id) || draft.exists() {
                return Err(Error::conflict(format!("skeleton \"{}\" exists; use mode \"edit\" to change it", req.id)));
            }
            let name = req.name.as_deref().map(str::trim).filter(|n| !n.is_empty()).unwrap_or(&req.id);
            fs::create_dir_all(&draft).at(&draft)?;
            git::run(&draft, &["init", "--quiet", "-b", "main"])?;
            write_scaffold(&draft, &req.id, name, req.description.as_deref().unwrap_or_default())?;
            write_repo_files(&draft, &req.id, name, req.description.as_deref().unwrap_or_default())?;
            DraftMeta {
                kind: DraftKind::New,
                based_on: None,
                source: None,
                created_at: util::now_rfc3339(),
                verification: None,
            }
        }
        DraftKind::Edit => {
            if !draft.exists() {
                let url = lib.repo_url(&req.id).ok_or_else(|| {
                    Error::not_found(format!("skeleton \"{}\" is not in the library; try sync_library", req.id))
                })?;
                git::clone(&url, &draft)?;
            } else if git::remote_url(&draft, "origin").is_some() {
                // A clean clone from an earlier draft: bring it up to date.
                git::run(&draft, &["pull", "--ff-only", "--quiet", "--tags"])?;
            }
            let latest = git::version_tags(&draft).into_iter().next().map(|(v, _)| v);
            let Some(latest) = latest else {
                return Err(Error::invalid(format!(
                    "skeleton \"{}\" has no published version; use mode \"new\"",
                    req.id
                )));
            };
            if let Some(from) = req.from_version.as_deref().map(str::trim).filter(|v| !v.is_empty() && *v != "latest")
                && semver::Version::parse(from.trim_start_matches(git::TAG_PREFIX)).ok().as_ref() != Some(&latest)
            {
                return Err(Error::invalid(format!(
                    "drafts start from the latest version ({latest}); fork an older version instead"
                )));
            }
            DraftMeta {
                kind: DraftKind::Edit,
                based_on: Some(latest.to_string()),
                source: None,
                created_at: util::now_rfc3339(),
                verification: None,
            }
        }
        DraftKind::Fork => {
            let source = req
                .source
                .as_deref()
                .ok_or_else(|| Error::invalid("mode \"fork\" needs \"source\": the id of the skeleton to fork"))?;
            if lib.exists(&req.id) || draft.exists() {
                return Err(Error::conflict(format!(
                    "skeleton \"{}\" already exists; choose another id for the fork",
                    req.id
                )));
            }
            let target = lib.resolve_published(source, req.from_version.as_deref())?;
            let source_manifest = lib.load_manifest(source, &target)?;
            let url = lib
                .repo_url(source)
                .ok_or_else(|| Error::not_found(format!("skeleton \"{source}\" has no repository")))?;
            // A fork keeps the history up to the forked version, but not the parent's tags:
            // its own versions start again at 1.0.0.
            git::clone(&url, &draft)?;
            git::run(&draft, &["reset", "--hard", "--quiet", &format!("{}{}", git::TAG_PREFIX, target.label())])?;
            for (version, _) in git::version_tags(&draft) {
                git::run(&draft, &["tag", "-d", &format!("{}{version}", git::TAG_PREFIX)])?;
            }
            git::run(&draft, &["remote", "rename", "origin", "upstream"])?;
            let name = req
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| format!("{} (fork)", source_manifest.name));
            let description = req
                .description
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
                .unwrap_or(&source_manifest.description)
                .to_string();
            rewrite_fork_manifest(&draft, &req.id, &name, Some(&description), source, &target.label())?;
            util::write_atomic(&draft.join(CHANGELOG_FILE), b"# Changelog\n")?;
            write_repo_files(&draft, &req.id, &name, &description)?;
            DraftMeta {
                kind: DraftKind::Fork,
                based_on: Some(target.label()),
                source: Some(source.to_string()),
                created_at: util::now_rfc3339(),
                verification: None,
            }
        }
    };
    meta.save(&draft)?;
    lib.draft_info(&req.id).ok_or_else(|| Error::internal("draft was not created"))
}

/// Files at the root of a skeleton repository (outside `files/`): README for people browsing
/// the repository, AGENTS.md for agents working on it, and .gitignore.
fn write_repo_files(dir: &Path, id: &str, name: &str, description: &str) -> Result<()> {
    let fill =
        |text: &str| text.replace("__ID__", id).replace("__NAME__", name).replace("__DESCRIPTION__", description);
    util::write_atomic(&dir.join("README.md"), fill(REPO_README).as_bytes())?;
    util::write_atomic(&dir.join("AGENTS.md"), fill(REPO_AGENTS).as_bytes())?;
    util::write_atomic(&dir.join(".gitignore"), REPO_GITIGNORE.as_bytes())?;
    if !dir.join(CHANGELOG_FILE).exists() {
        util::write_atomic(&dir.join(CHANGELOG_FILE), b"# Changelog\n")?;
    }
    Ok(())
}

pub const REPO_GITIGNORE: &str =
    "# fastDev draft state and local artefacts\n.fastdev-draft.toml\nnode_modules/\n.DS_Store\n";

pub const REPO_README: &str = "# __NAME__

__DESCRIPTION__

This repository is a [fastDev](https://github.com/roma-vibe/fastDev) project skeleton (`__ID__`). fastDev lists it
from a registry, downloads it when first used and creates named, ready-to-run projects from it.

- `template.toml` — the manifest: requirements, options, env, setup steps and commands.
- `files/` — the files of a new project (`*.tmpl` files are rendered with the project name and options).
- `CHANGELOG.md` — what changed in every version.

Versions are the `vX.Y.Z` tags of this repository and never change once published.
";

pub const REPO_AGENTS: &str = "# __NAME__ skeleton — instructions for AI agents

This repository is a fastDev skeleton (`__ID__`). It is not a project: fastDev copies `files/` into
new projects and renders `*.tmpl` files. Read the fastDev skeleton authoring guide first
(MCP tool `get_authoring_guide`, or `docs/skeleton-authoring.md` in the fastDev repository).

- Change `template.toml` and `files/` only; `files/AGENTS.md.tmpl` and `files/SPEC.md.tmpl` are the
  instructions of the future projects, not of this repository.
- Do not commit, tag or edit `CHANGELOG.md` by hand: validate, verify and publish through fastDev
  (`validate_skeleton`, `verify_skeleton`, `publish_skeleton`). Publishing commits, creates the
  `vX.Y.Z` tag, pushes and updates the registry.
- Never move or delete a published tag.
- Everything is written in English.
";

fn rewrite_fork_manifest(
    draft: &Path,
    id: &str,
    name: &str,
    description: Option<&str>,
    source: &str,
    version: &str,
) -> Result<()> {
    let path = draft.join(MANIFEST_FILE);
    let text = fs::read_to_string(&path).at(&path)?;
    let mut doc: toml_edit::DocumentMut =
        text.parse().map_err(|err: toml_edit::TomlError| Error::validation(err.to_string()))?;
    doc["id"] = toml_edit::value(id);
    doc["name"] = toml_edit::value(name);
    if let Some(description) = description.map(str::trim).filter(|d| !d.is_empty()) {
        doc["description"] = toml_edit::value(description);
    }
    let mut origin = toml_edit::InlineTable::new();
    origin.insert("id", source.into());
    origin.insert("version", version.into());
    doc["forked_from"] = toml_edit::value(origin);
    util::write_atomic(&path, doc.to_string().as_bytes())
}

fn write_scaffold(draft: &Path, id: &str, name: &str, description: &str) -> Result<()> {
    let files = draft.join(FILES_DIR);
    fs::create_dir_all(&files).at(&files)?;
    let manifest = SCAFFOLD_MANIFEST
        .replace("__ID__", id)
        .replace("__NAME__", &toml_escape(name))
        .replace("__DESCRIPTION__", &toml_escape(description));
    util::write_atomic(&draft.join(MANIFEST_FILE), manifest.as_bytes())?;
    util::write_atomic(&files.join(AGENTS_TEMPLATE), SCAFFOLD_AGENTS.as_bytes())?;
    util::write_atomic(&files.join(SPEC_TEMPLATE), SCAFFOLD_SPEC.as_bytes())?;
    util::write_atomic(&files.join("README.md.tmpl"), SCAFFOLD_README.as_bytes())?;
    util::write_atomic(&files.join(".env.example"), b"APP_NAME=App\nAPP_SLUG=app\n")?;
    util::write_atomic(&files.join(GITIGNORE_SOURCE), b".env\n.DS_Store\n")?;
    Ok(())
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

const SCAFFOLD_MANIFEST: &str = r#"# Manifest reference: docs/manifest.md
schema = 1
id = "__ID__"
name = "__NAME__"
description = "__DESCRIPTION__"
category = "other"          # web | desktop | cli | library | mobile | other
languages = []              # e.g. ["node"], ["rust"], ["php"], ["python"]
tags = []
stack = []

[requirements]              # tool = "semver requirement", e.g. node = ">=24"

[env]
APP_NAME = "{{ project.name }}"
APP_SLUG = "{{ project.slug }}"

# [ports]
# APP_PORT = 3000

# [[set]]
# file = "package.json"
# path = ["name"]
# value = "{{ project.slug }}"

# [[setup]]
# label = "Install dependencies"
# run = "npm install"

# [commands.dev]
# label = "Dev"
# run = "npm run dev"
# long = true
# url = "http://localhost:${APP_PORT}"
# primary = true

[verify]
commands = []
"#;

const SCAFFOLD_AGENTS: &str = r"# {{ project.name }} — instructions for AI agents

Read SPEC.md first. Describe here how to work with THIS project: map of folders,
architecture rules, commands, configuration (.env), testing and conventions.
";

const SCAFFOLD_SPEC: &str = r"# {{ project.name }} — Specification

## Initial brief

{{ project.brief if project.brief else '_No brief was given. Ask the owner what to build._' }}

## Goals

## Scope

## Open questions
";

const SCAFFOLD_README: &str = r"# {{ project.name }}
";

/// Throws away the draft: an unpublished skeleton is deleted from the workspace, a published one
/// is reset to its latest release.
pub fn discard_draft(lib: &Library, id: &str) -> Result<()> {
    if !util::is_valid_id(id) {
        return Err(Error::invalid(format!("\"{id}\" is not a valid skeleton id")));
    }
    let draft = lib.draft_dir(id);
    if !lib.has_draft(id) {
        return Err(Error::not_found(format!("skeleton \"{id}\" has no draft")));
    }
    // Only a folder directly inside the workspace is ever deleted or reset.
    let workspace = fs::canonicalize(lib.workspace()).at(lib.workspace())?;
    if fs::canonicalize(&draft).at(&draft)?.parent() != Some(workspace.as_path()) {
        return Err(Error::not_allowed(format!("{} is not inside the workspace", draft.display())));
    }
    match git::version_tags(&draft).into_iter().next() {
        None => fs::remove_dir_all(&draft).at(&draft)?,
        Some((latest, _)) => {
            git::run(&draft, &["reset", "--hard", "--quiet", &format!("{}{latest}", git::TAG_PREFIX)])?;
            git::run(&draft, &["clean", "-fdq"])?;
            let meta = draft.join(DRAFT_META);
            if meta.exists() {
                fs::remove_file(&meta).at(&meta)?;
            }
        }
    }
    Ok(())
}

/// Hash of the draft content (everything except `.git` and the draft state).
pub fn draft_hash(lib: &Library, id: &str) -> Result<String> {
    util::hash_tree(&lib.draft_dir(id), &[DRAFT_META])
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Bump {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PublishRequest {
    pub id: String,
    #[serde(default)]
    pub bump: Option<Bump>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub changes: Vec<String>,
    #[serde(default)]
    pub allow_unverified: bool,
    /// Development only (CLI): re-publish the latest version in place while its tag has not been
    /// pushed anywhere. Never available through MCP.
    #[serde(default)]
    pub replace: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishResult {
    pub id: String,
    pub version: String,
    pub tag: String,
    pub commit: String,
    /// Workspace clone that was tagged.
    pub path: String,
    pub verified: bool,
    pub pushed: bool,
    /// Registry whose `index.json` was updated.
    pub registry: Option<String>,
    pub warnings: Vec<String>,
}

pub fn next_version(
    existing: &[semver::Version],
    bump: Option<Bump>,
    explicit: Option<&str>,
) -> Result<semver::Version> {
    let latest = existing.first();
    let version = match (explicit.map(str::trim).filter(|v| !v.is_empty()), bump, latest) {
        (Some(v), _, _) => {
            semver::Version::parse(v).map_err(|_| Error::invalid(format!("\"{v}\" is not a semver version")))?
        }
        (None, _, None) => semver::Version::new(1, 0, 0),
        (None, Some(bump), Some(latest)) => match bump {
            Bump::Patch => semver::Version::new(latest.major, latest.minor, latest.patch + 1),
            Bump::Minor => semver::Version::new(latest.major, latest.minor + 1, 0),
            Bump::Major => semver::Version::new(latest.major + 1, 0, 0),
        },
        (None, None, Some(_)) => {
            return Err(Error::invalid("give \"bump\" (patch, minor, major) or an explicit \"version\""));
        }
    };
    if let Some(latest) = latest
        && &version <= latest
    {
        return Err(Error::invalid(format!("version {version} must be greater than the latest version {latest}")));
    }
    Ok(version)
}

/// Publishes the draft: changelog entry, commit, `vX.Y.Z` tag, push (when `push` and the
/// repository has an `origin`), registry update, and download of the new version.
pub fn publish(lib: &Library, req: &PublishRequest, push: bool) -> Result<PublishResult> {
    let id = req.id.as_str();
    if !lib.has_draft(id) {
        return Err(Error::not_found(format!("skeleton \"{id}\" has no draft to publish")));
    }
    let changes: Vec<String> = req.changes.iter().map(|c| c.trim().to_string()).filter(|c| !c.is_empty()).collect();
    if changes.is_empty() {
        return Err(Error::invalid("\"changes\" must list at least one changelog entry"));
    }
    let report = lib.validate(id, &Target::Draft);
    if !report.valid {
        let list: Vec<String> = report
            .errors
            .iter()
            .map(|e| format!("{}{}", e.file.as_deref().map(|f| format!("{f}: ")).unwrap_or_default(), e.message))
            .collect();
        return Err(Error::validation(format!("the draft is not valid:\n- {}", list.join("\n- "))));
    }
    let draft = lib.draft_dir(id);
    let meta = DraftMeta::load(&draft);
    let hash = draft_hash(lib, id)?;
    let verified = meta.as_ref().and_then(|m| m.verification.as_ref()).is_some_and(|v| v.passed && v.hash == hash);
    if !verified && !req.allow_unverified {
        return Err(Error::validation(
            "the current draft has no passing verification; run verify_skeleton first (or set allow_unverified when the toolchain is not installed locally)",
        ));
    }
    let mut warnings = Vec::new();
    let has_origin = git::remote_url(&draft, "origin").is_some();
    if has_origin && let Err(err) = git::run(&draft, &["fetch", "--quiet", "--tags", "origin"]) {
        warnings.push(format!("could not fetch tags from origin: {}", err.message));
    }
    let mut existing: Vec<semver::Version> = git::version_tags(&draft).into_iter().map(|(v, _)| v).collect();
    let replaced = if req.replace {
        let latest =
            existing.first().cloned().ok_or_else(|| Error::invalid("nothing to replace: no published version"))?;
        if let Some(v) = req.version.as_deref().filter(|v| !v.trim().is_empty())
            && semver::Version::parse(v.trim()).ok() != Some(latest.clone())
        {
            return Err(Error::invalid(format!("only the latest version ({latest}) can be replaced")));
        }
        let tag = format!("{}{latest}", git::TAG_PREFIX);
        if has_origin && !git::run(&draft, &["ls-remote", "--tags", "origin", &format!("refs/tags/{tag}")])?.is_empty()
        {
            return Err(Error::not_allowed(format!("{tag} is already pushed; publish a new version instead")));
        }
        remove_changelog_entry(&draft, &latest.to_string())?;
        git::run(&draft, &["tag", "-d", &tag])?;
        existing.remove(0);
        Some(latest)
    } else {
        None
    };
    let version = match &replaced {
        Some(v) => v.clone(),
        None => next_version(&existing, req.bump, req.version.as_deref())?,
    };
    let tag = format!("{}{version}", git::TAG_PREFIX);

    let manifest = Manifest::load(&draft)?;
    let mut notes = Vec::new();
    if let (Some(origin), true) = (&manifest.forked_from, existing.is_empty()) {
        notes.push(format!("Forked from {} {}.", origin.id, origin.version));
    }
    if !verified {
        notes.push("Published without a passing verification.".to_string());
        warnings.push("published without a passing verification".to_string());
    } else if let Some(skipped) = meta.as_ref().and_then(|m| m.verification.as_ref()).map(|v| &v.skipped)
        && !skipped.is_empty()
    {
        notes.push(format!("Not verified on the publishing machine: {}.", skipped.join("; ")));
        warnings.push(format!("some selections were not verified: {}", skipped.join("; ")));
    }
    let note = (!notes.is_empty()).then(|| notes.join(" "));
    add_changelog_entry(&draft, &version.to_string(), &changes, note.as_deref())?;
    if !draft.join(".gitignore").exists() {
        util::write_atomic(&draft.join(".gitignore"), REPO_GITIGNORE.as_bytes())?;
    }

    git::run(&draft, &["add", "-A"])?;
    if !git::is_clean(&draft) {
        git::run(&draft, &["commit", "--quiet", "-m", &format!("Release {version}")])?;
    }
    git::run(&draft, &["tag", "-a", &tag, "-m", &format!("{} {version}", manifest.name)])?;
    let commit = git::head(&draft).unwrap_or_default();
    let meta_file = draft.join(DRAFT_META);
    if meta_file.exists() {
        fs::remove_file(&meta_file).at(&meta_file)?;
    }

    let mut pushed = false;
    if push && has_origin {
        match git::run(&draft, &["push", "--quiet", "--follow-tags", "origin", "HEAD"]) {
            Ok(_) => pushed = true,
            Err(err) => {
                warnings.push(format!("push failed: {}; push the repository and the tag {tag} by hand", err.message))
            }
        }
    }

    // The registry points to origin when there is one, otherwise to the local repository.
    let repo = git::remote_url(&draft, "origin").unwrap_or_else(|| draft.to_string_lossy().into_owned());
    let registry = match lib.writable_registry() {
        Some(source) => {
            let entry = IndexEntry::from_manifest(&manifest, &repo, &version.to_string());
            match index::upsert(&source, entry, push, &mut warnings) {
                Ok(_) => Some(source.url),
                Err(err) => {
                    warnings.push(format!("registry not updated: {}", err.message));
                    None
                }
            }
        }
        None => {
            warnings.push("no local registry is configured; add the repository to a registry index.json so it appears in other libraries".into());
            None
        }
    };
    if replaced.is_some() {
        lib.forget_version(id, &version)?;
    }
    if let Err(err) = lib.refresh(id) {
        warnings.push(format!("could not download the new version: {}", err.message));
    }

    Ok(PublishResult {
        id: id.to_string(),
        version: version.to_string(),
        tag,
        commit,
        path: draft.to_string_lossy().into_owned(),
        verified,
        pushed,
        registry,
        warnings,
    })
}

/// Creates a throwaway project from `target` (all features on) and runs setup + verify commands.
pub fn verify(lib: &Library, id: &str, target: &Target, taken_ports: &HashSet<u16>, ctx: &JobCtx) -> Result<Value> {
    ctx.step("Validate");
    let report = lib.validate(id, target);
    for warning in &report.warnings {
        ctx.log(format!(
            "warning: {}{}",
            warning.file.as_deref().map(|f| format!("{f}: ")).unwrap_or_default(),
            warning.message
        ));
    }
    if !report.valid {
        for error in &report.errors {
            ctx.log(format!(
                "error: {}{}",
                error.file.as_deref().map(|f| format!("{f}: ")).unwrap_or_default(),
                error.message
            ));
        }
        return Err(Error::validation(format!("validation failed with {} error(s)", report.errors.len())));
    }
    // A draft is verified from a snapshot, so edits made while it runs cannot mix into the result.
    let (_snapshot, dir, hash) = match target {
        Target::Draft => {
            let tmp = tempfile::Builder::new().prefix("fastdev-snapshot-").tempdir()?;
            let dir = tmp.path().join(id);
            util::copy_dir(&lib.draft_dir(id), &dir)?;
            let hash = util::hash_tree(&dir, &[DRAFT_META])?;
            (Some(tmp), dir, Some(hash))
        }
        Target::Version(_) => (None, lib.target_dir(id, target)?, None),
    };
    let outcome = run_verification(&dir, &target.label(), taken_ports, ctx);
    if let Some(hash) = &hash {
        let draft = lib.draft_dir(id);
        // Drafts written by hand may lack .draft.toml; create it so the result is kept.
        let mut meta = DraftMeta::load(&draft).unwrap_or_else(|| DraftMeta {
            kind: if git::version_tags(&draft).is_empty() { DraftKind::New } else { DraftKind::Edit },
            based_on: git::version_tags(&draft).first().map(|(v, _)| v.to_string()),
            source: None,
            created_at: util::now_rfc3339(),
            verification: None,
        });
        meta.verification = Some(Verification {
            hash: hash.clone(),
            passed: outcome.is_ok(),
            at: util::now_rfc3339(),
            job_id: ctx.id().to_string(),
            skipped: outcome.clone().unwrap_or_default(),
        });
        meta.save(&draft)?;
    }
    outcome
        .map(|skipped| json!({ "id": id, "target": target.label(), "passed": true, "hash": hash, "skipped": skipped }))
}

/// Verifies the default selection and every `[verify] variants` entry. A selection whose tools
/// are missing on this machine is skipped with a warning (returned), as long as one was verified.
fn run_verification(dir: &Path, version: &str, taken_ports: &HashSet<u16>, ctx: &JobCtx) -> Result<Vec<String>> {
    let manifest = Manifest::load(dir)?;
    let all_features = manifest.default_selection(true).features;
    // The default choices first, then every declared variant.
    let mut variants: Vec<BTreeMap<String, String>> = vec![BTreeMap::new()];
    variants.extend(manifest.verify.variants.iter().cloned());
    let mut skipped = Vec::new();
    let mut verified = 0;
    for variant in &variants {
        let selection = manifest.select(&all_features, variant)?;
        let missing = missing_tools(&manifest, &manifest.resolve(&selection));
        if !missing.is_empty() {
            let label = describe_choices(&selection.choices);
            let what = if label.is_empty() { "defaults".to_string() } else { label };
            ctx.warn(format!("Not verified here ({what}): {} not installed", missing.join(", ")));
            skipped.push(format!("{what}: {}", missing.join(", ")));
            continue;
        }
        verify_selection(&manifest, dir, version, &selection, taken_ports, ctx)?;
        verified += 1;
    }
    if verified == 0 {
        return Err(Error::new(
            crate::ErrorCode::Toolchain,
            format!("nothing could be verified on this machine: {}", skipped.join("; ")),
        ));
    }
    Ok(skipped)
}

/// Tools that setup or the verify commands of a selection need and that are missing or too old.
fn missing_tools(manifest: &Manifest, resolved: &Resolved) -> Vec<String> {
    let verify_scripts: Vec<&str> =
        manifest.verify.commands.iter().filter_map(|k| resolved.commands.get(k)).map(|c| c.run.as_str()).collect();
    toolchain::check_requirements(&resolved.requirements)
        .into_iter()
        .filter(|s| !s.satisfied && resolved.needs_tool(manifest, &s.tool, &verify_scripts))
        .map(|s| format!("{} {}", s.label, s.requirement))
        .collect()
}

/// "docker=full, data=local" for step labels; empty when the manifest has no choices.
fn describe_choices(choices: &BTreeMap<String, String>) -> String {
    choices.iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")
}

fn verify_selection(
    manifest: &Manifest,
    dir: &Path,
    version: &str,
    selection: &Selection,
    taken_ports: &HashSet<u16>,
    ctx: &JobCtx,
) -> Result<()> {
    let resolved = manifest.resolve(selection);
    let label = describe_choices(&selection.choices);
    let suffix = if label.is_empty() { String::new() } else { format!(" ({label})") };

    ctx.step(&format!("Check requirements{suffix}"));
    // Tools used by setup or the verify commands must be installed; others are warnings.
    let verify_scripts: Vec<&str> =
        manifest.verify.commands.iter().filter_map(|k| resolved.commands.get(k)).map(|c| c.run.as_str()).collect();
    check_tools(manifest, &resolved, &verify_scripts, true, "", ctx)?;

    ctx.step(&format!("Create test project{suffix}"));
    let tmp = tempfile::Builder::new().prefix("fastdev-verify-").tempdir()?;
    let project = tmp.path().join(format!("verify-{}", manifest.id));
    // Unique per run: the slug names Docker Compose projects, images and volumes, and parallel
    // verifications (or a cleanup) must not touch each other's.
    let slug = format!("verify-{}-{}", manifest.id, &util::short_id()[..6]);
    let options = CreateOptions {
        name: format!("Verify {}", manifest.name),
        slug: Some(slug),
        brief: "Verification build.".into(),
        git: false,
        install: true,
        agents_md: true,
        spec_md: true,
        claude_md: true,
        features: selection.features.clone(),
        choices: selection.choices.clone(),
    };
    let source = Source { dir, manifest, version, repo: None, commit: None };
    generator::materialize(&source, &options, &project, taken_ports, &mut |line| ctx.log(line))?;
    ctx.log(format!("Test project: {}", project.display()));

    let outcome = (|| -> Result<()> {
        run_steps(&resolved.setup, &project, ctx)?;
        for key in &manifest.verify.commands {
            // Commands added by one option (e.g. a Docker image build) are skipped in the others.
            let Some(command) = resolved.commands.get(key) else {
                ctx.log(format!("Skipped {key}: not available{suffix}"));
                continue;
            };
            let label = if command.label.is_empty() { key.clone() } else { command.label.clone() };
            ctx.step(&format!("Run {label}{suffix}"));
            ctx.log(format!("$ {}", command.run));
            shell::run_checked(&command.run, &project, &mut |line| ctx.log(line))?;
        }
        Ok(())
    })();

    for script in &resolved.cleanup {
        ctx.log(format!("$ {script}"));
        if let Err(err) = shell::run_cleanup(script, &project, &mut |line| ctx.log(line)) {
            ctx.warn(format!("cleanup failed: {}", err.message));
        }
    }
    outcome
}

/// Logs the toolchain check of a selection. A missing tool that setup (or `extra` commands) needs
/// fails when `blocking`; every other missing tool becomes a warning with an install hint.
pub fn check_tools(
    manifest: &Manifest,
    resolved: &Resolved,
    extra: &[&str],
    blocking: bool,
    advice: &str,
    ctx: &JobCtx,
) -> Result<()> {
    let mut missing = Vec::new();
    for status in toolchain::check_requirements(&resolved.requirements) {
        if status.satisfied {
            ctx.log(format!(
                "✓ {} {} ({})",
                status.label,
                status.requirement,
                status.version.as_deref().unwrap_or("found")
            ));
        } else if blocking && resolved.needs_tool(manifest, &status.tool, extra) {
            missing.push(status.describe());
        } else {
            ctx.warn(format!("Needed later: {}", status.describe()));
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(Error::new(crate::ErrorCode::Toolchain, format!("Missing: {}{advice}", missing.join(" "))))
    }
}

/// Runs setup steps in a project folder.
pub fn run_steps(steps: &[SetupStep], project: &Path, ctx: &JobCtx) -> Result<()> {
    for step in steps {
        ctx.step(&step.label);
        ctx.log(format!("$ {}", step.run));
        shell::run_checked(&step.run, project, &mut |line| ctx.log(line))?;
    }
    Ok(())
}

/// Temp copy of a target's `files/` folder, used by dependency tools.
pub fn temp_copy(lib: &Library, id: &str, target: &Target) -> Result<(tempfile::TempDir, PathBuf)> {
    let tmp = tempfile::Builder::new().prefix("fastdev-updates-").tempdir()?;
    let dir = tmp.path().join(id);
    util::copy_dir(&lib.target_dir(id, target)?.join(FILES_DIR), &dir)?;
    Ok((tmp, dir))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> semver::Version {
        semver::Version::parse(s).unwrap()
    }

    #[test]
    fn computes_next_versions() {
        assert_eq!(next_version(&[], None, None).unwrap(), v("1.0.0"));
        assert_eq!(next_version(&[v("1.2.3")], Some(Bump::Patch), None).unwrap(), v("1.2.4"));
        assert_eq!(next_version(&[v("1.2.3")], Some(Bump::Minor), None).unwrap(), v("1.3.0"));
        assert_eq!(next_version(&[v("1.2.3")], Some(Bump::Major), None).unwrap(), v("2.0.0"));
        assert!(next_version(&[v("1.2.3")], None, None).is_err());
        assert!(next_version(&[v("1.2.3")], None, Some("1.0.0")).is_err());
        assert_eq!(next_version(&[v("1.2.3")], None, Some("1.5.0")).unwrap(), v("1.5.0"));
    }

    fn identity(dir: &Path) {
        git::run(dir, &["config", "user.name", "fastDev tests"]).unwrap();
        git::run(dir, &["config", "user.email", "tests@fastdev.local"]).unwrap();
    }

    #[test]
    fn new_edit_fork_publish_flow() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = tmp.path().join("registry");
        git::init_repo(&registry);
        index::init(&registry, "Test").unwrap();
        let workspace = tmp.path().join("workspace");
        let lib =
            Library::new(tmp.path().join("data"), &workspace, vec![registry.to_string_lossy().into_owned()], vec![]);
        let req = |mode, id: &str| DraftRequest {
            mode,
            id: id.into(),
            source: None,
            from_version: None,
            name: Some("Demo".into()),
            description: Some("A demo".into()),
        };

        // New skeleton: a fresh repository in the workspace.
        let draft = create_draft(&lib, &req(DraftKind::New, "demo")).unwrap();
        assert_eq!(draft.kind, DraftKind::New);
        let repo = workspace.join("demo");
        identity(&repo);
        assert!(git::is_work_tree(&repo) && repo.join("AGENTS.md").is_file());
        assert!(lib.validate("demo", &Target::Draft).valid, "{:?}", lib.validate("demo", &Target::Draft).errors);

        // Unverified publish is refused, allowed explicitly.
        let mut publish_req = PublishRequest {
            id: "demo".into(),
            bump: None,
            version: None,
            changes: vec!["Initial version".into()],
            allow_unverified: false,
            replace: false,
        };
        assert!(publish(&lib, &publish_req, false).is_err());
        publish_req.allow_unverified = true;
        let published = publish(&lib, &publish_req, false).unwrap();
        assert_eq!((published.version.as_str(), published.tag.as_str()), ("1.0.0", "v1.0.0"));
        assert!(published.registry.is_some());
        assert!(!lib.has_draft("demo"));
        assert!(git::is_clean(&repo));
        assert!(git::is_clean(&registry));
        // The registry lists it, the mirror has the version, the export validates.
        let entry = index::load(&index::source(&registry.to_string_lossy(), tmp.path())).unwrap().skeletons;
        assert_eq!(entry[0].latest.as_deref(), Some("1.0.0"));
        assert!(lib.is_downloaded("demo"));
        assert_eq!(lib.versions("demo").unwrap().len(), 1);
        let v1 = Target::Version(semver::Version::new(1, 0, 0));
        assert!(lib.validate("demo", &v1).valid);
        assert_eq!(lib.changelog("demo")[0].version, "1.0.0");

        // Edit → 1.1.0; uncommitted edits make a draft even without create_draft.
        fs::write(repo.join("files/README.md.tmpl"), "# {{ project.name }}\n\nChanged.\n").unwrap();
        assert!(lib.has_draft("demo"));
        assert!(create_draft(&lib, &req(DraftKind::Edit, "demo")).is_err(), "a draft already exists");
        let publish_req = PublishRequest {
            id: "demo".into(),
            bump: Some(Bump::Minor),
            version: None,
            changes: vec!["Better docs".into()],
            allow_unverified: true,
            replace: false,
        };
        assert_eq!(publish(&lib, &publish_req, false).unwrap().version, "1.1.0");
        assert_eq!(lib.versions("demo").unwrap().len(), 2);
        // The old version is still exported unchanged.
        let old = lib.target_dir("demo", &v1).unwrap();
        assert!(!fs::read_to_string(old.join("files/README.md.tmpl")).unwrap().contains("Changed"));

        // Fork of 1.0.0: own id, no parent tags, versions start at 1.0.0.
        let mut fork = req(DraftKind::Fork, "demo-plus");
        fork.source = Some("demo".into());
        fork.from_version = Some("1.0.0".into());
        fork.name = Some("Demo Plus".into());
        create_draft(&lib, &fork).unwrap();
        let fork_repo = workspace.join("demo-plus");
        identity(&fork_repo);
        assert!(git::version_tags(&fork_repo).is_empty());
        let manifest = lib.load_manifest("demo-plus", &Target::Draft).unwrap();
        assert_eq!(manifest.id, "demo-plus");
        assert_eq!(manifest.forked_from.unwrap().version, "1.0.0");
        let publish_req = PublishRequest {
            id: "demo-plus".into(),
            bump: None,
            version: None,
            changes: vec!["Fork".into()],
            allow_unverified: true,
            replace: false,
        };
        assert_eq!(publish(&lib, &publish_req, false).unwrap().version, "1.0.0");
        assert!(lib.changelog("demo-plus")[0].body.contains("Forked from demo 1.0.0"));
        assert_eq!(lib.list().len(), 2);

        // Discarding: an unpublished skeleton is removed, a published one is reset.
        create_draft(&lib, &req(DraftKind::New, "tmp")).unwrap();
        discard_draft(&lib, "tmp").unwrap();
        assert!(!workspace.join("tmp").exists());
        create_draft(&lib, &req(DraftKind::Edit, "demo")).unwrap();
        fs::write(repo.join("files/extra.txt"), "x").unwrap();
        discard_draft(&lib, "demo").unwrap();
        assert!(!repo.join("files/extra.txt").exists() && !lib.has_draft("demo"));

        // Another data folder (e.g. the CLI's) downloaded 1.1.0 before it is re-published below.
        let other = Library::new(
            tmp.path().join("other-data"),
            &workspace,
            vec![registry.to_string_lossy().into_owned()],
            vec![],
        );
        let v11 = Target::Version(semver::Version::new(1, 1, 0));
        let other_v11 = other.target_dir("demo", &v11).unwrap();

        // Re-publishing the latest version in place (development) keeps one changelog entry.
        fs::write(repo.join("files/README.md.tmpl"), "# {{ project.name }}\n\nReplaced.\n").unwrap();
        let replace = PublishRequest {
            id: "demo".into(),
            bump: None,
            version: None,
            changes: vec!["Better docs again".into()],
            allow_unverified: true,
            replace: true,
        };
        assert_eq!(publish(&lib, &replace, false).unwrap().version, "1.1.0");
        assert_eq!(lib.versions("demo").unwrap().len(), 2);
        assert_eq!(lib.changelog("demo").iter().filter(|e| e.version == "1.1.0").count(), 1);
        let exported = lib.target_dir("demo", &v11).unwrap();
        assert!(fs::read_to_string(exported.join("files/README.md.tmpl")).unwrap().contains("Replaced"));
        // The tag of a local repository moved: syncing the other data folder refreshes its export.
        other.refresh("demo").unwrap();
        assert_eq!(other.target_dir("demo", &v11).unwrap(), other_v11);
        assert!(fs::read_to_string(other_v11.join("files/README.md.tmpl")).unwrap().contains("Replaced"));
    }

    #[test]
    fn discard_stays_inside_the_workspace() {
        let tmp = tempfile::tempdir().unwrap();
        let workspace = tmp.path().join("workspace");
        fs::create_dir_all(&workspace).unwrap();
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join(MANIFEST_FILE), "schema = 1\n").unwrap();
        let lib = Library::new(tmp.path().join("data"), &workspace, vec![], vec![]);
        let absolute = outside.to_string_lossy().into_owned();
        for id in [absolute.as_str(), "../outside", ""] {
            assert!(discard_draft(&lib, id).is_err(), "{id:?}");
        }
        // A link in the workspace that points elsewhere is not followed either.
        std::os::unix::fs::symlink(&outside, workspace.join("linked")).unwrap();
        assert!(discard_draft(&lib, "linked").is_err());
        assert!(outside.join(MANIFEST_FILE).is_file());
    }
}
