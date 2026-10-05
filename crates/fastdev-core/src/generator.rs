//! Turns a skeleton version (or draft) into a project folder. See SPEC §6.

use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, TcpListener};
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::edits;
use crate::envfile::EnvFile;
use crate::error::{Error, IoContext, Result};
use crate::library::{AGENTS_TEMPLATE, FILES_DIR, GITIGNORE_SOURCE, SPEC_TEMPLATE};
use crate::manifest::{Manifest, Selection};
use crate::project_meta::{LOCK_FILE, META_FILE, ProjectMeta};
use crate::render::{self, OptionsContext, ProjectContext, SkeletonContext, TEMPLATE_SUFFIX, TemplateContext};
use crate::util::{self, list_files};

pub const CLAUDE_MD: &str = "@AGENTS.md\n";

/// Options of a new project (SPEC §6.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOptions {
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub brief: String,
    #[serde(default = "yes")]
    pub git: bool,
    #[serde(default = "yes")]
    pub install: bool,
    #[serde(default = "yes")]
    pub agents_md: bool,
    #[serde(default = "yes")]
    pub spec_md: bool,
    #[serde(default = "yes")]
    pub claude_md: bool,
    #[serde(default)]
    pub features: BTreeMap<String, bool>,
    /// Choice → option key (e.g. `docker = "full"`); missing choices use their defaults.
    #[serde(default)]
    pub choices: BTreeMap<String, String>,
}

fn yes() -> bool {
    true
}

/// Result of writing the files (phase A).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Materialized {
    pub path: PathBuf,
    pub name: String,
    pub slug: String,
    pub features: BTreeMap<String, bool>,
    pub choices: BTreeMap<String, String>,
    pub ports: IndexMap<String, u16>,
    pub files: usize,
}

pub struct Source<'a> {
    pub dir: &'a Path,
    pub manifest: &'a Manifest,
    /// Version label written to `.fastdev.toml` ("1.0.0" or "draft").
    pub version: &'a str,
    /// Repository and commit of the version (recorded in the project for later updates).
    pub repo: Option<&'a str>,
    pub commit: Option<&'a str>,
}

/// A file of `files/` with its destination and conditions. Any path segment may end with
/// `@key=value` conditions (several allowed, `a|b` for alternatives): `app.blade.php@frontend=react`,
/// `Pages@frontend=vue/Home.vue`. `key` is a choice (value = option) or a feature (`on`/`off`).
/// The file is copied only when every condition holds, without the conditions in its path.
#[derive(Debug, Clone, PartialEq)]
pub struct FileVariant {
    pub dest: String,
    pub conditions: Vec<(String, Vec<String>)>,
}

fn is_condition(text: &str) -> bool {
    let word = |w: &str| {
        !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    };
    text.split_once('=').is_some_and(|(key, values)| {
        key.starts_with(|c: char| c.is_ascii_lowercase()) && word(key) && values.split('|').all(word)
    })
}

pub fn parse_variant(rel: &str) -> FileVariant {
    let mut dest = Vec::new();
    let mut conditions = Vec::new();
    for segment in rel.split('/') {
        let mut parts = segment.split('@');
        let name = parts.next().unwrap_or_default();
        let rest: Vec<&str> = parts.collect();
        if !name.is_empty() && !rest.is_empty() && rest.iter().all(|c| is_condition(c)) {
            dest.push(name);
            for condition in rest {
                let (key, values) = condition.split_once('=').expect("checked above");
                conditions.push((key.to_string(), values.split('|').map(str::to_string).collect()));
            }
        } else {
            dest.push(segment);
        }
    }
    FileVariant { dest: dest.join("/"), conditions }
}

impl FileVariant {
    pub fn applies(&self, selection: &Selection) -> bool {
        self.conditions.iter().all(|(key, values)| {
            if let Some(value) = selection.choices.get(key) {
                values.contains(value)
            } else if let Some(on) = selection.features.get(key) {
                values.iter().any(|v| v == if *on { "on" } else { "off" })
            } else {
                false
            }
        })
    }
}

/// `packages[""].name`-style rendering of a `[[set]]` path for logs.
fn describe_path(path: &[String]) -> String {
    let mut out = String::new();
    for key in path {
        let plain = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || "_-[]=@/".contains(c));
        if plain {
            if !out.is_empty() {
                out.push('.');
            }
            out.push_str(key);
        } else {
            out.push_str(&format!("[{key:?}]"));
        }
    }
    out
}

/// Final project path of a destination: without `.tmpl`, `_gitignore` renamed.
pub fn output_path(dest: &str) -> String {
    let mut out = dest.strip_suffix(TEMPLATE_SUFFIX).unwrap_or(dest).to_string();
    if out == GITIGNORE_SOURCE || out.ends_with(&format!("/{GITIGNORE_SOURCE}")) {
        out = format!("{}.gitignore", &out[..out.len() - GITIGNORE_SOURCE.len()]);
    }
    out
}

/// Whether a file of `files/` matches a pattern set; `*.tmpl` files also match without the suffix.
pub fn matches(set: &GlobSet, rel: &str) -> bool {
    set.is_match(rel) || rel.strip_suffix(TEMPLATE_SUFFIX).is_some_and(|plain| set.is_match(plain))
}

/// Builds a matcher for feature file patterns: exact paths, folders and globs.
pub fn pattern_matcher(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let pattern = pattern.trim_start_matches("./").trim_end_matches('/');
        for p in [pattern.to_string(), format!("{pattern}/**")] {
            builder.add(
                Glob::new(&p).map_err(|err| Error::validation(format!("invalid file pattern \"{pattern}\": {err}")))?,
            );
        }
    }
    builder.build().map_err(|err| Error::validation(err.to_string()))
}

pub fn resolve_slug(options: &CreateOptions) -> Result<String> {
    let name = options.name.trim();
    if name.is_empty() {
        return Err(Error::invalid("project name is required"));
    }
    if name.chars().count() > 80 {
        return Err(Error::invalid("project name must be at most 80 characters"));
    }
    let slug = match options.slug.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(slug) => slug.to_string(),
        None => util::slugify(name),
    };
    if !util::is_valid_slug(&slug) {
        return Err(Error::invalid(format!(
            "slug \"{slug}\" must use lowercase latin letters, digits, dashes, dots or underscores"
        )));
    }
    Ok(slug)
}

/// Picks free ports: at least the manifest value, not in `taken`, bindable on localhost.
pub fn allocate_ports(requested: &IndexMap<String, u16>, taken: &HashSet<u16>) -> Result<IndexMap<String, u16>> {
    let mut assigned = IndexMap::new();
    let mut used: HashSet<u16> = taken.clone();
    for (key, start) in requested {
        let mut port = *start;
        loop {
            if !used.contains(&port) && port_is_free(port) {
                break;
            }
            port = port.checked_add(1).ok_or_else(|| Error::internal(format!("no free port for {key}")))?;
        }
        used.insert(port);
        assigned.insert(key.clone(), port);
    }
    Ok(assigned)
}

/// Free on every address a dev server may bind: IPv4 and IPv6, loopback and wildcard
/// (Vite listens on `localhost`, which macOS resolves to `::1`).
fn port_is_free(port: u16) -> bool {
    let addresses: [IpAddr; 4] = [
        Ipv4Addr::LOCALHOST.into(),
        Ipv4Addr::UNSPECIFIED.into(),
        Ipv6Addr::LOCALHOST.into(),
        Ipv6Addr::UNSPECIFIED.into(),
    ];
    addresses.iter().all(|ip| match TcpListener::bind((*ip, port)) {
        Ok(_) => true,
        Err(err) => err.kind() == std::io::ErrorKind::AddrNotAvailable,
    })
}

/// Phase A: writes all files of the project. The caller removes `target` on error.
pub fn materialize(
    source: &Source<'_>,
    options: &CreateOptions,
    target: &Path,
    taken_ports: &HashSet<u16>,
    log: &mut dyn FnMut(String),
) -> Result<Materialized> {
    let m = source.manifest;
    let slug = resolve_slug(options)?;
    let selection = m.select(&options.features, &options.choices)?;
    let resolved = m.resolve(&selection);
    let options_ctx = OptionsContext {
        git: options.git,
        install: options.install,
        agents_md: options.agents_md,
        spec_md: options.spec_md,
        claude_md: options.claude_md && options.agents_md,
    };
    let jinja = render::environment();
    let ports = allocate_ports(&resolved.ports, taken_ports)?;
    let mut ctx = TemplateContext {
        project: ProjectContext {
            name: options.name.trim().to_string(),
            slug: slug.clone(),
            brief: options.brief.trim().to_string(),
            created_at: util::today(),
        },
        skeleton: SkeletonContext { id: m.id.clone(), name: m.name.clone(), version: source.version.to_string() },
        features: selection.features.clone(),
        choices: selection.choices.clone(),
        options: options_ctx.clone(),
        env: IndexMap::new(),
        ports: ports.clone(),
    };

    // Environment values: manifest env, enabled features, selected options, ports, secrets.
    let mut env_values: IndexMap<String, String> = IndexMap::new();
    for (key, value) in &resolved.env {
        env_values.insert(key.clone(), render::render_str(&jinja, &format!("env.{key}"), value, &ctx)?);
    }
    for (key, port) in &ports {
        env_values.insert(key.clone(), port.to_string());
    }
    let mut ctx_env = env_values.clone();
    for key in m.secrets.keys() {
        ctx_env.insert(key.clone(), String::new());
    }
    ctx.env = ctx_env;

    // Files.
    let files_dir = source.dir.join(FILES_DIR);
    let excluded = pattern_matcher(&resolved.exclude)?;
    let force_included = pattern_matcher(&resolved.include)?;
    fs::create_dir_all(target).at(target)?;
    let mut count = 0;
    for rel in list_files(&files_dir)? {
        let variant = parse_variant(&rel);
        if !variant.applies(&selection) {
            continue;
        }
        let dest = &variant.dest;
        let skipped = |set: &GlobSet| matches(set, &rel) || matches(set, dest);
        if skipped(&excluded) && !skipped(&force_included) {
            continue;
        }
        if (dest == AGENTS_TEMPLATE || dest == "AGENTS.md") && !options.agents_md {
            continue;
        }
        if (dest == SPEC_TEMPLATE || dest == "SPEC.md") && !options.spec_md {
            continue;
        }
        if dest == "CLAUDE.md" && !options_ctx.claude_md {
            continue;
        }
        let src = files_dir.join(&rel);
        let out_rel = output_path(dest);
        let dst = target.join(&out_rel);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).at(parent)?;
        }
        if dest.ends_with(TEMPLATE_SUFFIX) {
            let source_text = fs::read_to_string(&src).at(&src)?;
            let rendered = render::render_str(&jinja, &rel, &source_text, &ctx)?;
            fs::write(&dst, rendered).at(&dst)?;
            let perms = fs::metadata(&src).at(&src)?.permissions();
            fs::set_permissions(&dst, perms).at(&dst)?;
        } else {
            fs::copy(&src, &dst).at(&src)?;
        }
        count += 1;
    }
    log(format!("Copied {count} files"));

    // Structured edits.
    for edit in &m.set {
        if let Some(feature) = &edit.feature
            && !selection.features.get(feature).copied().unwrap_or(false)
        {
            continue;
        }
        let file = target.join(&edit.file);
        if !file.is_file() {
            continue;
        }
        let value = render::render_str(&jinja, &format!("set {}", edit.file), &edit.value, &ctx)?;
        let format = edits::format_of(&edit.file, edit.format.as_deref())
            .ok_or_else(|| Error::validation(format!("[[set]] {}: set format = \"json\" or \"toml\"", edit.file)))?;
        edits::set_value(&file, &edit.path, &value, format)?;
        log(format!("Set {} → {} in {}", describe_path(&edit.path), value, edit.file));
    }

    // .env.example and .env.
    if !env_values.is_empty() || !m.secrets.is_empty() || target.join(".env.example").is_file() {
        let mut example = EnvFile::read(&target.join(".env.example"));
        for (key, value) in &env_values {
            example.set(key, value);
        }
        for key in m.secrets.keys() {
            example.set(key, "");
        }
        util::write_atomic(&target.join(".env.example"), example.render().as_bytes())?;
        let mut env = example.clone();
        for (key, bytes) in &m.secrets {
            env.set(key, &util::random_hex(*bytes));
        }
        util::write_atomic(&target.join(".env"), env.render().as_bytes())?;
        log("Wrote .env and .env.example".to_string());
    }
    // Nested `.env.example` files (e.g. one per package of a workspace) get their `.env` copy.
    for rel in list_files(target)? {
        if let Some(dir) = rel.strip_suffix("/.env.example") {
            let env = target.join(dir).join(".env");
            if !env.exists() {
                fs::copy(target.join(&rel), &env).at(&env)?;
                log(format!("Wrote {dir}/.env from {rel}"));
            }
        }
    }

    if options_ctx.claude_md && !target.join("CLAUDE.md").exists() {
        fs::write(target.join("CLAUDE.md"), CLAUDE_MD).at(&target.join("CLAUDE.md"))?;
    }

    let mut meta = ProjectMeta::new(options.name.trim(), &slug, m, source.version, &selection, &resolved);
    if let Some(skeleton) = meta.skeleton.as_mut() {
        skeleton.repo = source.repo.map(str::to_string);
        skeleton.commit = source.commit.map(str::to_string);
    }
    meta.save(target)?;
    write_lock(target, &meta)?;

    Ok(Materialized {
        path: target.to_path_buf(),
        name: options.name.trim().to_string(),
        slug,
        features: selection.features,
        choices: selection.choices,
        ports,
        files: count,
    })
}

/// Files that are not fingerprinted: local settings and fastDev's own metadata.
const UNTRACKED: &[&str] = &[".env", META_FILE, LOCK_FILE];

/// `.fastdev.lock`: fingerprints of every generated file, the base for safe updates later
/// (reconfiguring options, upgrading to a newer skeleton version).
fn write_lock(target: &Path, meta: &ProjectMeta) -> Result<()> {
    let mut files = BTreeMap::new();
    for rel in list_files(target)? {
        if !UNTRACKED.contains(&rel.as_str()) && !rel.ends_with("/.env") {
            files.insert(rel.clone(), util::sha256_file(&target.join(&rel))?);
        }
    }
    let lock = ProjectLock {
        skeleton: meta.skeleton.as_ref().map(|s| format!("{}@{}", s.id, s.version)).unwrap_or_default(),
        commit: meta.skeleton.as_ref().and_then(|s| s.commit.clone()),
        created_at: meta.project.created_at.clone(),
        files,
    };
    let body = toml::to_string_pretty(&lock).map_err(|err| Error::internal(err.to_string()))?;
    let header = "# Written by fastDev when the project was created: fingerprints of the generated files,\n\
                  # used to reconfigure or upgrade the project safely later. Do not edit.\n\n";
    util::write_atomic(&target.join(LOCK_FILE), format!("{header}{body}").as_bytes())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectLock {
    pub skeleton: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub created_at: String,
    pub files: BTreeMap<String, String>,
}

/// Checks that the target folder can be used.
pub fn check_target(target: &Path) -> Result<()> {
    if target.exists() && !(target.is_dir() && util::is_empty_dir(target)) {
        return Err(Error::conflict(format!("{} already exists and is not empty", target.display())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn sample_skeleton(dir: &Path) -> Manifest {
        write(
            &dir.join("template.toml"),
            r#"
schema = 1
id = "sample"
name = "Sample"
languages = ["node"]

[features.docker]
files = ["Dockerfile", "docker"]
env = { COMPOSE_PROJECT_NAME = "{{ project.slug }}" }

[env]
APP_NAME = "{{ project.name }}"
DB_PATH = "data/{{ project.slug }}.sqlite"

[ports]
APP_PORT = 3000

[secrets]
APP_SECRET = 16

[[set]]
file = "package.json"
path = ["name"]
value = "{{ project.slug }}"

[commands.dev]
label = "Dev"
run = "npm run dev"
long = true
url = "http://localhost:${APP_PORT}"
"#,
        );
        let files = dir.join("files");
        write(&files.join("package.json"), "{\n  \"name\": \"app\",\n  \"private\": true\n}\n");
        write(&files.join(".env.example"), "# Settings\nAPP_NAME=App\nAPP_PORT=3000\nLOG_LEVEL=info\n");
        write(&files.join("_gitignore"), ".env\n");
        write(&files.join("Dockerfile"), "FROM node\n");
        write(&files.join("docker/entry.sh"), "#!/bin/sh\n");
        write(&files.join("web/.env.example"), "VITE_TITLE=Shop\n");
        write(&files.join("AGENTS.md.tmpl"), "# {{ project.name }}\n{% if features.docker %}Docker{% endif %}\n");
        write(&files.join("SPEC.md.tmpl"), "{{ project.brief }}\n");
        write(&files.join("web/App.vue"), "<template>{{ title }}</template>\n");
        Manifest::load(dir).unwrap()
    }

    fn options(docker: bool) -> CreateOptions {
        CreateOptions {
            name: "Мой магазин".into(),
            slug: None,
            brief: "Sell things".into(),
            git: false,
            install: false,
            agents_md: true,
            spec_md: true,
            claude_md: true,
            features: BTreeMap::from([("docker".into(), docker)]),
            choices: BTreeMap::new(),
        }
    }

    #[test]
    fn materializes_project() {
        let tmp = tempfile::tempdir().unwrap();
        let skeleton = tmp.path().join("skeleton");
        let manifest = sample_skeleton(&skeleton);
        let target = tmp.path().join("out");
        let source = Source { dir: &skeleton, manifest: &manifest, version: "1.0.0", repo: None, commit: None };
        let result = materialize(&source, &options(false), &target, &HashSet::new(), &mut |_| {}).unwrap();

        assert_eq!(result.slug, "moi-magazin");
        assert!(target.join(".gitignore").is_file());
        assert!(!target.join("_gitignore").exists());
        assert!(!target.join("Dockerfile").exists());
        assert!(!target.join("docker/entry.sh").exists());
        assert_eq!(fs::read_to_string(target.join("AGENTS.md")).unwrap(), "# Мой магазин\n\n");
        assert_eq!(fs::read_to_string(target.join("web/App.vue")).unwrap(), "<template>{{ title }}</template>\n");
        assert_eq!(fs::read_to_string(target.join("CLAUDE.md")).unwrap(), CLAUDE_MD);
        assert!(fs::read_to_string(target.join("package.json")).unwrap().contains("\"name\": \"moi-magazin\""));

        let env = EnvFile::read(&target.join(".env"));
        assert_eq!(env.get("APP_NAME"), Some("Мой магазин"));
        assert_eq!(env.get("DB_PATH"), Some("data/moi-magazin.sqlite"));
        assert_eq!(env.get("LOG_LEVEL"), Some("info"));
        assert_eq!(env.get("APP_SECRET").map(str::len), Some(32));
        assert_eq!(env.get("COMPOSE_PROJECT_NAME"), None);
        let example = EnvFile::read(&target.join(".env.example"));
        assert_eq!(example.get("APP_SECRET"), Some(""));
        assert!(target.join(".fastdev.toml").is_file());
        assert_eq!(fs::read_to_string(target.join("web/.env")).unwrap(), "VITE_TITLE=Shop\n");
        let lock = fs::read_to_string(target.join(LOCK_FILE)).unwrap();
        assert!(lock.contains("\"package.json\" = ") && !lock.contains("\".env\""), "{lock}");
    }

    #[test]
    fn includes_feature_files_and_skips_taken_ports() {
        let tmp = tempfile::tempdir().unwrap();
        let skeleton = tmp.path().join("skeleton");
        let manifest = sample_skeleton(&skeleton);
        let target = tmp.path().join("out");
        let source = Source { dir: &skeleton, manifest: &manifest, version: "1.0.0", repo: None, commit: None };
        let taken = HashSet::from([3000, 3001]);
        let result = materialize(&source, &options(true), &target, &taken, &mut |_| {}).unwrap();
        assert!(target.join("Dockerfile").is_file());
        assert!(target.join("docker/entry.sh").is_file());
        assert!(result.ports["APP_PORT"] >= 3002);
        let env = EnvFile::read(&target.join(".env"));
        assert_eq!(env.get("COMPOSE_PROJECT_NAME"), Some("moi-magazin"));
        assert!(fs::read_to_string(target.join("AGENTS.md")).unwrap().contains("Docker"));
    }

    #[test]
    fn skips_ports_taken_on_ipv6() {
        let listener = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let ports = allocate_ports(&IndexMap::from([("P".to_string(), port)]), &HashSet::new()).unwrap();
        assert_ne!(ports["P"], port);
    }

    #[test]
    fn parses_file_variants() {
        let v = parse_variant("resources/views/app.blade.php@frontend=react");
        assert_eq!(v.dest, "resources/views/app.blade.php");
        assert_eq!(v.conditions, vec![("frontend".to_string(), vec!["react".to_string()])]);
        let v = parse_variant("resources/js/Pages@frontend=vue|svelte/Home.vue");
        assert_eq!(v.dest, "resources/js/Pages/Home.vue");
        assert_eq!(v.conditions[0].1, ["vue", "svelte"]);
        // Scoped folders and e-mail-like names are not conditions.
        assert_eq!(parse_variant("src/@types/x.d.ts").dest, "src/@types/x.d.ts");
        assert_eq!(parse_variant("a@b.c").conditions, vec![]);

        let selection = Selection {
            features: BTreeMap::from([("auth".into(), true)]),
            choices: BTreeMap::from([("frontend".into(), "vue".into())]),
        };
        assert!(parse_variant("x@frontend=vue@auth=on").applies(&selection));
        assert!(!parse_variant("x@frontend=react").applies(&selection));
        assert!(!parse_variant("x@auth=off").applies(&selection));
        assert_eq!(output_path("config/_gitignore"), "config/.gitignore");
        assert_eq!(output_path("README.md.tmpl"), "README.md");
        assert_eq!(describe_path(&["packages".into(), "".into(), "name".into()]), "packages[\"\"].name");
        assert_eq!(describe_path(&["package[name=app]".into(), "name".into()]), "package[name=app].name");
    }

    #[test]
    fn materializes_file_variants() {
        let tmp = tempfile::tempdir().unwrap();
        let skeleton = tmp.path().join("skeleton");
        write(
            &skeleton.join("template.toml"),
            r#"
schema = 1
id = "demo"
name = "Demo"

[choices.frontend]
label = "Frontend"
default = "vue"
options.vue = { label = "Vue" }
options.react = { label = "React" }
"#,
        );
        let files = skeleton.join("files");
        write(&files.join("AGENTS.md.tmpl"), "{{ choices.frontend }}\n");
        write(&files.join("SPEC.md.tmpl"), "\n");
        write(&files.join("web@frontend=vue/App.vue"), "vue\n");
        write(&files.join("web@frontend=react/App.tsx"), "react\n");
        write(&files.join("vite.config.ts.tmpl@frontend=react"), "// {{ project.slug }}\n");
        let manifest = Manifest::load(&skeleton).unwrap();
        let source = Source { dir: &skeleton, manifest: &manifest, version: "1.0.0", repo: None, commit: None };
        let mut opts = options(false);
        opts.features.clear();
        opts.choices = BTreeMap::from([("frontend".into(), "react".into())]);
        let target = tmp.path().join("out");
        materialize(&source, &opts, &target, &HashSet::new(), &mut |_| {}).unwrap();
        assert!(target.join("web/App.tsx").is_file());
        assert!(!target.join("web/App.vue").exists());
        assert_eq!(fs::read_to_string(target.join("vite.config.ts")).unwrap(), "// moi-magazin\n");
        assert_eq!(fs::read_to_string(target.join("AGENTS.md")).unwrap(), "react\n");
    }

    #[test]
    fn rejects_bad_slugs() {
        let mut opts = options(false);
        opts.slug = Some("Bad Slug".into());
        assert!(resolve_slug(&opts).is_err());
        opts.name = "  ".into();
        assert!(resolve_slug(&opts).is_err());
    }
}
