//! Dependency update adapters (SPEC §5.9). v1 supports npm (with workspaces).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::authoring;
use crate::error::{Error, IoContext, Result};
use crate::jobs::JobCtx;
use crate::library::{FILES_DIR, Library, Target};
use crate::manifest::Manifest;
use crate::{shell, util};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateLevel {
    Patch,
    Minor,
    Major,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutdatedPackage {
    pub name: String,
    /// Workspace folder relative to the project root; empty for the root package.
    pub workspace: String,
    pub dependency_type: String,
    pub current: Option<String>,
    pub wanted: Option<String>,
    pub latest: String,
    pub level: UpdateLevel,
    /// The `[updates] hold` range the latest version is outside of; level updates skip it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub held: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PackageSelection {
    pub name: String,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

fn ecosystem(manifest: &Manifest) -> Result<String> {
    let ecosystem = manifest
        .updates
        .as_ref()
        .map(|u| u.ecosystem.clone())
        .ok_or_else(|| Error::invalid(format!("skeleton \"{}\" has no [updates] section", manifest.id)))?;
    if ecosystem != "npm" {
        return Err(Error::invalid(format!("dependency updates for \"{ecosystem}\" are not supported yet")));
    }
    Ok(ecosystem)
}

/// Installs a temp copy of the target and lists outdated packages.
pub fn check(lib: &Library, id: &str, target: &Target, ctx: &JobCtx) -> Result<Vec<OutdatedPackage>> {
    let manifest = lib.load_manifest(id, target)?;
    ecosystem(&manifest)?;
    ctx.step("Prepare copy");
    let (_tmp, dir) = authoring::temp_copy(lib, id, target)?;
    ctx.step("Install dependencies");
    shell::run_checked("npm install --ignore-scripts --no-audit --no-fund", &dir, &mut |l| ctx.log(l))?;
    ctx.step("Check outdated packages");
    let mut packages = npm_outdated(&dir, ctx)?;
    mark_held(&mut packages, &manifest);
    ctx.log(format!("{} outdated package(s)", packages.len()));
    Ok(packages)
}

/// Marks packages whose latest version is outside their `[updates] hold` range.
fn mark_held(packages: &mut [OutdatedPackage], manifest: &Manifest) {
    let Some(updates) = &manifest.updates else { return };
    for pkg in packages {
        let Some(range) = updates.hold.get(&pkg.name) else { continue };
        let inside = semver::VersionReq::parse(range)
            .ok()
            .zip(semver::Version::parse(&pkg.latest).ok())
            .is_some_and(|(req, latest)| req.matches(&latest));
        if !inside {
            pkg.held = Some(range.clone());
        }
    }
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// Package name → workspace folder ("" for root).
fn workspace_folders(dir: &Path) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    if let Some(root) = read_json(&dir.join("package.json")) {
        if let Some(name) = root.get("name").and_then(Value::as_str) {
            map.insert(name.to_string(), String::new());
        }
        let patterns: Vec<String> = match root.get("workspaces") {
            Some(Value::Array(items)) => items.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
            Some(Value::Object(obj)) => obj
                .get("packages")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        for pattern in patterns {
            let folders: Vec<String> = match pattern.strip_suffix("/*") {
                Some(parent) => fs::read_dir(dir.join(parent))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter(|e| e.path().join("package.json").is_file())
                    .map(|e| format!("{parent}/{}", e.file_name().to_string_lossy()))
                    .collect(),
                None => vec![pattern.clone()],
            };
            for folder in folders {
                if let Some(name) = read_json(&dir.join(&folder).join("package.json"))
                    .and_then(|j| j.get("name").and_then(Value::as_str).map(str::to_string))
                {
                    map.insert(name, folder);
                }
            }
        }
    }
    map
}

fn npm_outdated(dir: &Path, ctx: &JobCtx) -> Result<Vec<OutdatedPackage>> {
    // At the root, npm lists direct dependencies of the root and of every workspace;
    // `--workspaces` would hide the root's own dependencies.
    let script = "npm outdated --json --long";
    let mut out = String::new();
    // Exit code 1 means "something is outdated".
    let code = shell::run_streaming(script, dir, &mut |line| {
        out.push_str(&line);
        out.push('\n');
    })?;
    if code > 1 {
        ctx.log(out.clone());
        return Err(Error::command(format!("`{script}` exited with code {code}")));
    }
    let start = out.find('{').unwrap_or(out.len());
    let json: Value = serde_json::from_str(out[start..].trim()).unwrap_or(Value::Object(Default::default()));
    let folders = workspace_folders(dir);
    let mut packages = Vec::new();
    if let Value::Object(map) = json {
        for (name, value) in map {
            let entries = match value {
                Value::Array(items) => items,
                other => vec![other],
            };
            for entry in entries {
                let get = |key: &str| entry.get(key).and_then(Value::as_str).map(str::to_string);
                let Some(latest) = get("latest") else { continue };
                let current = get("current");
                // `dependedByLocation` is the workspace folder ("" for the root) in npm 8+.
                let workspace = get("dependedByLocation")
                    .unwrap_or_else(|| folders.get(&get("dependent").unwrap_or_default()).cloned().unwrap_or_default());
                let from = current.clone().or_else(|| get("wanted"));
                let Some(level) = level_between(from.as_deref(), &latest) else { continue };
                packages.push(OutdatedPackage {
                    name: name.clone(),
                    workspace,
                    dependency_type: get("type").unwrap_or_else(|| "dependencies".into()),
                    current,
                    wanted: get("wanted"),
                    latest,
                    level,
                    held: None,
                });
            }
        }
    }
    packages.sort_by(|a, b| (&a.workspace, &a.name).cmp(&(&b.workspace, &b.name)));
    Ok(packages)
}

fn level_between(current: Option<&str>, latest: &str) -> Option<UpdateLevel> {
    let latest = semver::Version::parse(latest).ok()?;
    let Some(current) = current.and_then(|c| semver::Version::parse(c).ok()) else {
        return Some(UpdateLevel::Major);
    };
    if latest <= current {
        None
    } else if latest.major != current.major {
        Some(UpdateLevel::Major)
    } else if latest.minor != current.minor {
        Some(UpdateLevel::Minor)
    } else {
        Some(UpdateLevel::Patch)
    }
}

/// Applies updates into the draft (created from the latest version when missing).
pub fn apply(
    lib: &Library,
    id: &str,
    selection: &[PackageSelection],
    level: Option<UpdateLevel>,
    ctx: &JobCtx,
) -> Result<Value> {
    if selection.is_empty() && level.is_none() {
        return Err(Error::invalid("give \"packages\" or a \"level\""));
    }
    if !lib.has_draft(id) {
        ctx.step("Create draft");
        authoring::create_draft(
            lib,
            &authoring::DraftRequest {
                mode: crate::library::DraftKind::Edit,
                id: id.to_string(),
                source: None,
                from_version: None,
                name: None,
                description: None,
            },
        )?;
        ctx.log("Created a draft from the latest version");
    }
    let manifest = lib.load_manifest(id, &Target::Draft)?;
    ecosystem(&manifest)?;

    ctx.step("Prepare copy");
    let (_tmp, dir) = authoring::temp_copy(lib, id, &Target::Draft)?;
    ctx.step("Install dependencies");
    shell::run_checked("npm install --ignore-scripts --no-audit --no-fund", &dir, &mut |l| ctx.log(l))?;
    ctx.step("Check outdated packages");
    let mut outdated = npm_outdated(&dir, ctx)?;
    mark_held(&mut outdated, &manifest);

    let chosen: Vec<(OutdatedPackage, String)> = outdated
        .into_iter()
        .filter_map(|pkg| {
            if let Some(sel) = selection
                .iter()
                .find(|s| s.name == pkg.name && s.workspace.as_deref().is_none_or(|w| w == pkg.workspace))
            {
                let version = sel.version.clone().unwrap_or_else(|| pkg.latest.clone());
                return Some((pkg, version));
            }
            if let Some(range) = &pkg.held {
                ctx.log(format!("{} {} is held at {range}", pkg.name, pkg.latest));
                return None;
            }
            match level {
                Some(max) if selection.is_empty() && pkg.level <= max => {
                    let version = pkg.latest.clone();
                    Some((pkg, version))
                }
                _ => None,
            }
        })
        .collect();
    if chosen.is_empty() {
        ctx.log("Nothing to update");
        return Ok(serde_json::json!({ "id": id, "updated": [], "draft": lib.draft_dir(id) }));
    }

    ctx.step("Apply updates");
    let mut groups: BTreeMap<(String, bool), Vec<String>> = BTreeMap::new();
    for (pkg, version) in &chosen {
        let dev = pkg.dependency_type == "devDependencies";
        groups.entry((pkg.workspace.clone(), dev)).or_default().push(format!("{}@{version}", pkg.name));
    }
    for ((workspace, dev), specs) in &groups {
        let mut script = format!("npm install --ignore-scripts --no-audit --no-fund {}", specs.join(" "));
        if *dev {
            script.push_str(" --save-dev");
        }
        if !workspace.is_empty() {
            script.push_str(&format!(" --workspace={workspace}"));
        }
        ctx.log(format!("$ {script}"));
        shell::run_checked(&script, &dir, &mut |l| ctx.log(l))?;
    }

    ctx.step("Copy changes to the draft");
    let files = lib.draft_dir(id).join(FILES_DIR);
    let mut copied = Vec::new();
    for rel in util::list_files(&dir)? {
        let name = rel.rsplit('/').next().unwrap_or_default();
        if name != "package.json" && name != "package-lock.json" {
            continue;
        }
        let dst = files.join(&rel);
        if !dst.exists() {
            continue;
        }
        let new = fs::read(dir.join(&rel)).at(&dir.join(&rel))?;
        if fs::read(&dst).ok().as_deref() != Some(new.as_slice()) {
            util::write_atomic(&dst, &new)?;
            copied.push(rel);
        }
    }
    for file in &copied {
        ctx.log(format!("Updated {file}"));
    }
    let updated: Vec<Value> = chosen
        .iter()
        .map(|(pkg, version)| {
            serde_json::json!({
                "name": pkg.name,
                "workspace": pkg.workspace,
                "from": pkg.current.clone().or(pkg.wanted.clone()),
                "to": version,
                "level": pkg.level,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "id": id,
        "updated": updated,
        "files": copied,
        "draft": lib.draft_dir(id),
        "next": "Run verify_skeleton on the draft, then publish_skeleton with a changelog.",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_updates() {
        assert_eq!(level_between(Some("3.5.1"), "3.5.4"), Some(UpdateLevel::Patch));
        assert_eq!(level_between(Some("3.4.1"), "3.5.0"), Some(UpdateLevel::Minor));
        assert_eq!(level_between(Some("3.4.1"), "4.0.0"), Some(UpdateLevel::Major));
        assert_eq!(level_between(Some("3.4.1"), "3.4.1"), None);
        assert_eq!(level_between(None, "1.0.0"), Some(UpdateLevel::Major));
    }

    #[test]
    fn marks_held_packages() {
        let manifest = Manifest::parse(
            "schema = 1\nid = \"a\"\nname = \"A\"\n[updates]\necosystem = \"npm\"\nhold = { typescript = \"<6.1\" }\n",
        )
        .unwrap();
        let pkg = |name: &str, latest: &str| OutdatedPackage {
            name: name.into(),
            workspace: String::new(),
            dependency_type: "devDependencies".into(),
            current: Some("6.0.3".into()),
            wanted: None,
            latest: latest.into(),
            level: UpdateLevel::Major,
            held: None,
        };
        let mut packages = vec![pkg("typescript", "7.0.2"), pkg("typescript", "6.0.9"), pkg("vite", "9.0.0")];
        mark_held(&mut packages, &manifest);
        assert_eq!(packages[0].held.as_deref(), Some("<6.1"));
        assert_eq!(packages[1].held, None);
        assert_eq!(packages[2].held, None);
    }
}
