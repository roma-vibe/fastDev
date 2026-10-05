# Architecture

## Processes

```
┌────────────── fastDev.app (one process) ───────────────┐
│  Webview (Vue)  ──invoke "call"──▶  src-tauri          │
│        ▲                               │                │
│        └──── "fastdev://event" ◀──┐    ▼                │
│                                   fastdev_core::Core    │◀── Unix socket ◀── fastdev-mcp ◀── MCP clients
│                                     ├ Library (files)   │
│                                     ├ Registry (SQLite) │
│                                     ├ JobManager        │
│                                     └ Runner ──▶ project processes (own process groups)
└─────────────────────────────────────────────────────────┘
fastdev-cli ──▶ its own Core on the same data folder (no long-running processes)
```

## Crates

| Crate | Role |
|---|---|
| `crates/fastdev-protocol` | Data folder and socket paths, control-socket messages, **MCP tool definitions** (name, description, JSON Schema). Tiny, shared by the app and the bridge. |
| `crates/fastdev-core` | Everything except the UI. `api.rs` is the single API (`Core::call(method, args, caller)`) used by the UI, the control socket (MCP) and the CLI. |
| `crates/fastdev-mcp` | stdio MCP server (JSON-RPC 2.0, one message per line). Lists tools from the protocol crate, forwards calls to the socket, never starts the app. |
| `crates/fastdev-cli` | Headless commands (`validate`, `verify`, `publish`, `create`, `call` …) on a `Core` with the same data folder. |
| `src-tauri` | Window (overlay title bar, Liquid Glass via `tauri-plugin-liquid-glass`), menu bar icon, lifecycle (hide on close, confirm quit with running processes), `call` command, event forwarding. |

## Core modules

| Module | Responsibility |
|---|---|
| `api` | Dispatch of every method; MCP callers are limited to the protocol tool list; UI-only methods (settings, delete from disk, open in apps) |
| `library` | Catalog (registries + sources + workspace), mirrors and version exports, drafts in workspace clones, changelog, validation |
| `index` | Registries: `index.json` model, local/remote sources, sync, upsert on publish |
| `git` | Thin wrapper over the git CLI (mirror, fetch, tags, archive/export, status) |
| `manifest` | `template.toml` model (strict serde) |
| `generator` | Materialising a project: choices/feature filtering, conditional files (`path@choice=option`), `.tmpl` rendering, `_gitignore`, `[[set]]`, ports, `.env`, `CLAUDE.md`, `.fastdev.toml`, `.fastdev.lock` |
| `authoring` | Drafts (new/edit/fork as git clones), verification, publishing (changelog, commit, tag, push, registry update), version bumps |
| `updates` | Dependency update adapters (npm) |
| `registry` | SQLite registry of projects (`PRAGMA user_version` migrations) |
| `runner` | Project processes: `/bin/sh -c` in a new process group, output ring buffer with absolute line indexes, SIGTERM → SIGKILL of the whole group (stop returns once it is empty) |
| `jobs` | Background jobs with steps, warnings, log and result; `wait` for MCP |
| `events` | `Event` enum and `OutputHub` (batches output lines every 80 ms) |
| `toolchain` | Login-shell PATH, tool versions, requirement checks, sanitised child environment |
| `shell` | Running commands (streaming, capture); tracks the process groups of job commands so quitting stops them |
| `envfile`, `edits`, `render`, `project_meta`, `settings`, `util` | Helpers |
| `control` | Unix-socket server (one JSON request per line) |

## Data

- `~/Library/Application Support/fastDev/` (`FASTDEV_HOME` overrides): `settings.json`, `fastdev.db`, `fastdev.sock`.
- Skeletons: one git repository each (tags = versions); registries (`settings.registries`) list them; downloads live in `<data>/skeletons/<id>/{repo.git,versions/}`, remote registries in `<data>/registries/`; authoring clones in `settings.workspacePath`.
- Project: `.fastdev.lock` with fingerprints of the generated files.
- Project: `.fastdev.toml` in the project folder (origin, features, setup, commands).

## Events and logs

The core emits `jobUpdated`, `jobOutput`, `runUpdated`, `runOutput`, `projectsChanged`, `libraryChanged` (file watcher on the library, debounced) and `settingsChanged`. Output events carry `start`, the absolute index of their first line; the frontend merges them with logs fetched in parallel (`src/lib/logBuffer.ts`) so no line is lost or duplicated.

## Frontend

`src/api` (transport + typed API + browser mock) → `src/stores` (Pinia, one per domain) → `src/pages` → `src/components` (`ui/` kit, feature components). Design tokens and the glass utilities are in `src/styles/main.css`; dark mode uses `data-theme` on `<html>`. Strings are English in code with Russian in `src/i18n/ru.ts`.

Running `npm run dev` alone serves the UI in a browser with `src/api/mock.ts`, which is useful for design work.
