# fastDev MCP server

fastDev has no built-in AI. Agents (Claude Code, Codex, any MCP client) work with it through the `fastdev-mcp` server.

```
MCP client ──stdio JSON-RPC──▶ fastdev-mcp ──Unix socket──▶ running fastDev app
```

- The app owns the library, the project registry, jobs and all project processes. While it runs it listens on `~/Library/Application Support/fastDev/fastdev.sock` (user-only permissions, no network port).
- `fastdev-mcp` always lists every tool and forwards each call to the socket.
- When the app is closed, every call returns: *fastDev is not running. Ask the user to start the fastDev app, then retry.* The bridge never starts the app; agents must not try to.
- Changes made through MCP appear in the app window immediately.

## Connecting

The bridge is inside the app bundle: `/Applications/fastDev.app/Contents/MacOS/fastdev-mcp` (in development: `target/debug/fastdev-mcp`). Settings → MCP shows the exact commands with copy buttons.

Claude Code (user scope, available in every project):

```bash
claude mcp add --scope user fastdev -- "/Applications/fastDev.app/Contents/MacOS/fastdev-mcp"
```

Codex (`~/.codex/config.toml`):

```toml
[mcp_servers.fastdev]
command = "/Applications/fastDev.app/Contents/MacOS/fastdev-mcp"
```

Check the connection from a terminal: `fastdev-mcp --check` prints `fastdev_status` of the running app.

## Conventions

- Arguments are `snake_case`; results are JSON objects with `camelCase` keys (the raw manifest keeps `template.toml` keys).
- Projects are referenced by id, slug or absolute path (`project`).
- Skeleton targets are `"draft"` or a version like `"1.2.0"`; by default the draft when it exists, otherwise the latest version.
- Errors come back as tool errors `"[code] message"`; codes: `not_found`, `invalid_input`, `conflict`, `validation`, `toolchain`, `command_failed`, `not_allowed`, `io`, `internal`.

### Jobs

Long operations (`create_project`, `run_project_setup`, `verify_skeleton`, `check_skeleton_updates`, `apply_skeleton_updates`) run as jobs. The tool waits up to `wait_seconds` and returns the job:

```json
{ "jobId": "job-…", "status": "running|succeeded|failed", "title": "…", "steps": [{ "label": "Copy files", "status": "done" }],
  "result": { … }, "error": { "code": "…", "message": "…" }, "warnings": [], "logStart": 0, "log": ["…"] }
```

If `status` is still `running`, call `get_job { job_id, wait_seconds }` until it finishes.

## Tools

### General

| Tool | Arguments | Result |
|---|---|---|
| `fastdev_status` | — | `appVersion`, `registries`, `workspacePath`, `projectsDir`, `toolchain[]`, `runningProcesses[]`, `projects` |
| `get_job` | `job_id`, `wait_seconds?`, `log_lines?` | job (see above) |

### Library

| Tool | Arguments | Result |
|---|---|---|
| `list_skeletons` | `query?`, `language?`, `category?` | `skeletons[]`: id, name, description, category, languages, tags, stack, versions (downloaded, newest first), latest, draft, forkedFrom, source (`registry`\|`custom`\|`workspace`), repo, downloaded, workspacePath, translations (of name and description), favorite |
| `get_skeleton` | `id`, `version?` | downloads the repository if needed; summary, target, manifest, requirements (checked locally), featureRequirements, changelog, validation, draftVerified, parent (for forks: newer parent versions), paths (repo, workspace, target, files) |
| `preview_skeleton` | `id`, `version?` (or `"draft"`), `features?`, `choices?`, `wait_seconds?` | job; `result.preview`: url, path, message (what to do next), dir, translations; `result.run`. The copy is reused while the skeleton content and selection are unchanged; an edited draft gets a fresh copy |
| `stop_preview` | `id` | stops the preview command |
| `get_authoring_guide` | — | `guide` (docs/skeleton-authoring.md), `manifestReference` (docs/manifest.md), `workspacePath`, `registries` |
| `sync_library` | — | pulls registries and fetches downloaded skeletons: `registries[]`, `skeletons[]` with `ok`/`error` |

### Projects

| Tool | Arguments | Result |
|---|---|---|
| `list_projects` | — | `projects[]` |
| `get_project` | `project` | project: id, name, slug, path, skeletonId/Version, features, choices, ports, setupStatus, exists, commands[] (key, label, run, long, primary, url, inputs, lastRun), setup[], running[], latestVersion, updateAvailable, translations |
| `create_project` | `skeleton`, `name`, `version?` (or `"draft"` to test a skeleton draft end to end), `slug?`, `parent_dir?`, `git?`, `install?`, `features?` (`{name: bool}`), `choices?` (`{name: option}`, e.g. `{"docker": "full", "data": "local"}`), `agents_md?`, `spec_md?`, `claude_md?`, `brief?`, `wait_seconds?` | job; `result.project`, `result.setupStatus` |
| `import_project` | `path` | project |
| `remove_project` | `project` | `{ removed, path }` — files are never deleted |
| `run_project_command` | `project`, `command`, `inputs?` (`{NAME: value}` for commands with inputs), `wait_seconds?` | `run` (status, exitCode), `output[]`, `url` |
| `stop_project_command` | `project`, `command` | `run` |
| `get_project_logs` | `project`, `command?`, `lines?` | `runs[]` with `run`, `start`, `lines[]` |
| `run_project_setup` | `project`, `wait_seconds?` | job |

`run_project_command` waits for short commands (default up to 300 s) and returns their output; for long ones (`long = true`) it returns after `wait_seconds` (default 5) with the first output while the process keeps running. Deleting projects from disk and changing settings are only possible in the app window.

### Skeleton authoring

| Tool | Arguments | Result |
|---|---|---|
| `create_skeleton_draft` | `mode` (`new`\|`edit`\|`fork`), `id`, `source?` (fork), `from_version?` (fork), `name?`, `description?` | `draft` info (path of the workspace clone), `manifestPath`, `filesPath` |
| `validate_skeleton` | `id`, `target?` | `valid`, `errors[]`, `warnings[]` (file + message) |
| `verify_skeleton` | `id`, `target?`, `wait_seconds?` | job; stores the result in the draft |
| `publish_skeleton` | `id`, `changes[]`, `bump?` or `version?`, `allow_unverified?` | `version`, `tag`, `commit`, `path`, `verified`, `pushed`, `registry`, `warnings` |
| `discard_skeleton_draft` | `id` | `{ discarded }` — resets the clone to the latest tag, or deletes an unpublished skeleton; only a folder directly inside the workspace is touched |
| `check_skeleton_updates` | `id`, `target?`, `wait_seconds?` | job; `result.packages[]`: name, workspace, dependencyType, current, wanted, latest, level |
| `apply_skeleton_updates` | `id`, `packages?` (`{name, workspace?, version?}`), `level?`, `wait_seconds?` | job; `result.updated[]`, `result.files[]` |

## Typical sessions

**"Create a Node + Vue project called Coffee Shop, everything in Docker, data in the project folder; the idea is …"**

1. `list_skeletons { query: "vue" }` → `node-vue`, latest `1.0.0`.
2. `create_project { skeleton: "node-vue", name: "Coffee Shop", choices: { docker: "full", data: "local" }, brief: "…" }` → wait for the job. The available choices and options are in `get_skeleton` → `manifest.choices`.
3. Tell the owner the path; optionally `run_project_command { project, command: "dev" }` and give the URL.

**"Show me the node-vue skeleton"**

1. `preview_skeleton { id: "node-vue" }` → wait for the job.
2. Give the owner `result.preview.url` (or `path`) and `result.preview.message`; `stop_preview` when they are done.

**"Update node-vue dependencies"**

1. `check_skeleton_updates { id: "node-vue" }` → review levels.
2. `apply_skeleton_updates { id: "node-vue", level: "minor" }`.
3. `verify_skeleton { id: "node-vue", target: "draft" }`.
4. `publish_skeleton { id: "node-vue", bump: "minor", changes: ["Vue 3.5 → 3.6", "…"] }`.

**"Create a Laravel project with React, Bootstrap and PostgreSQL"**

1. `get_skeleton { id: "laravel-inertia" }` → `manifest.choices` lists `frontend`, `css`, `database`, `docker`, `data` and their options.
2. `create_project { skeleton: "laravel-inertia", name: "…", choices: { frontend: "react", css: "bootstrap", database: "postgres" } }`.

**"Create a Rust desktop skeleton"** — `get_authoring_guide`, then follow its §4.1.
