//! `fastdev-cli` — headless access to the fastDev API for agents and CI.
//!
//! It runs the same code as the app (`fastdev_core::Core`) against the same data folder,
//! so projects created here appear in the app. Long-running project processes are only
//! managed by the app.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use fastdev_core::events::{Event, EventSink};
use fastdev_core::{Core, CoreConfig};
use fastdev_protocol::Caller;
use serde_json::{Value, json};

const USAGE: &str = "fastdev-cli — headless fastDev commands

Usage: fastdev-cli [--workspace <dir>] [--registry <url-or-dir>]… <command> [options]

Library:
  skeletons                              List skeletons (registries, custom sources, workspace)
  sync                                   Pull registries and downloaded skeleton repositories
  registry-init <dir> [--name <name>]    Create a local registry repository (index.json)
  add-source <url>                       Add a skeleton repository by URL
  show <id> [--target draft|X.Y.Z]       Skeleton details (JSON)
  validate <id> [--target …] | --all     Validate a draft or version (all: every skeleton)
  verify <id> [--target …]               Throwaway project + setup + verify commands
  draft new|edit|fork <id> [--source <id>] [--from X.Y.Z] [--name <name>] [--description <text>]
  publish <id> (--bump patch|minor|major | --version X.Y.Z) --change <text>… [--allow-unverified]
         [--replace]   development only: re-publish the latest version while its tag is not pushed
  discard <id>                           Delete the draft
  outdated <id> [--target …]             List outdated dependencies
  update <id> (--level patch|minor|major | --package <name>…)

  preview <id> [--version X.Y.Z|draft] [--choice <name>=<option>]… [--for <seconds>]
                                         Run the skeleton preview and stream its output; a long-running
                                         preview stops on Ctrl+C, after --for seconds, or (without
                                         --for) when stdin closes (Ctrl+D)

Projects:
  projects                               List registered projects
  remove <project>                       Unregister a project (id, slug or path); files stay on disk
  create <skeleton> --name <name> [--dir <parent>] [--version X.Y.Z|draft] [--slug <slug>]
         [--feature <name>]… [--choice <name>=<option>]… [--no-git] [--no-install]
         [--no-agents] [--no-spec] [--no-claude] [--brief <text>]

Generic:
  call <method> [<json-arguments>]       Call any API method

Environment: FASTDEV_HOME (data folder), FASTDEV_WORKSPACE, FASTDEV_REGISTRY.";

/// Prints job and process output to stderr while commands run.
struct StderrSink;

impl EventSink for StderrSink {
    fn emit(&self, event: Event) {
        let (prefix, lines) = match event {
            Event::JobOutput { lines, .. } => ("  ", lines),
            Event::RunOutput { lines, .. } => ("│ ", lines),
            _ => return,
        };
        let mut err = std::io::stderr();
        for line in lines {
            let _ = writeln!(err, "{prefix}{line}");
        }
    }
}

/// Set by SIGINT/SIGTERM/SIGHUP: processes started by the runner live in their own process
/// groups, so the CLI must stop them itself instead of dying with the terminal's Ctrl+C.
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_: nix::libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

#[allow(unsafe_code)]
fn trap_signals() {
    use nix::sys::signal::{SigHandler, Signal, signal};
    for sig in [Signal::SIGINT, Signal::SIGTERM, Signal::SIGHUP] {
        // SAFETY: the handler only stores into an atomic, which is async-signal-safe.
        let _ = unsafe { signal(sig, SigHandler::Handler(on_signal)) };
    }
}

fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::SeqCst)
}

struct Args {
    items: Vec<String>,
}

impl Args {
    fn flag(&mut self, name: &str) -> bool {
        match self.items.iter().position(|a| a == name) {
            Some(pos) => {
                self.items.remove(pos);
                true
            }
            None => false,
        }
    }

    fn value(&mut self, name: &str) -> Option<String> {
        let pos = self.items.iter().position(|a| a == name)?;
        if pos + 1 >= self.items.len() {
            fail(&format!("{name} needs a value"));
        }
        let value = self.items.remove(pos + 1);
        self.items.remove(pos);
        Some(value)
    }

    fn values(&mut self, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        while let Some(v) = self.value(name) {
            out.push(v);
        }
        out
    }

    fn positional(&mut self) -> Option<String> {
        if self.items.is_empty() { None } else { Some(self.items.remove(0)) }
    }

    fn require(&mut self, what: &str) -> String {
        self.positional().unwrap_or_else(|| fail(&format!("missing {what}")))
    }
}

/// The lines of [`USAGE`] that describe one command (its line and the indented continuation).
fn command_usage(command: &str) -> Option<String> {
    let lines: Vec<&str> = USAGE.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.strip_prefix("  ").is_some_and(|rest| rest.split_whitespace().next() == Some(command)))?;
    let mut out = vec![format!("Usage: fastdev-cli {}", lines[start].trim())];
    out.extend(
        lines[start + 1..]
            .iter()
            .take_while(|l| l.starts_with("         ") && !l.trim().is_empty())
            .map(|l| format!("  {}", l.trim())),
    );
    Some(out.join("\n"))
}

fn fail(message: &str) -> ! {
    eprintln!("error: {message}\n\nRun `fastdev-cli --help` for usage.");
    std::process::exit(2);
}

fn main() {
    let mut args = Args { items: std::env::args().skip(1).collect() };
    // Global options first, so `--workspace <dir> draft --help` finds the command.
    let workspace = args.value("--workspace").or_else(|| std::env::var("FASTDEV_WORKSPACE").ok()).map(PathBuf::from);
    let mut registries = args.values("--registry");
    if args.items.is_empty() || args.flag("--help") || args.flag("-h") {
        let command = args.items.iter().find(|a| !a.starts_with('-')).cloned();
        println!("{}", command.and_then(|c| command_usage(&c)).unwrap_or_else(|| USAGE.to_string()));
        return;
    }
    if registries.is_empty()
        && let Ok(value) = std::env::var("FASTDEV_REGISTRY")
    {
        registries.push(value);
    }
    let absolute = |path: String| -> String {
        let p = PathBuf::from(&path);
        if p.is_relative() && p.exists() {
            p.canonicalize().map(|c| c.to_string_lossy().into_owned()).unwrap_or(path)
        } else {
            path
        }
    };
    let config = CoreConfig {
        data_dir: fastdev_protocol::data_dir(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        bridge_path: None,
        workspace_override: workspace.map(|p| p.canonicalize().unwrap_or(p)),
        registries_override: (!registries.is_empty()).then(|| registries.into_iter().map(absolute).collect()),
    };
    let core = Core::new(config, Arc::new(StderrSink)).unwrap_or_else(|err| fail(&err.message));
    let command = args.require("command");

    let result = match command.as_str() {
        "skeletons" => call(&core, "list_skeletons", json!({})).map(|v| {
            for s in v["skeletons"].as_array().into_iter().flatten() {
                println!(
                    "{:<24} {:<10} {:<28} {:<10} {}",
                    s["id"].as_str().unwrap_or_default(),
                    s["latest"].as_str().unwrap_or("-"),
                    s["name"].as_str().unwrap_or_default(),
                    s["source"].as_str().unwrap_or_default(),
                    [
                        if s["downloaded"].as_bool() == Some(true) { "" } else { "not downloaded" },
                        if s["draft"].is_null() { "" } else { "draft" },
                    ]
                    .iter()
                    .filter(|t| !t.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(", ")
                );
            }
            Value::Null
        }),
        "sync" => call(&core, "sync_library", json!({})),
        "preview" => {
            let id = args.require("skeleton id");
            let choices = parse_choices(args.values("--choice"));
            let seconds =
                args.value("--for").map(|v| v.parse::<u64>().unwrap_or_else(|_| fail("--for expects seconds")));
            let version = args.value("--version");
            let result = job(&core, "preview_skeleton", json!({ "id": id, "version": version, "choices": choices }))
                .map(|result| {
                    let preview = &result["preview"];
                    eprintln!();
                    for key in ["url", "path", "message"] {
                        if let Some(value) = preview[key].as_str().filter(|v| !v.is_empty()) {
                            eprintln!("{key}: {value}");
                        }
                    }
                    if result["run"]["status"] == "running" {
                        wait_for_preview(&core, &id, seconds);
                    }
                    Value::Null
                });
            let _ = call(&core, "stop_preview", json!({ "id": id }));
            if interrupted() {
                eprintln!("Preview stopped.");
                std::process::exit(130);
            }
            result
        }
        "add-source" => {
            let url = args.require("repository URL");
            call(&core, "add_skeleton_source", json!({ "url": url }))
        }
        "registry-init" => {
            let dir = PathBuf::from(args.require("folder"));
            let name = args.value("--name").unwrap_or_else(|| "fastDev skeletons".into());
            fastdev_core::index::init(&dir, &name).map(|()| json!({ "registry": dir })).map_err(|e| e.message)
        }
        "show" => {
            let id = args.require("skeleton id");
            call(&core, "get_skeleton", json!({ "id": id, "target": args.value("--target") }))
        }
        "validate" => {
            if args.flag("--all") {
                validate_all(&core)
            } else {
                let id = args.require("skeleton id");
                call(&core, "validate_skeleton", json!({ "id": id, "target": args.value("--target") })).and_then(|v| {
                    if v["valid"].as_bool() == Some(true) {
                        Ok(v)
                    } else {
                        Err(serde_json::to_string_pretty(&v).unwrap_or_default())
                    }
                })
            }
        }
        "verify" => {
            let id = args.require("skeleton id");
            job(&core, "verify_skeleton", json!({ "id": id, "target": args.value("--target") }))
        }
        "draft" => {
            let mode = args.require("mode (new, edit, fork)");
            let id = args.require("skeleton id");
            call(
                &core,
                "create_skeleton_draft",
                json!({
                    "mode": mode,
                    "id": id,
                    "source": args.value("--source"),
                    "from_version": args.value("--from"),
                    "name": args.value("--name"),
                    "description": args.value("--description"),
                }),
            )
        }
        "publish" => {
            let id = args.require("skeleton id");
            call(
                &core,
                "publish_skeleton",
                json!({
                    "id": id,
                    "bump": args.value("--bump"),
                    "version": args.value("--version"),
                    "changes": args.values("--change"),
                    "allow_unverified": args.flag("--allow-unverified"),
                    "replace": args.flag("--replace"),
                }),
            )
        }
        "discard" => {
            let id = args.require("skeleton id");
            call(&core, "discard_skeleton_draft", json!({ "id": id }))
        }
        "outdated" => {
            let id = args.require("skeleton id");
            job(&core, "check_skeleton_updates", json!({ "id": id, "target": args.value("--target") }))
        }
        "update" => {
            let id = args.require("skeleton id");
            let packages: Vec<Value> =
                args.values("--package").into_iter().map(|name| json!({ "name": name })).collect();
            job(
                &core,
                "apply_skeleton_updates",
                json!({ "id": id, "packages": packages, "level": args.value("--level") }),
            )
        }
        "remove" => {
            let project = args.require("project (id, slug or path)");
            call(&core, "remove_project", json!({ "project": project }))
        }
        "projects" => call(&core, "list_projects", json!({})).map(|v| {
            for p in v["projects"].as_array().into_iter().flatten() {
                println!(
                    "{:<14} {:<24} {:<18} {}",
                    p["id"].as_str().unwrap_or_default(),
                    p["name"].as_str().unwrap_or_default(),
                    format!(
                        "{} {}",
                        p["skeletonId"].as_str().unwrap_or("-"),
                        p["skeletonVersion"].as_str().unwrap_or("")
                    ),
                    p["path"].as_str().unwrap_or_default()
                );
            }
            Value::Null
        }),
        "create" => {
            let skeleton = args.require("skeleton id");
            let features: BTreeMap<String, bool> = args.values("--feature").into_iter().map(|f| (f, true)).collect();
            let choices = parse_choices(args.values("--choice"));
            job(
                &core,
                "create_project",
                json!({
                    "skeleton": skeleton,
                    "name": args.value("--name").unwrap_or_else(|| fail("--name is required")),
                    "slug": args.value("--slug"),
                    "version": args.value("--version"),
                    "parent_dir": args.value("--dir"),
                    "features": features,
                    "choices": choices,
                    "git": !args.flag("--no-git"),
                    "install": !args.flag("--no-install"),
                    "agents_md": !args.flag("--no-agents"),
                    "spec_md": !args.flag("--no-spec"),
                    "claude_md": !args.flag("--no-claude"),
                    "brief": args.value("--brief"),
                }),
            )
        }
        "call" => {
            let method = args.require("method");
            let arguments = args
                .positional()
                .map(|text| serde_json::from_str(&text).unwrap_or_else(|err| fail(&format!("invalid JSON: {err}"))))
                .unwrap_or_else(|| json!({}));
            core.call(&method, arguments, Caller::Cli).map_err(|err| format!("[{}] {}", err.code.as_str(), err.message))
        }
        other => fail(&format!("unknown command \"{other}\"")),
    };
    if let Some(extra) = args.positional() {
        fail(&format!("unexpected argument \"{extra}\""));
    }

    match result {
        Ok(Value::Null) => {}
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value).unwrap_or_default()),
        Err(message) => {
            eprintln!("error: {message}");
            std::process::exit(if interrupted() { 130 } else { 1 });
        }
    }
}

fn parse_choices(pairs: Vec<String>) -> BTreeMap<String, String> {
    pairs
        .into_iter()
        .map(|pair| match pair.split_once('=') {
            Some((name, option)) => (name.to_string(), option.to_string()),
            None => fail(&format!("--choice expects name=option, got \"{pair}\"")),
        })
        .collect()
}

fn call(core: &Arc<Core>, method: &str, arguments: Value) -> Result<Value, String> {
    core.call(method, arguments, Caller::Cli).map_err(|err| err.message)
}

/// Starts a job and waits until it finishes; fails when the job fails or on Ctrl+C, after stopping
/// the processes the job started (they live in their own process groups).
fn job(core: &Arc<Core>, method: &str, mut arguments: Value) -> Result<Value, String> {
    trap_signals();
    // Short waits, so an interrupt is noticed within a second.
    arguments["wait_seconds"] = json!(1);
    let mut value = call(core, method, arguments)?;
    while value["status"] == "running" {
        if interrupted() {
            core.stop_all_runs();
            return Err("interrupted".into());
        }
        let id = value["jobId"].as_str().unwrap_or_default().to_string();
        value = call(core, "get_job", json!({ "job_id": id, "wait_seconds": 1, "log_lines": 0 }))?;
    }
    std::thread::sleep(Duration::from_millis(150));
    if let Some(warnings) = value["warnings"].as_array() {
        for warning in warnings {
            eprintln!("warning: {}", warning.as_str().unwrap_or_default());
        }
    }
    if value["status"] == "failed" {
        return Err(value["error"]["message"].as_str().unwrap_or("job failed").to_string());
    }
    Ok(value["result"].clone())
}

/// Keeps a long-running preview alive (its output streams to stderr) until `--for` elapses,
/// Ctrl+C, stdin closes or the process exits by itself.
fn wait_for_preview(core: &Arc<Core>, id: &str, seconds: Option<u64>) {
    let stdin_closed = Arc::new(AtomicBool::new(false));
    match seconds {
        Some(s) => eprintln!("Preview is running for {s} s. Press Ctrl+C to stop it."),
        None => {
            eprintln!("Preview is running. Press Ctrl+C (or Ctrl+D) to stop it.");
            let flag = stdin_closed.clone();
            std::thread::spawn(move || {
                let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut Vec::new());
                flag.store(true, Ordering::SeqCst);
            });
        }
    }
    let deadline = seconds.map(|s| Instant::now() + Duration::from_secs(s));
    let mut checked = Instant::now();
    loop {
        if interrupted() || stdin_closed.load(Ordering::SeqCst) || deadline.is_some_and(|d| Instant::now() >= d) {
            return;
        }
        if checked.elapsed() >= Duration::from_secs(1) {
            checked = Instant::now();
            let state = call(core, "get_preview", json!({ "id": id })).unwrap_or_default();
            if state["run"]["status"] != "running" {
                eprintln!("The preview process exited ({}).", state["run"]["status"].as_str().unwrap_or("unknown"));
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn validate_all(core: &Arc<Core>) -> Result<Value, String> {
    let list = call(core, "list_skeletons", json!({}))?;
    let mut failed = Vec::new();
    for skeleton in list["skeletons"].as_array().into_iter().flatten() {
        let id = skeleton["id"].as_str().unwrap_or_default();
        let mut targets: Vec<String> = skeleton["versions"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        // Not downloaded yet: validate the latest version (this downloads it).
        if targets.is_empty()
            && let Some(latest) = skeleton["latest"].as_str()
        {
            targets.push(latest.to_string());
        }
        if !skeleton["draft"].is_null() {
            targets.push("draft".into());
        }
        for target in targets {
            let report = call(core, "validate_skeleton", json!({ "id": id, "target": target }))?;
            let ok = report["valid"].as_bool() == Some(true);
            println!("{} {id} {target}", if ok { "✓" } else { "✗" });
            for issue in report["errors"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(report["warnings"].as_array().into_iter().flatten())
            {
                println!(
                    "    {}: {}{}",
                    issue["level"].as_str().unwrap_or_default(),
                    issue["file"].as_str().map(|f| format!("{f}: ")).unwrap_or_default(),
                    issue["message"].as_str().unwrap_or_default()
                );
            }
            if !ok {
                failed.push(format!("{id} {target}"));
            }
        }
    }
    if failed.is_empty() { Ok(Value::Null) } else { Err(format!("invalid: {}", failed.join(", "))) }
}
