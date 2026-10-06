//! The single API of fastDev. The UI (Tauri command `call`), the MCP bridge (control socket)
//! and the CLI all go through [`Core::call`]. MCP callers may only use the tools listed in
//! `fastdev_protocol::tools`.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use fastdev_protocol::Caller;
use notify::Watcher;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::authoring::{self, DraftRequest, PublishRequest};
use crate::envfile;
use crate::error::{Error, IoContext, Result};
use crate::events::{Event, EventSink, OutputHub};
use crate::generator::{self, CreateOptions, Source};
use crate::jobs::{Job, JobCtx, JobManager};
use crate::library::{DraftMeta, FILES_DIR, Library, Target};
use crate::manifest::Manifest;
use crate::project_meta::ProjectMeta;
use crate::registry::{ProjectRecord, Registry, SetupStatus};
use crate::runner::{RunInfo, Runner};
use crate::settings::{Editor, Settings, Terminal};
use crate::updates::{self, PackageSelection, UpdateLevel};
use crate::{shell, toolchain, util};

pub const AUTHORING_GUIDE: &str = include_str!("../../../docs/skeleton-authoring.md");
pub const MANIFEST_REFERENCE: &str = include_str!("../../../docs/manifest.md");
pub const MCP_REFERENCE: &str = include_str!("../../../docs/mcp.md");

/// Marker written into a preview folder once its setup succeeded.
const PREVIEW_READY: &str = ".fastdev-preview-ready";
const DRAFT_TARGET: &str = crate::library::DRAFT_DIR;

pub struct CoreConfig {
    pub data_dir: PathBuf,
    pub app_version: String,
    /// Absolute path of the `fastdev-mcp` binary, shown in Settings → MCP.
    pub bridge_path: Option<PathBuf>,
    /// Workspace folder that wins over settings (CLI `--workspace`).
    pub workspace_override: Option<PathBuf>,
    /// Registries that win over settings (CLI `--registry`).
    pub registries_override: Option<Vec<String>>,
}

pub struct Core {
    config: CoreConfig,
    settings: RwLock<Settings>,
    registry: Registry,
    jobs: JobManager,
    runner: Runner,
    hub: Arc<OutputHub>,
    watcher: Mutex<Option<notify::RecommendedWatcher>>,
    /// Latest preview per skeleton id.
    previews: Mutex<HashMap<String, PreviewInfo>>,
    /// Folders of projects being created and the ports they took, until the project is registered
    /// (or its creation failed): concurrent jobs must not share a folder or a port.
    creating: Mutex<HashMap<PathBuf, HashSet<u16>>>,
    /// Held while a creation job allocates its ports and writes its files.
    allocating: Mutex<()>,
}

/// A running or finished preview of a skeleton (SPEC §5.10).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewInfo {
    pub id: String,
    pub target: String,
    pub choices: BTreeMap<String, String>,
    /// The throwaway project folder.
    pub dir: String,
    pub command: String,
    pub label: String,
    pub long: bool,
    pub url: Option<String>,
    /// Existing file or folder to show (e.g. a build output).
    pub path: Option<String>,
    pub message: String,
    pub run_id: Option<String>,
    /// Translations of `label` and `message`: language → English text → translation.
    pub translations: BTreeMap<String, BTreeMap<String, String>>,
}

/// Content of [`PREVIEW_READY`]: what to run before the preview folder is deleted.
#[derive(Debug, serde::Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewMarker {
    ready_at: String,
    #[serde(default)]
    cleanup: Vec<String>,
}

/// Runs the cleanup scripts of a preview folder (`<target>-<hash>`), then deletes it.
fn remove_preview(folder: &Path, id: &str, warn: &mut dyn FnMut(String)) {
    let project = folder.join(format!("preview-{id}"));
    let marker = fs::read_to_string(project.join(PREVIEW_READY))
        .ok()
        .and_then(|text| serde_json::from_str::<PreviewMarker>(&text).ok());
    for script in marker.map(|m| m.cleanup).unwrap_or_default() {
        if let Err(err) = shell::run_cleanup(&script, &project, &mut |_| {}) {
            warn(format!("preview cleanup `{script}` failed: {}", err.message));
        }
    }
    if let Err(err) = fs::remove_dir_all(folder) {
        warn(format!("could not delete {}: {err}", folder.display()));
    }
}

/// Skeleton ids name folders (workspace clone, download cache, previews), so a value like `../x`
/// or an absolute path must never reach a path join. Every method names them `id`, `skeleton`
/// (create_project) or `source` (the parent of a fork).
fn check_skeleton_ids(arguments: &Value) -> Result<()> {
    for key in ["id", "skeleton", "source"] {
        let Some(id) = arguments.get(key).and_then(Value::as_str) else { continue };
        if key == "source" && id.is_empty() {
            continue;
        }
        if !util::is_valid_id(id) {
            return Err(Error::invalid(format!(
                "\"{id}\" is not a valid skeleton id: use lowercase letters, digits and single dashes, starting with a letter"
            )));
        }
    }
    Ok(())
}

fn args<T: DeserializeOwned>(value: Value) -> Result<T> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|err| Error::invalid(format!("invalid arguments: {err}")))
}

#[derive(Deserialize)]
struct IdArg {
    id: String,
}

#[derive(Deserialize)]
struct ProjectArg {
    project: String,
}

#[derive(Deserialize)]
struct TargetArgs {
    id: String,
    #[serde(default, alias = "version")]
    target: Option<String>,
    #[serde(default)]
    wait_seconds: Option<u64>,
}

impl Core {
    pub fn new(config: CoreConfig, sink: Arc<dyn EventSink>) -> Result<Arc<Self>> {
        fs::create_dir_all(&config.data_dir).at(&config.data_dir)?;
        let settings = Settings::load(&config.data_dir.join("settings.json"));
        let registry = Registry::open(&config.data_dir.join("fastdev.db"))?;
        let hub = OutputHub::new(sink);
        let core = Arc::new(Self {
            settings: RwLock::new(settings),
            registry,
            jobs: JobManager::new(hub.clone()),
            runner: Runner::new(hub.clone()),
            hub,
            watcher: Mutex::new(None),
            previews: Mutex::new(HashMap::new()),
            creating: Mutex::new(HashMap::new()),
            allocating: Mutex::new(()),
            config,
        });
        Ok(core)
    }

    fn emit(&self, event: Event) {
        self.hub.sink().emit(event);
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().expect("settings").clone()
    }

    pub fn app_version(&self) -> &str {
        &self.config.app_version
    }

    pub fn data_dir(&self) -> &Path {
        &self.config.data_dir
    }

    pub fn workspace_path(&self) -> PathBuf {
        self.config.workspace_override.clone().unwrap_or_else(|| self.settings().workspace_path)
    }

    pub fn registries(&self) -> Vec<String> {
        self.config.registries_override.clone().unwrap_or_else(|| self.settings().registries)
    }

    pub fn library(&self) -> Result<Library> {
        let settings = self.settings();
        Ok(Library::new(&self.config.data_dir, self.workspace_path(), self.registries(), settings.skeleton_sources))
    }

    /// Pulls registries and downloaded skeletons in the background (app start).
    pub fn sync_in_background(self: &Arc<Self>) {
        let core = self.clone();
        std::thread::spawn(move || {
            if let Ok(lib) = core.library() {
                let report = lib.sync();
                for item in report.registries.iter().chain(report.skeletons.iter()).filter(|i| !i.ok) {
                    log::warn!("sync {}: {}", item.name, item.error.clone().unwrap_or_default());
                }
                core.emit(Event::LibraryChanged);
            }
        });
    }

    pub fn running_runs(&self) -> Vec<RunInfo> {
        self.runner.running()
    }

    /// Stops every project run and every process started by a job (setup, verification…).
    pub fn stop_all_runs(&self) {
        self.runner.stop_all();
        shell::stop_all();
    }

    /// Ports of registered projects and of projects being created.
    fn taken_ports(&self) -> Result<HashSet<u16>> {
        let mut taken = self.registry.taken_ports()?;
        taken.extend(self.creating.lock().expect("creating").values().flatten());
        Ok(taken)
    }

    /// Watches the workspace and local registries and emits `LibraryChanged` (debounced).
    pub fn watch_library(self: &Arc<Self>) {
        let mut paths: Vec<PathBuf> = vec![self.workspace_path()];
        paths.extend(self.registries().iter().filter_map(|url| crate::index::local_path(url)));
        paths.retain(|p| p.is_dir());
        if paths.is_empty() {
            *self.watcher.lock().expect("watcher") = None;
            return;
        }
        let (tx, rx) = mpsc::channel::<()>();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(event) = event {
                let relevant = event.paths.iter().any(|p| {
                    let s = p.to_string_lossy();
                    !s.contains("/node_modules/") && !s.contains("/.git/")
                });
                if relevant {
                    let _ = tx.send(());
                }
            }
        });
        let Ok(mut watcher) = watcher else { return };
        for path in &paths {
            let _ = watcher.watch(path, notify::RecursiveMode::Recursive);
        }
        let weak = Arc::downgrade(self);
        std::thread::spawn(move || {
            while rx.recv().is_ok() {
                while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
                match weak.upgrade() {
                    Some(core) => core.emit(Event::LibraryChanged),
                    None => break,
                }
            }
        });
        *self.watcher.lock().expect("watcher") = Some(watcher);
    }

    /// Entry point for every caller.
    pub fn call(self: &Arc<Self>, method: &str, arguments: Value, caller: Caller) -> Result<Value> {
        if caller == Caller::Mcp && !fastdev_protocol::tools::names().contains(&method) {
            return Err(Error::not_allowed(format!("\"{method}\" is not available through MCP")));
        }
        check_skeleton_ids(&arguments)?;
        let default_wait = if caller == Caller::Ui { 0 } else { 120 };
        match method {
            // General
            "fastdev_status" => self.status(),
            "get_job" => {
                #[derive(Deserialize)]
                struct A {
                    job_id: String,
                    #[serde(default)]
                    wait_seconds: Option<u64>,
                    #[serde(default)]
                    log_lines: Option<usize>,
                }
                let a: A = args(arguments)?;
                let job =
                    self.jobs.get(&a.job_id).ok_or_else(|| Error::not_found(format!("job {} not found", a.job_id)))?;
                Ok(job_response(&job, a.wait_seconds.unwrap_or(0), a.log_lines.unwrap_or(80)))
            }
            // Library
            "list_skeletons" => self.list_skeletons(args(arguments)?),
            "get_skeleton" => {
                let a: TargetArgs = args(arguments)?;
                self.get_skeleton(&a.id, a.target.as_deref())
            }
            "get_authoring_guide" => Ok(json!({
                "workspacePath": self.workspace_path(),
                "registries": self.registries(),
                "guide": AUTHORING_GUIDE,
                "manifestReference": MANIFEST_REFERENCE,
            })),
            // Projects
            "list_projects" => {
                let projects: Vec<Value> = self.registry.list()?.into_iter().map(|r| self.project_view(r)).collect();
                Ok(json!({ "projects": projects }))
            }
            "get_project" => {
                let a: ProjectArg = args(arguments)?;
                Ok(self.project_view(self.registry.find(&a.project)?))
            }
            "create_project" => self.create_project(arguments, default_wait),
            "import_project" => {
                #[derive(Deserialize)]
                struct A {
                    path: String,
                }
                let a: A = args(arguments)?;
                self.import_project(&a.path)
            }
            "remove_project" => {
                let a: ProjectArg = args(arguments)?;
                let record = self.registry.find(&a.project)?;
                self.stop_project_runs(&record.id);
                self.registry.remove(&record.id)?;
                self.runner.forget_project(&record.id);
                self.emit(Event::ProjectsChanged);
                Ok(json!({ "removed": true, "path": record.path, "note": "Files on disk were not touched." }))
            }
            "run_project_command" => self.run_project_command(args(arguments)?),
            "stop_project_command" => {
                #[derive(Deserialize)]
                struct A {
                    project: String,
                    command: String,
                }
                let a: A = args(arguments)?;
                let record = self.registry.find(&a.project)?;
                let run = self
                    .runner
                    .latest(&record.id, &a.command)
                    .ok_or_else(|| Error::not_found(format!("\"{}\" has not been started", a.command)))?;
                let info = self.runner.stop(&run.info().id)?;
                Ok(json!({ "run": info }))
            }
            "get_project_logs" => {
                #[derive(Deserialize)]
                struct A {
                    project: String,
                    #[serde(default)]
                    command: Option<String>,
                    #[serde(default)]
                    lines: Option<usize>,
                }
                let a: A = args(arguments)?;
                let record = self.registry.find(&a.project)?;
                let lines = a.lines.unwrap_or(200).min(5000);
                let runs: Vec<Value> = self
                    .runner
                    .for_project(&record.id)
                    .into_iter()
                    .filter(|r| a.command.as_deref().is_none_or(|c| r.info().command == c))
                    .map(|r| {
                        let (start, tail) = r.tail_with_start(lines);
                        json!({ "run": r.info(), "start": start, "lines": tail })
                    })
                    .collect();
                Ok(json!({ "runs": runs }))
            }
            "run_project_setup" => {
                #[derive(Deserialize)]
                struct A {
                    project: String,
                    #[serde(default)]
                    wait_seconds: Option<u64>,
                }
                let a: A = args(arguments)?;
                self.run_project_setup(&a.project, a.wait_seconds.unwrap_or(default_wait))
            }
            // Skeleton authoring
            "create_skeleton_draft" => {
                let req: DraftRequest = args(arguments)?;
                let info = authoring::create_draft(&self.library()?, &req)?;
                self.emit(Event::LibraryChanged);
                let files = PathBuf::from(&info.path).join(FILES_DIR);
                Ok(json!({
                    "draft": info,
                    "manifestPath": PathBuf::from(&info.path).join("template.toml"),
                    "filesPath": files,
                    "next": "Edit template.toml and files/ with your file tools, then validate_skeleton, verify_skeleton and publish_skeleton. Read get_authoring_guide first if you have not.",
                }))
            }
            "validate_skeleton" => {
                let a: TargetArgs = args(arguments)?;
                let lib = self.library()?;
                let target = lib.resolve_target(&a.id, a.target.as_deref())?;
                Ok(serde_json::to_value(lib.validate(&a.id, &target))?)
            }
            "verify_skeleton" => {
                let a: TargetArgs = args(arguments)?;
                let lib = self.library()?;
                let target = lib.resolve_target(&a.id, a.target.as_deref())?;
                let taken = self.taken_ports()?;
                let core = self.clone();
                let id = a.id.clone();
                let job = self.jobs.start(
                    "verify_skeleton",
                    &format!("{} {}", a.id, target.label()),
                    &["Validate", "Check requirements", "Create test project"],
                    move |ctx| {
                        let result = authoring::verify(&lib, &id, &target, &taken, ctx);
                        core.emit(Event::LibraryChanged);
                        result
                    },
                );
                Ok(job_response(&job, a.wait_seconds.unwrap_or(if caller == Caller::Ui { 0 } else { 300 }), 80))
            }
            "publish_skeleton" => {
                let req: PublishRequest = args(arguments)?;
                if req.replace && caller == Caller::Mcp {
                    return Err(Error::not_allowed("replacing a published version is not available through MCP"));
                }
                let result = authoring::publish(&self.library()?, &req, self.settings().push_on_publish)?;
                self.emit(Event::LibraryChanged);
                Ok(serde_json::to_value(result)?)
            }
            "discard_skeleton_draft" => {
                let a: IdArg = args(arguments)?;
                authoring::discard_draft(&self.library()?, &a.id)?;
                self.emit(Event::LibraryChanged);
                Ok(json!({ "discarded": true }))
            }
            "check_skeleton_updates" => {
                let a: TargetArgs = args(arguments)?;
                let lib = self.library()?;
                let target = lib.resolve_target(&a.id, a.target.as_deref())?;
                let id = a.id.clone();
                let job = self.jobs.start(
                    "check_skeleton_updates",
                    &format!("{} {}", a.id, target.label()),
                    &["Prepare copy", "Install dependencies", "Check outdated packages"],
                    move |ctx| Ok(json!({ "id": id, "target": target.label(), "packages": updates::check(&lib, &id, &target, ctx)? })),
                );
                Ok(job_response(&job, a.wait_seconds.unwrap_or(if caller == Caller::Ui { 0 } else { 300 }), 40))
            }
            "apply_skeleton_updates" => {
                #[derive(Deserialize)]
                struct A {
                    id: String,
                    #[serde(default)]
                    packages: Vec<PackageSelection>,
                    #[serde(default)]
                    level: Option<UpdateLevel>,
                    #[serde(default)]
                    wait_seconds: Option<u64>,
                }
                let a: A = args(arguments)?;
                let lib = self.library()?;
                lib.resolve_target(&a.id, None)?;
                let core = self.clone();
                let id = a.id.clone();
                let job = self.jobs.start(
                    "apply_skeleton_updates",
                    &a.id,
                    &[
                        "Prepare copy",
                        "Install dependencies",
                        "Check outdated packages",
                        "Apply updates",
                        "Copy changes to the draft",
                    ],
                    move |ctx| {
                        let result = updates::apply(&lib, &id, &a.packages, a.level, ctx);
                        core.emit(Event::LibraryChanged);
                        result
                    },
                );
                Ok(job_response(&job, a.wait_seconds.unwrap_or(if caller == Caller::Ui { 0 } else { 300 }), 40))
            }
            "preview_skeleton" => self.preview_skeleton(arguments, if caller == Caller::Ui { 0 } else { 300 }),
            "stop_preview" => {
                let a: IdArg = args(arguments)?;
                self.stop_preview(&a.id);
                Ok(json!({ "stopped": true, "preview": self.preview_view(&a.id) }))
            }
            "get_preview" => {
                let a: IdArg = args(arguments)?;
                Ok(self.preview_view(&a.id))
            }
            "delete_preview" => {
                let a: IdArg = args(arguments)?;
                self.stop_preview(&a.id);
                self.previews.lock().expect("previews").remove(&a.id);
                let dir = self.config.data_dir.join("previews").join(&a.id);
                let mut warnings = Vec::new();
                for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                    remove_preview(&entry.path(), &a.id, &mut |w| warnings.push(w));
                }
                if dir.exists() {
                    fs::remove_dir_all(&dir).at(&dir)?;
                }
                Ok(json!({ "deleted": true, "warnings": warnings }))
            }
            "sync_library" => {
                let report = self.library()?.sync();
                self.emit(Event::LibraryChanged);
                Ok(serde_json::to_value(report)?)
            }
            // UI / CLI only
            "app_info" => Ok(self.app_info()),
            "add_skeleton_source" => {
                #[derive(Deserialize)]
                struct A {
                    url: String,
                }
                let a: A = args(arguments)?;
                let url = a.url.trim().to_string();
                if url.is_empty() {
                    return Err(Error::invalid("repository URL is empty"));
                }
                let id = self.library()?.download_source(&url)?;
                let mut sources = self.settings().skeleton_sources;
                if !sources.contains(&url) {
                    sources.push(url);
                }
                self.update_settings(json!({ "skeletonSources": sources }))?;
                Ok(json!({ "id": id }))
            }
            "remove_skeleton_source" => {
                #[derive(Deserialize)]
                struct A {
                    url: String,
                }
                let a: A = args(arguments)?;
                let mut sources = self.settings().skeleton_sources;
                sources.retain(|s| s != &a.url);
                self.update_settings(json!({ "skeletonSources": sources }))
            }
            "skeleton_requirements" => {
                #[derive(Deserialize)]
                struct A {
                    id: String,
                    #[serde(default)]
                    version: Option<String>,
                    #[serde(default)]
                    features: BTreeMap<String, bool>,
                    #[serde(default)]
                    choices: BTreeMap<String, String>,
                }
                let a: A = args(arguments)?;
                let lib = self.library()?;
                let target = lib.resolve_target(&a.id, a.version.as_deref())?;
                let manifest = lib.load_manifest(&a.id, &target)?;
                let selection = manifest.select(&a.features, &a.choices)?;
                let resolved = manifest.resolve(&selection);
                let requirements: Vec<Value> = toolchain::check_requirements(&resolved.requirements)
                    .into_iter()
                    .map(|status| {
                        let needed = resolved.needs_tool(&manifest, &status.tool, &[]);
                        let mut value = serde_json::to_value(&status).unwrap_or_default();
                        value["neededForSetup"] = json!(needed);
                        value
                    })
                    .collect();
                Ok(json!({
                    "choices": selection.choices,
                    "requirements": requirements,
                    "setup": resolved.setup,
                    "commands": resolved.commands.keys().collect::<Vec<_>>(),
                }))
            }
            "get_settings" => Ok(self.settings_view()),
            "update_settings" => self.update_settings(arguments),
            "set_favorite" => {
                #[derive(Deserialize)]
                struct A {
                    id: String,
                    favorite: bool,
                }
                let a: A = args(arguments)?;
                let mut favorites = self.settings().favorites;
                favorites.retain(|f| f != &a.id);
                if a.favorite {
                    favorites.push(a.id);
                }
                self.update_settings(json!({ "favorites": favorites }))
            }
            "toolchain" => {
                #[derive(Deserialize)]
                struct A {
                    #[serde(default)]
                    refresh: bool,
                }
                let a: A = args(arguments)?;
                if a.refresh {
                    toolchain::clear_cache();
                }
                Ok(json!({ "tools": toolchain::overview(), "path": toolchain::login_path() }))
            }
            "list_jobs" => Ok(json!({ "jobs": self.jobs.list() })),
            "list_runs" => Ok(json!({ "runs": self.runner.list() })),
            "get_run_logs" => {
                #[derive(Deserialize)]
                struct A {
                    run_id: String,
                    #[serde(default)]
                    lines: Option<usize>,
                }
                let a: A = args(arguments)?;
                let run = self.runner.get(&a.run_id).ok_or_else(|| Error::not_found("run not found"))?;
                let (start, lines) = run.tail_with_start(a.lines.unwrap_or(5000));
                Ok(json!({ "run": run.info(), "start": start, "lines": lines }))
            }
            "clear_run_logs" => {
                #[derive(Deserialize)]
                struct A {
                    run_id: String,
                }
                let a: A = args(arguments)?;
                if let Some(run) = self.runner.get(&a.run_id) {
                    run.clear();
                }
                Ok(json!({ "cleared": true }))
            }
            "stop_run" => {
                #[derive(Deserialize)]
                struct A {
                    run_id: String,
                }
                let a: A = args(arguments)?;
                Ok(json!({ "run": self.runner.stop(&a.run_id)? }))
            }
            "touch_project" => {
                let a: ProjectArg = args(arguments)?;
                let record = self.registry.find(&a.project)?;
                self.registry.touch(&record.id, &util::now_rfc3339())?;
                Ok(json!({ "ok": true }))
            }
            "locate_project" => {
                #[derive(Deserialize)]
                struct A {
                    project: String,
                    path: String,
                }
                let a: A = args(arguments)?;
                let record = self.registry.find(&a.project)?;
                let path = canonical_dir(&a.path)?;
                self.registry.set_path(&record.id, &path)?;
                self.emit(Event::ProjectsChanged);
                Ok(self.project_view(self.registry.find(&record.id)?))
            }
            "delete_project_from_disk" => {
                if caller != Caller::Ui {
                    return Err(Error::not_allowed("projects can only be deleted from disk in the fastDev window"));
                }
                #[derive(Deserialize)]
                struct A {
                    project: String,
                    confirm: String,
                }
                let a: A = args(arguments)?;
                let record = self.registry.find(&a.project)?;
                if a.confirm != record.slug {
                    return Err(Error::invalid("type the project slug to confirm"));
                }
                self.stop_project_runs(&record.id);
                let path = PathBuf::from(&record.path);
                if path.is_dir() {
                    ensure_deletable(&path)?;
                    fs::remove_dir_all(&path).at(&path)?;
                }
                self.registry.remove(&record.id)?;
                self.runner.forget_project(&record.id);
                self.emit(Event::ProjectsChanged);
                Ok(json!({ "deleted": true }))
            }
            "open_project" => {
                #[derive(Deserialize)]
                struct A {
                    project: String,
                    target: String,
                }
                let a: A = args(arguments)?;
                let record = self.registry.find(&a.project)?;
                let settings = self.settings();
                match a.target.as_str() {
                    "editor" => open_in_editor(&settings, &record.path)?,
                    "terminal" => open_app(terminal_app(settings.terminal), &record.path)?,
                    "finder" => open_path(&record.path)?,
                    other => return Err(Error::invalid(format!("unknown target \"{other}\""))),
                }
                let _ = self.registry.touch(&record.id, &util::now_rfc3339());
                Ok(json!({ "ok": true }))
            }
            "open_url" => {
                #[derive(Deserialize)]
                struct A {
                    url: String,
                }
                let a: A = args(arguments)?;
                if !(a.url.starts_with("http://") || a.url.starts_with("https://")) {
                    return Err(Error::invalid("only http(s) URLs can be opened"));
                }
                open_path(&a.url)?;
                Ok(json!({ "ok": true }))
            }
            "open_path" => {
                #[derive(Deserialize)]
                struct A {
                    path: String,
                    #[serde(default)]
                    target: Option<String>,
                }
                let a: A = args(arguments)?;
                if !Path::new(&a.path).exists() {
                    return Err(Error::not_found(format!("{} does not exist", a.path)));
                }
                let settings = self.settings();
                match a.target.as_deref().unwrap_or("finder") {
                    "editor" => open_in_editor(&settings, &a.path)?,
                    "terminal" => open_app(terminal_app(settings.terminal), &a.path)?,
                    "finder" => open_path(&a.path)?,
                    other => return Err(Error::invalid(format!("unknown target \"{other}\""))),
                }
                Ok(json!({ "ok": true }))
            }
            other => Err(Error::not_found(format!("unknown method \"{other}\""))),
        }
    }

    fn preview_run_owner(id: &str) -> String {
        format!("preview:{id}")
    }

    fn stop_preview(&self, id: &str) {
        for run in self.runner.for_project(&Self::preview_run_owner(id)) {
            if run.is_running() {
                let _ = self.runner.stop(&run.info().id);
            }
        }
    }

    /// Preview state with its latest run and output tail.
    fn preview_view(&self, id: &str) -> Value {
        let info = self.previews.lock().expect("previews").get(id).cloned();
        let run = info.as_ref().and_then(|i| i.run_id.as_ref()).and_then(|run_id| self.runner.get(run_id));
        let (start, lines) = run.as_ref().map(|r| r.tail_with_start(200)).unwrap_or((0, Vec::new()));
        json!({
            "preview": info,
            "run": run.map(|r| r.info()),
            "start": start,
            "lines": lines,
        })
    }

    /// Sets up a throwaway copy of a skeleton (once per version and selection) and runs its
    /// preview command, without registering a project.
    fn preview_skeleton(self: &Arc<Self>, arguments: Value, default_wait: u64) -> Result<Value> {
        #[derive(Deserialize)]
        struct A {
            id: String,
            #[serde(default)]
            version: Option<String>,
            #[serde(default)]
            features: BTreeMap<String, bool>,
            #[serde(default)]
            choices: BTreeMap<String, String>,
            #[serde(default)]
            wait_seconds: Option<u64>,
        }
        let a: A = args(arguments)?;
        let lib = self.library()?;
        let target = match a.version.as_deref() {
            Some(DRAFT_TARGET) => lib.resolve_target(&a.id, Some(DRAFT_TARGET))?,
            other => lib.resolve_published(&a.id, other)?,
        };
        let manifest = lib.load_manifest(&a.id, &target)?;
        let selection = manifest.select(&a.features, &a.choices)?;
        let resolved = manifest.resolve(&selection);
        let key = manifest.preview_command(&resolved).ok_or_else(|| {
            Error::invalid(format!("skeleton \"{}\" has nothing to preview (no [preview] or primary command)", a.id))
        })?;
        let command =
            resolved.commands.get(&key).cloned().ok_or_else(|| {
                Error::invalid(format!("preview command \"{key}\" is not available with these options"))
            })?;
        let preview = resolved.preview.clone();
        let skeleton_dir = lib.target_dir(&a.id, &target)?;
        // A preview folder is reused while the skeleton content and the selection stay the same:
        // `<target>-<content>-<selection>`. The content of a draft changes with every edit; a
        // version re-published in place gets a new commit.
        let content = match &target {
            Target::Draft => {
                let manifest_text = fs::read_to_string(skeleton_dir.join(crate::manifest::MANIFEST_FILE))
                    .at(&skeleton_dir.join(crate::manifest::MANIFEST_FILE))?;
                util::short_hash(&format!("{manifest_text}\n{}", util::hash_tree(&skeleton_dir.join(FILES_DIR), &[])?))
            }
            Target::Version(v) => util::short_hash(&lib.version_commit(&a.id, v).unwrap_or_else(|| v.to_string())),
        };
        let selection_hash = util::short_hash(&serde_json::to_string(&selection)?);
        let slug_hash = util::short_hash(&format!("{}\n{selection_hash}", target.label()))[..6].to_string();
        let previews_dir = self.config.data_dir.join("previews").join(&a.id);
        let content_prefix = format!("{}-{}-", target.label(), &content[..8]);
        let folder = previews_dir.join(format!("{content_prefix}{}", &selection_hash[..8]));
        let project = folder.join(format!("preview-{}", a.id));
        let ready = project.join(PREVIEW_READY).is_file();
        self.stop_preview(&a.id);
        // Folders of older content of the same target (an edited draft, a re-published version).
        let stale: Vec<PathBuf> = fs::read_dir(&previews_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                name.starts_with(&format!("{}-", target.label())) && !name.starts_with(&content_prefix)
            })
            .collect();

        let label = if command.label.is_empty() { key.clone() } else { command.label.clone() };
        let mut steps: Vec<String> = vec!["Check requirements".into()];
        if !ready {
            steps.push("Copy files".into());
            steps.extend(resolved.setup.iter().map(|s| s.label.clone()));
        }
        steps.push(format!("Start {label}"));
        let step_refs: Vec<&str> = steps.iter().map(String::as_str).collect();

        let core = self.clone();
        let id = a.id.clone();
        let version = target.label();
        let translations = manifest.translations_of(&step_refs);
        let title = format!("{} {version}", a.id);
        let job = self.jobs.start_translated("preview_skeleton", &title, &step_refs, translations, move |ctx| {
            ctx.step("Check requirements");
            authoring::check_tools(&manifest, &resolved, &[command.run.as_str()], true, "", ctx)?;
            for dir in &stale {
                ctx.log(format!("Removing an outdated preview: {}", dir.display()));
                remove_preview(dir, &id, &mut |w| ctx.warn(w));
            }
            if ready {
                ctx.log(format!("Reusing the preview prepared earlier: {}", project.display()));
            } else {
                ctx.step("Copy files");
                if project.exists() {
                    fs::remove_dir_all(&project).at(&project)?;
                }
                let options = CreateOptions {
                    name: format!("{} preview", manifest.name),
                    // Unique per target and selection: the slug names Docker Compose projects, images,
                    // volumes and app data folders, so copies of other selections are not touched.
                    // Outdated copies of the same selection are cleaned up before this one starts.
                    slug: Some(format!("preview-{id}-{slug_hash}")),
                    brief: String::new(),
                    git: false,
                    install: true,
                    agents_md: true,
                    spec_md: true,
                    claude_md: false,
                    features: selection.features.clone(),
                    choices: selection.choices.clone(),
                };
                let source =
                    Source { dir: &skeleton_dir, manifest: &manifest, version: &version, repo: None, commit: None };
                let taken = core.taken_ports()?;
                generator::materialize(&source, &options, &project, &taken, &mut |line| ctx.log(line))?;
                authoring::run_steps(&resolved.setup, &project, ctx)?;
                let cleanup = resolved.cleanup.iter().chain(&preview.cleanup).cloned().collect();
                let marker = PreviewMarker { ready_at: util::now_rfc3339(), cleanup };
                util::write_atomic(&project.join(PREVIEW_READY), serde_json::to_string_pretty(&marker)?.as_bytes())?;
            }

            ctx.step(&format!("Start {label}"));
            let env = envfile::read_values(&project.join(".env"));
            let url = preview.url.clone().or_else(|| command.url.clone()).map(|u| envfile::interpolate(&u, &env));
            let no_inputs = BTreeMap::new();
            let run =
                core.runner.start(&Self::preview_run_owner(&id), &project, &key, &command, url.clone(), &no_inputs)?;
            let handle = core.runner.get(&run.id).ok_or_else(|| Error::internal("preview run disappeared"))?;
            // Long commands: give the server a moment and fail fast when it exits at once.
            // Short commands (builds): wait until they finish.
            let mut finished =
                handle.wait(if command.long { Duration::from_secs(4) } else { Duration::from_secs(1800) });
            // A server is announced only once it answers (Docker may build images first).
            if let Some(url) = url.as_deref().filter(|_| finished.status == crate::runner::RunStatus::Running) {
                ctx.log(format!("Waiting for {url} …"));
                let deadline = std::time::Instant::now() + Duration::from_secs(300);
                loop {
                    if util::http_answers(url) {
                        break;
                    }
                    finished = handle.wait(Duration::from_secs(1));
                    if finished.status != crate::runner::RunStatus::Running {
                        break;
                    }
                    if std::time::Instant::now() > deadline {
                        ctx.warn(format!("{url} did not answer within 5 minutes; the server may still be starting"));
                        break;
                    }
                }
            }
            if finished.status != crate::runner::RunStatus::Running && finished.exit_code != Some(0) {
                for line in handle.tail(30) {
                    ctx.log(line);
                }
                return Err(Error::command(format!("`{}` exited with code {:?}", command.run, finished.exit_code)));
            }
            let path = preview
                .path
                .as_ref()
                .map(|p| project.join(p))
                .filter(|p| p.exists())
                .map(|p| p.to_string_lossy().into_owned());
            let mut vars = env.clone();
            vars.insert("PREVIEW_DIR".into(), project.to_string_lossy().into_owned());
            let message = envfile::interpolate(&preview.message, &vars);
            // The message is interpolated here, so its translations are interpolated too.
            let translations = manifest
                .translations
                .iter()
                .map(|(lang, dict)| {
                    let mut own = BTreeMap::new();
                    if let Some(text) = dict.get(&preview.message) {
                        own.insert(message.clone(), envfile::interpolate(text, &vars));
                    }
                    if let Some(text) = dict.get(&label) {
                        own.insert(label.clone(), text.clone());
                    }
                    (lang.clone(), own)
                })
                .filter(|(_, own)| !own.is_empty())
                .collect();
            let info = PreviewInfo {
                id: id.clone(),
                target: version.clone(),
                choices: selection.choices.clone(),
                dir: project.to_string_lossy().into_owned(),
                command: key.clone(),
                label: label.clone(),
                long: command.long,
                url,
                path,
                message,
                run_id: Some(run.id.clone()),
                translations,
            };
            core.previews.lock().expect("previews").insert(id.clone(), info);
            Ok(core.preview_view(&id))
        });
        Ok(job_response(&job, a.wait_seconds.unwrap_or(default_wait), 60))
    }

    fn status(&self) -> Result<Value> {
        Ok(json!({
            "appVersion": self.config.app_version,
            "workspacePath": self.workspace_path(),
            "registries": self.registries(),
            "projectsDir": self.settings().projects_dir,
            "dataDir": self.config.data_dir,
            "toolchain": toolchain::overview(),
            "runningProcesses": self.runner.running(),
            "projects": self.registry.list()?.len(),
        }))
    }

    fn app_info(&self) -> Value {
        let bridge = self.config.bridge_path.as_ref().map(|p| p.to_string_lossy().into_owned());
        let bridge_display =
            bridge.clone().unwrap_or_else(|| "/Applications/fastDev.app/Contents/MacOS/fastdev-mcp".into());
        json!({
            "version": self.config.app_version,
            "dataDir": self.config.data_dir,
            "socketPath": fastdev_protocol::socket_path(),
            "bridgePath": bridge,
            "bridgeExists": bridge.as_ref().is_some_and(|b| Path::new(b).is_file()),
            "claudeCommand": format!("claude mcp add --scope user fastdev -- \"{bridge_display}\""),
            "codexConfig": format!("[mcp_servers.fastdev]\ncommand = \"{bridge_display}\"\n"),
        })
    }

    fn settings_view(&self) -> Value {
        let settings = self.settings();
        serde_json::to_value(&settings).unwrap_or_default()
    }

    fn update_settings(self: &Arc<Self>, patch: Value) -> Result<Value> {
        let Value::Object(patch) = patch else {
            return Err(Error::invalid("settings patch must be an object"));
        };
        let old = self.settings();
        let mut value = serde_json::to_value(&old)?;
        for (key, v) in patch {
            if value.get(&key).is_none() {
                return Err(Error::invalid(format!("unknown setting \"{key}\"")));
            }
            value[key] = v;
        }
        let new: Settings =
            serde_json::from_value(value).map_err(|err| Error::invalid(format!("invalid settings: {err}")))?;
        new.save(&self.config.data_dir.join("settings.json"))?;
        *self.settings.write().expect("settings") = new.clone();
        self.emit(Event::SettingsChanged { settings: Box::new(new.clone()) });
        if new.workspace_path != old.workspace_path
            || new.registries != old.registries
            || new.skeleton_sources != old.skeleton_sources
        {
            self.watch_library();
            self.emit(Event::LibraryChanged);
        }
        Ok(self.settings_view())
    }

    fn list_skeletons(&self, filter: SkeletonFilter) -> Result<Value> {
        let lib = self.library()?;
        let favorites = self.settings().favorites;
        let query = filter.query.unwrap_or_default().to_lowercase();
        let skeletons: Vec<Value> = lib
            .list()
            .into_iter()
            .filter(|s| {
                filter.language.as_deref().is_none_or(|l| s.languages.iter().any(|x| x.eq_ignore_ascii_case(l)))
            })
            .filter(|s| filter.category.as_deref().is_none_or(|c| s.category.eq_ignore_ascii_case(c)))
            .filter(|s| {
                query.is_empty()
                    || [&s.id, &s.name, &s.description].iter().any(|f| f.to_lowercase().contains(&query))
                    || s.tags.iter().chain(s.stack.iter()).any(|t| t.to_lowercase().contains(&query))
            })
            .map(|s| {
                let mut value = serde_json::to_value(&s).unwrap_or_default();
                value["favorite"] = json!(favorites.contains(&s.id));
                value
            })
            .collect();
        Ok(json!({ "workspacePath": lib.workspace(), "registries": lib.registries(), "skeletons": skeletons }))
    }

    fn get_skeleton(&self, id: &str, target: Option<&str>) -> Result<Value> {
        let lib = self.library()?;
        let target = lib.resolve_target(id, target)?;
        let dir = lib.target_dir(id, &target)?;
        let summary = lib.summary(id);
        let validation = lib.validate(id, &target);
        let manifest = lib.load_manifest(id, &target).ok();
        let requirements =
            manifest.as_ref().map(|m| toolchain::check_requirements(&m.requirements)).unwrap_or_default();
        let feature_requirements: BTreeMap<String, Value> = manifest
            .as_ref()
            .map(|m| {
                m.features
                    .iter()
                    .map(|(name, f)| (name.clone(), json!(toolchain::check_requirements(&f.requirements))))
                    .collect()
            })
            .unwrap_or_default();
        let draft_up_to_date = if target == Target::Draft {
            let hash = authoring::draft_hash(&lib, id)?;
            DraftMeta::load(&lib.draft_dir(id)).and_then(|m| m.verification).map(|v| v.passed && v.hash == hash)
        } else {
            None
        };
        let parent = manifest.as_ref().and_then(|m| m.forked_from.clone()).map(|origin| {
            let latest = lib.latest(&origin.id).map(|v| v.to_string());
            let newer = match (semver::Version::parse(&origin.version), latest.as_deref().map(semver::Version::parse)) {
                (Ok(from), Some(Ok(latest))) => latest > from,
                _ => false,
            };
            let changes: Vec<_> = lib
                .changelog(&origin.id)
                .into_iter()
                .filter(|entry| {
                    matches!((semver::Version::parse(&entry.version), semver::Version::parse(&origin.version)), (Ok(v), Ok(from)) if v > from)
                })
                .collect();
            json!({ "id": origin.id, "version": origin.version, "latest": latest, "newer": newer, "changelog": changes })
        });
        Ok(json!({
            "summary": summary,
            "target": target.label(),
            "manifest": manifest,
            "requirements": requirements,
            "featureRequirements": feature_requirements,
            "changelog": lib.changelog(id),
            "validation": validation,
            "draftVerified": draft_up_to_date,
            "parent": parent,
            "favorite": self.settings().favorites.contains(&id.to_string()),
            "paths": {
                "repo": lib.repo_url(id),
                "workspace": lib.draft_dir(id),
                "target": dir,
                "files": dir.join(FILES_DIR),
                "manifest": dir.join("template.toml"),
            },
        }))
    }

    fn project_view(&self, record: ProjectRecord) -> Value {
        let path = PathBuf::from(&record.path);
        let exists = path.is_dir();
        let (meta, meta_error) = if exists {
            match ProjectMeta::load(&path) {
                Ok(Some(meta)) => (meta, None),
                Ok(None) => (ProjectMeta::infer(&path), None),
                Err(err) => (ProjectMeta::infer(&path), Some(err.message)),
            }
        } else {
            (ProjectMeta::infer(&path), None)
        };
        let env = envfile::read_values(&path.join(".env"));
        let runs = self.runner.for_project(&record.id);
        let commands: Vec<Value> = meta
            .active_commands()
            .into_iter()
            .map(|(key, cmd)| {
                let run = runs.iter().find(|r| r.info().command == key).map(|r| r.info());
                json!({
                    "key": key,
                    "label": if cmd.label.is_empty() { key.clone() } else { cmd.label.clone() },
                    "run": cmd.run,
                    "description": cmd.description,
                    "long": cmd.long,
                    "primary": cmd.primary,
                    "url": cmd.url.as_ref().map(|u| envfile::interpolate(u, &env)),
                    "inputs": cmd.inputs,
                    "lastRun": run,
                })
            })
            .collect();
        let latest = record.skeleton_id.as_deref().and_then(|id| self.library().ok().and_then(|lib| lib.latest(id)));
        let update_available = match (&latest, record.skeleton_version.as_deref().map(semver::Version::parse)) {
            (Some(latest), Some(Ok(current))) => latest > &current,
            _ => false,
        };
        let running: Vec<String> = runs.iter().filter(|r| r.is_running()).map(|r| r.info().command).collect();
        let mut value = serde_json::to_value(&record).unwrap_or_default();
        value["exists"] = json!(exists);
        value["choices"] = json!(meta.choices);
        value["translations"] = json!(meta.translations);
        value["commands"] = json!(commands);
        value["setup"] = json!(meta.active_setup());
        value["running"] = json!(running);
        value["latestVersion"] = json!(latest.map(|v| v.to_string()));
        value["updateAvailable"] = json!(update_available);
        value["metaError"] = json!(meta_error);
        value
    }

    fn stop_project_runs(&self, project_id: &str) {
        for run in self.runner.for_project(project_id) {
            if run.is_running() {
                let _ = self.runner.stop(&run.info().id);
            }
        }
    }

    fn create_project(self: &Arc<Self>, arguments: Value, default_wait: u64) -> Result<Value> {
        #[derive(Deserialize)]
        struct A {
            skeleton: String,
            #[serde(default)]
            version: Option<String>,
            name: String,
            #[serde(default)]
            slug: Option<String>,
            #[serde(default)]
            parent_dir: Option<String>,
            #[serde(default)]
            git: Option<bool>,
            #[serde(default)]
            install: Option<bool>,
            #[serde(default)]
            features: BTreeMap<String, bool>,
            #[serde(default)]
            choices: BTreeMap<String, String>,
            #[serde(default)]
            agents_md: Option<bool>,
            #[serde(default)]
            spec_md: Option<bool>,
            #[serde(default)]
            claude_md: Option<bool>,
            #[serde(default)]
            brief: Option<String>,
            #[serde(default)]
            wait_seconds: Option<u64>,
        }
        let a: A = args(arguments)?;
        let lib = self.library()?;
        // "draft" creates a real project from the workspace clone (to test a skeleton end to end).
        let target = match a.version.as_deref() {
            Some(DRAFT_TARGET) => lib.resolve_target(&a.skeleton, Some(DRAFT_TARGET))?,
            other => lib.resolve_published(&a.skeleton, other)?,
        };
        let manifest = lib.load_manifest(&a.skeleton, &target)?;
        let options = CreateOptions {
            name: a.name.clone(),
            slug: a.slug.clone(),
            brief: a.brief.clone().unwrap_or_default(),
            git: a.git.unwrap_or(true),
            install: a.install.unwrap_or(true),
            agents_md: a.agents_md.unwrap_or(true),
            spec_md: a.spec_md.unwrap_or(true),
            claude_md: a.claude_md.unwrap_or(true),
            features: a.features.clone(),
            choices: a.choices.clone(),
        };
        let explicit_slug = options.slug.as_deref().is_some_and(|s| !s.trim().is_empty());
        let slug = generator::resolve_slug(&options)?;
        let resolved = manifest.resolve(&manifest.select(&options.features, &options.choices)?);
        let parent = a
            .parent_dir
            .as_deref()
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(util::expand_home)
            .unwrap_or_else(|| self.settings().projects_dir);
        if !parent.is_absolute() {
            return Err(Error::invalid("parent_dir must be an absolute path"));
        }
        let (slug, claim) = self.claim_project(&parent, &slug, explicit_slug)?;
        let project_dir = claim.dir.clone();
        let mut options = options;
        options.slug = Some(slug);

        let mut steps: Vec<String> = vec!["Check requirements".into(), "Copy files".into()];
        let setup_steps = resolved.setup.clone();
        if options.install {
            steps.extend(setup_steps.iter().map(|s| s.label.clone()));
        }
        if options.git {
            steps.push("Initialize git".into());
        }
        steps.push("Register project".into());
        let step_refs: Vec<&str> = steps.iter().map(String::as_str).collect();

        let core = self.clone();
        let title = options.name.trim().to_string();
        let version_label = target.label();
        let skeleton_dir = lib.target_dir(&a.skeleton, &target)?;
        let origin = SkeletonOrigin {
            repo: lib.repo_url(&a.skeleton),
            commit: match &target {
                Target::Version(v) => lib.version_commit(&a.skeleton, v),
                Target::Draft => None,
            },
        };
        let translations = manifest.translations_of(&step_refs);
        let job = self.jobs.start_translated("create_project", &title, &step_refs, translations, move |ctx| {
            let _claim = claim;
            core.create_project_job(
                ctx,
                &skeleton_dir,
                &manifest,
                &version_label,
                &origin,
                &options,
                &project_dir,
                &setup_steps,
            )
        });
        Ok(job_response(&job, a.wait_seconds.unwrap_or(default_wait), 60))
    }

    #[allow(clippy::too_many_arguments)]
    fn create_project_job(
        &self,
        ctx: &JobCtx,
        skeleton_dir: &Path,
        manifest: &Manifest,
        version: &str,
        origin: &SkeletonOrigin,
        options: &CreateOptions,
        project_dir: &Path,
        setup_steps: &[crate::manifest::SetupStep],
    ) -> Result<Value> {
        let resolved = manifest.resolve(&manifest.select(&options.features, &options.choices)?);
        let mut options = options.clone();

        ctx.step("Check requirements");
        authoring::check_tools(
            manifest,
            &resolved,
            &[],
            options.install,
            " Install the missing tools, or turn off “Install dependencies” to create the project without setup.",
            ctx,
        )?;
        if options.git && !toolchain::detect("git").found {
            ctx.warn("git is not installed; the repository was not initialised");
            options.git = false;
            ctx.skip("Initialize git");
        }

        ctx.step("Copy files");
        // The folder was checked when the job was requested; another process may have used it since.
        let existed = take_project_dir(project_dir)?;
        let source = Source {
            dir: skeleton_dir,
            manifest,
            version,
            repo: origin.repo.as_deref(),
            commit: origin.commit.as_deref(),
        };
        let materialized = {
            // Ports count as taken from here until the project is registered.
            let _allocating = self.allocating.lock().expect("allocating");
            let taken = self.taken_ports()?;
            match generator::materialize(&source, &options, project_dir, &taken, &mut |line| ctx.log(line)) {
                Ok(m) => {
                    if let Some(ports) = self.creating.lock().expect("creating").get_mut(project_dir) {
                        ports.extend(m.ports.values());
                    }
                    m
                }
                Err(err) => {
                    cleanup(project_dir, existed);
                    return Err(err);
                }
            }
        };
        ctx.log(format!("Project folder: {}", project_dir.display()));

        let mut setup_status = if options.install { SetupStatus::Ok } else { SetupStatus::Skipped };
        if options.install {
            for step in setup_steps {
                ctx.step(&step.label);
                ctx.log(format!("$ {}", step.run));
                if let Err(err) = shell::run_checked(&step.run, project_dir, &mut |line| ctx.log(line)) {
                    ctx.warn(format!(
                        "{} failed: {}. Fix the problem and use “Run setup again” on the project page.",
                        step.label, err.message
                    ));
                    setup_status = SetupStatus::Failed;
                    break;
                }
            }
        }

        if options.git {
            ctx.step("Initialize git");
            init_git(project_dir, &manifest.id, version, ctx);
        }

        ctx.step("Register project");
        let record = ProjectRecord {
            id: util::short_id(),
            name: materialized.name.clone(),
            slug: materialized.slug.clone(),
            path: project_dir.to_string_lossy().into_owned(),
            skeleton_id: Some(manifest.id.clone()),
            skeleton_version: Some(version.to_string()),
            features: materialized.features.clone(),
            ports: materialized.ports.clone(),
            setup_status,
            created_at: util::now_rfc3339(),
            opened_at: Some(util::now_rfc3339()),
        };
        self.registry.insert(&record)?;
        self.emit(Event::ProjectsChanged);
        Ok(json!({ "project": self.project_view(record), "setupStatus": setup_status }))
    }

    /// Reserves a slug and its folder in `parent` for one creation job. The slug names the folder,
    /// `COMPOSE_PROJECT_NAME` and other per-project resources, so it must be unique among registered
    /// projects and running creations: a taken derived slug gets a `-2`, `-3`… suffix, a taken
    /// explicit slug is an error.
    fn claim_project(self: &Arc<Self>, parent: &Path, slug: &str, explicit: bool) -> Result<(String, ProjectClaim)> {
        let mut creating = self.creating.lock().expect("creating");
        let mut used: HashSet<String> = self.registry.list()?.into_iter().map(|p| p.slug).collect();
        used.extend(creating.keys().filter_map(|dir| dir.file_name()).map(|n| n.to_string_lossy().into_owned()));
        let slug = if !used.contains(slug) {
            slug.to_string()
        } else if explicit {
            return Err(Error::conflict(format!(
                "slug \"{slug}\" is already used by another project; choose a different slug"
            )));
        } else {
            (2..)
                .map(|n| {
                    let suffix = format!("-{n}");
                    let base = slug[..slug.len().min(64 - suffix.len())].trim_end_matches(['-', '.', '_']);
                    format!("{base}{suffix}")
                })
                .find(|candidate| !used.contains(candidate))
                .expect("a free slug")
        };
        let dir = parent.join(&slug);
        generator::check_target(&dir)?;
        if self.registry.find_by_path(&dir.to_string_lossy())?.is_some() {
            return Err(Error::conflict(format!("{} is already registered", dir.display())));
        }
        if creating.contains_key(&dir) {
            return Err(Error::conflict(format!("{} is already being created", dir.display())));
        }
        creating.insert(dir.clone(), HashSet::new());
        Ok((slug, ProjectClaim { core: Arc::downgrade(self), dir }))
    }

    /// Reserves `dir` for one creation job; the claim is released when the job ends.
    #[cfg(test)]
    fn claim_project_dir(self: &Arc<Self>, dir: &Path) -> Result<ProjectClaim> {
        let mut creating = self.creating.lock().expect("creating");
        if creating.contains_key(dir) {
            return Err(Error::conflict(format!("{} is already being created", dir.display())));
        }
        creating.insert(dir.to_path_buf(), HashSet::new());
        Ok(ProjectClaim { core: Arc::downgrade(self), dir: dir.to_path_buf() })
    }

    fn import_project(&self, path: &str) -> Result<Value> {
        let path = canonical_dir(path)?;
        if self.registry.find_by_path(&path)?.is_some() {
            return Err(Error::conflict(format!("{path} is already registered")));
        }
        let dir = PathBuf::from(&path);
        let meta = ProjectMeta::load(&dir)?.unwrap_or_else(|| ProjectMeta::infer(&dir));
        let ports: indexmap::IndexMap<String, u16> = envfile::read_values(&dir.join(".env"))
            .into_iter()
            .filter(|(k, _)| k.ends_with("_PORT"))
            .filter_map(|(k, v)| v.parse().ok().map(|p| (k, p)))
            .collect();
        let record = ProjectRecord {
            id: util::short_id(),
            name: meta.project.name.clone(),
            slug: meta.project.slug.clone(),
            path,
            skeleton_id: meta.skeleton.as_ref().map(|s| s.id.clone()),
            skeleton_version: meta.skeleton.as_ref().map(|s| s.version.clone()),
            features: meta.features.clone(),
            ports,
            setup_status: SetupStatus::Ok,
            created_at: util::now_rfc3339(),
            opened_at: Some(util::now_rfc3339()),
        };
        self.registry.insert(&record)?;
        self.emit(Event::ProjectsChanged);
        Ok(self.project_view(record))
    }

    fn run_project_command(self: &Arc<Self>, a: RunArgs) -> Result<Value> {
        let record = self.registry.find(&a.project)?;
        let dir = PathBuf::from(&record.path);
        let meta = ProjectMeta::load(&dir)?.unwrap_or_else(|| ProjectMeta::infer(&dir));
        let commands = meta.active_commands();
        let command = commands.get(&a.command).ok_or_else(|| {
            Error::not_found(format!(
                "project has no command \"{}\" (available: {})",
                a.command,
                commands.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;
        command.check_inputs(&a.command, &a.inputs)?;
        let env: HashMap<String, String> = envfile::read_values(&dir.join(".env"));
        let url = command.url.as_ref().map(|u| envfile::interpolate(u, &env));
        let info = self.runner.start(&record.id, &dir, &a.command, command, url.clone(), &a.inputs)?;
        let _ = self.registry.touch(&record.id, &util::now_rfc3339());
        let run = self.runner.get(&info.id).ok_or_else(|| Error::internal("run disappeared"))?;
        let wait = a.wait_seconds.unwrap_or(if command.long { 5 } else { 300 });
        let info = run.wait(Duration::from_secs(wait));
        let output = run.tail(if command.long { 60 } else { 200 });
        Ok(json!({ "run": info, "output": output, "url": url }))
    }

    fn run_project_setup(self: &Arc<Self>, project: &str, wait: u64) -> Result<Value> {
        let record = self.registry.find(project)?;
        let dir = PathBuf::from(&record.path);
        let meta = ProjectMeta::load(&dir)?.unwrap_or_else(|| ProjectMeta::infer(&dir));
        let steps = meta.active_setup();
        if steps.is_empty() {
            return Err(Error::invalid("this project has no setup steps"));
        }
        let labels: Vec<String> = steps.iter().map(|s| s.label.clone()).collect();
        let label_refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        let core = self.clone();
        let translations = meta.translations.clone();
        let job =
            self.jobs.start_translated("run_project_setup", &record.name, &label_refs, translations, move |ctx| {
                let mut result = Ok(());
                for step in &steps {
                    ctx.step(&step.label);
                    ctx.log(format!("$ {}", step.run));
                    result = shell::run_checked(&step.run, &dir, &mut |line| ctx.log(line));
                    if result.is_err() {
                        break;
                    }
                }
                let status = if result.is_ok() { SetupStatus::Ok } else { SetupStatus::Failed };
                core.registry.set_setup_status(&record.id, status)?;
                core.emit(Event::ProjectsChanged);
                result.map(|()| json!({ "setupStatus": status }))
            });
        Ok(job_response(&job, wait, 60))
    }
}

struct SkeletonOrigin {
    repo: Option<String>,
    commit: Option<String>,
}

/// A project folder (and its ports) reserved by a creation job; released on drop.
struct ProjectClaim {
    core: std::sync::Weak<Core>,
    dir: PathBuf,
}

impl Drop for ProjectClaim {
    fn drop(&mut self) {
        if let Some(core) = self.core.upgrade() {
            core.creating.lock().expect("creating").remove(&self.dir);
        }
    }
}

#[derive(Deserialize, Default)]
struct SkeletonFilter {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Deserialize)]
struct RunArgs {
    project: String,
    command: String,
    /// Values of the command's `inputs` (environment variable → value).
    #[serde(default)]
    inputs: BTreeMap<String, String>,
    #[serde(default)]
    wait_seconds: Option<u64>,
}

fn job_response(job: &Arc<Job>, wait: u64, log_lines: usize) -> Value {
    let snapshot = job.wait(Duration::from_secs(wait.min(600)));
    let mut value = serde_json::to_value(&snapshot).unwrap_or_default();
    value["jobId"] = json!(snapshot.id);
    let (start, lines) = job.log_tail(log_lines);
    value["logStart"] = json!(start);
    value["log"] = json!(lines);
    value
}

/// Creates the project folder, or takes an existing empty one. Returns whether it existed.
fn take_project_dir(dir: &Path) -> Result<bool> {
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent).at(parent)?;
    }
    match fs::create_dir(dir) {
        Ok(()) => Ok(false),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            generator::check_target(dir)?;
            Ok(true)
        }
        Err(err) => Err(Error::io(dir, err)),
    }
}

fn cleanup(dir: &Path, existed: bool) {
    if existed {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let _ = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
            }
        }
    } else {
        let _ = fs::remove_dir_all(dir);
    }
}

fn init_git(dir: &Path, skeleton: &str, version: &str, ctx: &JobCtx) {
    let run = |args: &[&str]| -> Result<String> {
        let (code, out) = shell::capture("git", args, dir)?;
        if code == 0 { Ok(out) } else { Err(Error::command(out)) }
    };
    if let Err(err) = run(&["init", "-b", "main"]) {
        ctx.warn(format!("git init failed: {}", err.message));
        return;
    }
    ctx.log("Initialized empty Git repository (branch main)");
    if let Err(err) = run(&["add", "-A"]) {
        ctx.warn(format!("git add failed: {}", err.message));
        return;
    }
    let has_identity = run(&["config", "user.email"]).is_ok_and(|v| !v.is_empty())
        && run(&["config", "user.name"]).is_ok_and(|v| !v.is_empty());
    let message = format!("Initial commit from fastDev skeleton {skeleton} {version}");
    match run(&["commit", "-q", "-m", &message]) {
        Ok(_) => {
            ctx.log(format!("Committed: {message}"));
            if !has_identity {
                ctx.warn(
                    "git user.name/user.email are not configured; git picked an automatic author for the first commit",
                );
            }
        }
        Err(err) => ctx.warn(format!("git commit failed: {}", err.message)),
    }
}

/// Refuses to delete folders that cannot be a project: the root, the home folder,
/// its ancestors and top-level folders like `~/Documents`.
fn ensure_deletable(path: &Path) -> Result<()> {
    let path = fs::canonicalize(path).at(path)?;
    let home = fs::canonicalize(util::home_dir()).unwrap_or_else(|_| util::home_dir());
    let too_shallow = path.components().count() < 4;
    let protected = home.starts_with(&path) || path.parent() == Some(home.as_path());
    if too_shallow || protected {
        return Err(Error::not_allowed(format!(
            "refusing to delete {}: it does not look like a project folder",
            path.display()
        )));
    }
    Ok(())
}

fn canonical_dir(path: &str) -> Result<String> {
    let path = util::expand_home(path.trim());
    let canonical =
        fs::canonicalize(&path).map_err(|_| Error::not_found(format!("{} does not exist", path.display())))?;
    if !canonical.is_dir() {
        return Err(Error::invalid(format!("{} is not a folder", canonical.display())));
    }
    Ok(canonical.to_string_lossy().into_owned())
}

fn terminal_app(terminal: Terminal) -> &'static str {
    match terminal {
        Terminal::Terminal => "Terminal",
        Terminal::Iterm => "iTerm",
        Terminal::Warp => "Warp",
        Terminal::Ghostty => "Ghostty",
    }
}

fn open_path(target: &str) -> Result<()> {
    let output = Process::new("/usr/bin/open").arg(target).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::command(String::from_utf8_lossy(&output.stderr).trim().to_string()))
    }
}

fn open_app(app: &str, path: &str) -> Result<()> {
    let output = Process::new("/usr/bin/open").args(["-a", app, path]).output()?;
    if output.status.success() {
        Ok(())
    } else {
        Err(Error::command(format!("cannot open {app}: {}", String::from_utf8_lossy(&output.stderr).trim())))
    }
}

fn open_in_editor(settings: &Settings, path: &str) -> Result<()> {
    match settings.editor {
        Editor::Vscode => open_app("Visual Studio Code", path),
        Editor::Cursor => open_app("Cursor", path),
        Editor::Zed => open_app("Zed", path),
        Editor::Custom => {
            let template = settings.editor_command.trim();
            if template.is_empty() {
                return Err(Error::invalid("set the custom editor command in Settings"));
            }
            let quoted = format!("'{}'", path.replace('\'', "'\\''"));
            let script = if template.contains("{path}") {
                template.replace("{path}", &quoted)
            } else {
                format!("{template} {quoted}")
            };
            // Spawned, not awaited: terminal editors would block otherwise.
            Process::new("/bin/sh")
                .args(["-c", &script])
                .env("PATH", toolchain::login_path())
                .spawn()
                .map_err(|err| Error::command(format!("`{script}` failed: {err}")))?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_core(root: &Path) -> Arc<Core> {
        let config = CoreConfig {
            data_dir: root.join("data"),
            app_version: "test".into(),
            bridge_path: None,
            workspace_override: Some(root.join("workspace")),
            registries_override: Some(Vec::new()),
        };
        Core::new(config, Arc::new(crate::events::NullSink)).unwrap()
    }

    #[test]
    fn concurrent_creations_keep_their_folders_and_ports_apart() {
        let tmp = tempfile::tempdir().unwrap();
        let skeleton = tmp.path().join("workspace/sample");
        fs::create_dir_all(skeleton.join(FILES_DIR)).unwrap();
        fs::write(
            skeleton.join("template.toml"),
            "schema = 1\nid = \"sample\"\nname = \"Sample\"\n\n[ports]\nAPP_PORT = 47310\n\n\
             [[setup]]\nlabel = \"Wait\"\nrun = \"sleep 1\"\n",
        )
        .unwrap();
        fs::write(skeleton.join("files/README.md"), "# Sample\n").unwrap();
        let core = test_core(tmp.path());
        let parent = tmp.path().join("projects");
        let create = |slug: &str| {
            let args = json!({ "skeleton": "sample", "version": "draft", "name": slug, "slug": slug,
                               "parent_dir": parent, "git": false, "wait_seconds": 0 });
            core.call("create_project", args, Caller::Cli)
        };
        let finish = |job: &Value| {
            core.call("get_job", json!({ "job_id": job["jobId"], "wait_seconds": 60 }), Caller::Cli).unwrap()
        };

        // A folder is reserved for the job that creates it: a second job never writes (or cleans) it.
        let first = create("one").unwrap();
        assert_eq!(create("one").unwrap_err().code, crate::error::ErrorCode::Conflict);
        // Ports count as taken before the first project is registered (its setup still runs).
        let second = create("two").unwrap();
        let (first, second) = (finish(&first), finish(&second));
        assert_eq!((first["status"].as_str(), second["status"].as_str()), (Some("succeeded"), Some("succeeded")));
        let port = |job: &Value| job["result"]["project"]["ports"]["APP_PORT"].as_u64().unwrap();
        assert_ne!(port(&first), port(&second));
        assert!(parent.join("one/README.md").is_file() && parent.join("two/README.md").is_file());
        assert!(core.creating.lock().unwrap().is_empty());

        // Claims are released when the job ends.
        let claim = core.claim_project_dir(&parent.join("three")).unwrap();
        assert!(core.claim_project_dir(&parent.join("three")).is_err());
        drop(claim);
        assert!(core.claim_project_dir(&parent.join("three")).is_ok());

        // Slugs stay unique across folders: a derived slug gets a suffix, an explicit one is refused.
        let elsewhere = tmp.path().join("elsewhere");
        let args = |extra: Value| {
            let mut args = json!({ "skeleton": "sample", "version": "draft", "name": "One", "parent_dir": elsewhere,
                                   "git": false, "install": false, "wait_seconds": 60 });
            args.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
            args
        };
        let job = core.call("create_project", args(json!({})), Caller::Cli).unwrap();
        assert_eq!(job["result"]["project"]["slug"], "one-2");
        assert!(elsewhere.join("one-2/README.md").is_file());
        let err = core.call("create_project", args(json!({ "slug": "two" })), Caller::Cli).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::Conflict);
    }

    #[test]
    fn rejects_skeleton_ids_that_are_paths() {
        assert!(check_skeleton_ids(&json!({ "id": "node-vue", "source": "", "project": "/any/path" })).is_ok());
        for bad in ["/Users/someone/project", "../outside", "a/b", "", "Node"] {
            assert!(check_skeleton_ids(&json!({ "id": bad })).is_err(), "{bad:?}");
            assert!(check_skeleton_ids(&json!({ "skeleton": bad })).is_err(), "{bad:?}");
        }
        assert!(check_skeleton_ids(&json!({ "source": "../outside" })).is_err());
    }

    #[test]
    fn protects_important_folders() {
        let home = util::home_dir();
        assert!(ensure_deletable(Path::new("/")).is_err());
        assert!(ensure_deletable(&home).is_err());
        if home.join("Documents").is_dir() {
            assert!(ensure_deletable(&home.join("Documents")).is_err());
        }
        let project = tempfile::tempdir_in(std::env::temp_dir()).unwrap();
        let nested = project.path().join("a/b");
        fs::create_dir_all(&nested).unwrap();
        assert!(ensure_deletable(&nested).is_ok());
    }
}
