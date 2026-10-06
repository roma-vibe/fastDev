# fastDev — Specification

Status: implemented in fastDev 0.1.0 (stages 1–14) · Version 1.0 · 2026-09-30

## 1. Summary

fastDev is a local macOS desktop application (Rust + Tauri 2) that keeps a **versioned library of ready-made project skeletons** and turns any of them into a real, named project in one click: files copied, project name injected, `.env` written, dependencies installed, Git initialised, agent instructions (`AGENTS.md`, `SPEC.md`) prepared.

Created projects appear in the app, where each one has its own page to run, stop, build and test it.

fastDev has **no built-in AI agents**. AI agents (Claude Code, Codex, …) work with fastDev through a local **MCP server** and through the Markdown documentation shipped in this repository. Agents — not the owner — author, improve, update and fork skeletons.

### Why

Every new project today makes an AI agent regenerate the same boilerplate: configs, folder layout, linting, Docker files, instructions. That costs tokens and time and produces slightly different results each time. fastDev replaces that with a prepared, tested, versioned library.

### Goals

1. Create a working, named project from a skeleton in under a minute, without any AI involvement.
2. Keep skeletons immutable per version; updates create new versions and never destroy old ones.
3. Allow forking a skeleton into a new, independent skeleton.
4. Run, stop, build and test created projects from the app.
5. Let AI agents do all library maintenance through MCP tools and clear docs.
6. Add new languages and stacks by adding skeleton repositories — **no app code changes**.

### Non-goals (v1)

- Generating skeletons on the fly from prompts. Skeletons are prepared files.
- Built-in AI agents, chat, or LLM calls.
- Migrating existing projects to a newer skeleton version automatically (the app only informs).
- Windows/Linux support (Tauri makes it possible later; v1 targets macOS).
- Remote/team library hosting (the library is a folder under Git; syncing is done with Git).

## 2. Glossary

| Term | Meaning |
|---|---|
| **Library** | All skeletons fastDev knows: listed by registries, added by URL, or being authored in the workspace. |
| **Registry** | A git repository (or local folder) with an `index.json` listing skeleton repositories. |
| **Skeleton repository** | One git repository per skeleton; its `vX.Y.Z` tags are the published versions. |
| **Workspace** | Folder with working clones of skeleton repositories being authored. |
| **Skeleton** | A prepared project template identified by an `id` (e.g. `node-vue`). Has one or more versions. |
| **Version** | An immutable, published state of a skeleton: the `vX.Y.Z` tag of its repository. |
| **Draft** | Unpublished changes in the workspace clone of a skeleton. Publishing a draft creates a version. |
| **Fork** | A new skeleton with its own `id`, created from a version of another skeleton; records its origin in `forked_from`. |
| **Manifest** | `template.toml` inside every version/draft; describes requirements, env, features, setup and commands. |
| **Project** | A folder created from a skeleton version and registered in the app. |
| **Feature** | An optional on/off part of a project (e.g. `auth`) toggled at creation time. |
| **Choice** | A single-choice option of a skeleton (e.g. the Docker mode) whose selected option can add files, env and requirements and replace setup steps and commands. |
| **Job** | A long-running operation (create project, verify skeleton, check updates) with steps and a log. |
| **Run** | A running process of a project command (e.g. `dev`). |

## 3. Technology

| Area | Choice |
|---|---|
| App shell | Tauri 2 (Rust), macOS 14+; Liquid Glass on macOS 26+ |
| Backend logic | Rust, Cargo workspace: `fastdev-protocol`, `fastdev-core`, `fastdev-mcp`, `fastdev-cli`, `src-tauri` app |
| Frontend | Vue 3 + TypeScript (strict) + Vite + Tailwind CSS 4 + Pinia + Vue Router |
| Storage | App settings: JSON; project registry: SQLite (`rusqlite`, bundled); library: plain files |
| Templating | `minijinja` for `*.tmpl` files; structured edits for JSON (`serde_json`) and TOML (`toml_edit`) |
| MCP | Own stdio JSON-RPC implementation in `fastdev-mcp`, forwarding to the app over a Unix socket |
| Quality | `rustfmt`, `clippy -D warnings`, `cargo test`, ESLint 9, Prettier, `vue-tsc`, Vitest |

## 4. Repository layout

```
fastDev/
├── AGENTS.md                 # How agents develop the fastDev app itself
├── CLAUDE.md                 # "@AGENTS.md" so Claude Code reads the same rules
├── SPEC.md                   # This document
├── README.md
├── Makefile                  # dev, check, build, library-check …
├── Cargo.toml                # Cargo workspace
├── package.json              # Frontend + Tauri CLI
├── index.html, vite.config.ts, tsconfig*.json, eslint.config.js
├── src/                      # Vue frontend
│   ├── api/                  # Typed wrappers over Tauri commands/events (+ browser mock)
│   ├── components/           # UI kit (glass panels, pills, buttons, inputs) and shared parts
│   ├── pages/                # Library, Skeleton, Projects, Project, Settings
│   ├── stores/               # Pinia stores by domain
│   ├── i18n/                 # t('English text') + ru dictionary
│   ├── router/
│   └── styles/               # Tailwind entry, design tokens, glass utilities
├── src-tauri/                # Tauri app crate (window, tray, commands, events)
├── crates/
│   ├── fastdev-protocol/     # Paths, control-socket messages, MCP tool definitions
│   ├── fastdev-core/         # Library, generator, registry, runner, jobs, toolchain, updates, control server
│   ├── fastdev-mcp/          # stdio MCP server → Unix socket bridge (bundled into the .app)
│   └── fastdev-cli/          # Headless library/project commands for agents and CI
├── docs/
│   ├── architecture.md       # Internal architecture of the app
│   ├── skeleton-authoring.md # THE guide for agents: create, change, fork, update skeletons
│   ├── manifest.md           # Full template.toml reference
│   └── mcp.md                # MCP tools reference and connection instructions
└── schemas/
    └── template.schema.json  # JSON Schema of template.toml (editor validation)
```

All documentation, comments and Markdown files are written in **English**. The UI is bilingual (English, Russian).

## 5. Skeleton library

### 5.1 Repositories, registries and cache

Every skeleton is **its own git repository**; there is no shared repository with all skeletons.

```
skeleton repository (e.g. github.com/<owner>/fastdev-node-vue)
├── template.toml         # manifest
├── files/                # exactly what goes into a new project
├── CHANGELOG.md          # one section per version, written on publish
├── README.md, AGENTS.md  # for people and agents working on the skeleton repository
└── .gitignore            # ignores .fastdev-draft.toml, node_modules
tags: v1.0.0, v1.1.0 …    # published versions
```

A **registry** is a git repository (or a local folder) with an `index.json`:

```json
{ "schema": 1, "name": "fastDev skeletons",
  "skeletons": [{ "id": "node-vue", "repo": "https://github.com/<owner>/fastdev-node-vue.git",
                  "name": "Node + Vue", "description": "…", "category": "web", "languages": ["node"],
                  "tags": [], "stack": [], "latest": "1.0.0" }] }
```

- The catalog is built from the registries (metadata of `index.json`), skeletons added by URL, and the workspace — **without downloading anything**.
- A skeleton is downloaded on demand (first project, opening its details): `git clone --mirror` into `~/Library/Application Support/fastDev/skeletons/<id>/repo.git`; a version is exported once (`git archive vX.Y.Z`) into `…/skeletons/<id>/versions/<X.Y.Z>/` together with its commit hash. Downloaded versions work offline.
- **Update library** (and app start, in the background) pulls the registries and fetches every downloaded skeleton, so new versions and skeletons appear.
- Remote registries are cloned into `…/fastDev/registries/<hash>/`; local registry folders are read in place (and fast-forwarded when they have an `origin`).
- **Add by URL**: any skeleton repository with a `vX.Y.Z` tag can be added without a registry.
- The **workspace** (`~/Documents/fastDev-library/skeletons/` by default) holds working clones of skeletons being authored.
- The public registry is `github.com/roma-vibe/fastdev-registry`; every skeleton repository is `github.com/roma-vibe/fastdev-<id>`. The owner's machine keeps a clone of the registry in `~/Documents/fastDev-library/registry/` (used as the registry when present, so publishing updates and pushes it) and the workspace clones in `~/Documents/fastDev-library/skeletons/<id>/` with `origin` on GitHub.
- `node_modules`, `target`, `vendor`, `.venv`, `dist` and similar build outputs are never part of a skeleton (validation rejects them).

### 5.2 Immutability

A published version is a git tag and is never changed. Projects record the repository and the commit of their version; fastDev keeps using the commit it downloaded and warns if a tag was moved later (only a local repository may re-publish a version in place during development, see §14; syncing then refreshes the download). Any change goes through a draft and a new version.

### 5.3 Files inside `files/`

- Copied verbatim by default.
- `*.tmpl` files are rendered with minijinja and saved without the `.tmpl` suffix. Only these files are rendered, because Vue, Blade and others also use `{{ }}`.
- `_gitignore` is renamed to `.gitignore` (a real `.gitignore` inside the library would affect this repository's Git).
- Files listed under a feature (`[features.<name>].files`) are copied only when the feature is enabled.
- `AGENTS.md.tmpl`, `SPEC.md.tmpl` in the root are **agent files**: included only when the corresponding option is on. They describe the future project (structure, commands, rules), not the skeleton.
- `.env.example` must exist when the skeleton uses environment variables. `.env` must not exist in `files/` (it is generated).

### 5.4 Manifest (`template.toml`)

Summary (full reference: `docs/manifest.md`, JSON Schema: `schemas/template.schema.json`):

```toml
schema = 1
id = "node-vue"
name = "Node + Vue"
description = "Fastify API and Vue 3 SPA with SQLite, Tailwind CSS and TypeScript."
category = "web"                       # web | desktop | cli | library | mobile | other
languages = ["node"]                    # catalog filters: rust, node, php, python, go, …
tags = ["fullstack", "typescript", "sqlite", "tailwind"]
stack = ["Node.js 24+", "Fastify 5", "Vue 3", "Vite", "Tailwind CSS 4", "SQLite"]
# forked_from = { id = "node-vue", version = "1.0.0" }   # set automatically for forks

[requirements]                          # tool → semver requirement, checked before creation
node = ">=24"
npm = ">=10"

[choices.docker]                        # single choice; options can replace setup and commands
label = "Docker"
default = "run"
options.none = { label = "No Docker" }
options.run = { label = "Docker to run the app", files = ["Dockerfile", "docker-compose.yml"], requirements = { docker = ">=24" } }

[choices.docker.options.full]
label = "Everything in Docker"
files = ["Dockerfile", "docker-compose.yml"]
requirements = { docker = ">=24" }
drop_requirements = ["node", "npm"]
setup = [{ label = "Install dependencies", run = "docker compose run --rm dev npm install" }]
commands.dev = { run = "docker compose up dev" }

[choices.data]
label = "Data storage"
default = "volume"
when = { docker = ["run", "full"] }
options.volume = { label = "Docker volume", env = { DOCKER_DATA = "data" } }
options.local = { label = "Project folder", env = { DOCKER_DATA = "./data" } }

[env]                                   # written to .env and .env.example (minijinja values)
APP_NAME = "{{ project.name }}"
APP_SLUG = "{{ project.slug }}"
DB_PATH = "data/{{ project.slug }}.sqlite"

[ports]                                 # unique free port per project, starting at the value
APP_PORT = 3000
WEB_PORT = 5173

[secrets]                               # random hex of N bytes in .env, empty in .env.example
# APP_SECRET = 32

[[set]]                                 # structured edits of JSON/TOML files
file = "package.json"
path = ["name"]
value = "{{ project.slug }}"

[[setup]]                               # run after files are written, when "Install" is on
label = "Install dependencies"
run = "npm install"

[commands.dev]                          # buttons on the project page, MCP run targets
label = "Dev"
run = "npm run dev"
long = true                             # long-running process with Stop
url = "http://localhost:${WEB_PORT}"    # ${VAR} is read from the project's .env
primary = true

[commands.docker-up]
label = "Docker up"
run = "docker compose up --build"
long = true
feature = "docker"                      # shown only when the feature is enabled

[verify]                                # commands run by "Verify" on a throwaway project
commands = ["check", "build"]

[updates]                               # dependency update adapter
ecosystem = "npm"                       # npm (v1); cargo, composer, pip later
```

### 5.5 Template context (minijinja)

| Variable | Example |
|---|---|
| `project.name`, `project.slug`, `project.brief`, `project.created_at` | `"My Shop"`, `"my-shop"`, initial prompt, `2026-09-30` |
| `skeleton.id`, `skeleton.name`, `skeleton.version` | `"node-vue"`, `"Node + Vue"`, `"1.0.0"` |
| `features.<name>` | `true` / `false` |
| `choices.<name>` | selected option, e.g. `"full"`; `""` when the choice does not apply |
| `options.git`, `options.install`, `options.agents_md`, `options.spec_md`, `options.claude_md` | booleans |
| `env.<KEY>`, `ports.<KEY>` | final values written to `.env` |

Undefined variables are errors (strict mode), so typos are caught by validation and verification.

### 5.6 Versioning rules

Semver, decided by the author (agent) at publish time:

- **patch** — dependency patch updates, fixes that do not change the project structure;
- **minor** — dependency minor updates, new optional features/files, improved docs;
- **major** — major dependency updates, structural changes, changed commands or env keys.

The first published version of a new skeleton or fork is `1.0.0` unless specified. Every publish prepends a `## <version> — <date>` section with the given change list to `CHANGELOG.md`.

### 5.7 Drafts, verification and publishing

1. **Create draft** — one of:
   - `new`: a new git repository in the workspace with a scaffold (manifest, `files/AGENTS.md.tmpl`, `files/SPEC.md.tmpl`, `.env.example`, `_gitignore`, repository README/AGENTS/.gitignore);
   - `edit`: the workspace clone of the skeleton at its latest version (cloned when missing, pulled when present);
   - `fork`: a clone of a version of another skeleton under a new `id`, with `forked_from` set, the parent's tags removed and `origin` renamed to `upstream`.
   Any uncommitted change or unreleased commit in a workspace clone also counts as a draft.
2. **Edit** files in the workspace clone directly (agents use their normal file tools; file contents never travel through MCP).
3. **Validate** — manifest schema, required files, forbidden folders, template syntax, feature file lists, command references.
4. **Verify** — create a throwaway project in a temp folder with test values (all features on), run setup and the `[verify].commands`; report logs. The result is stored in `.fastdev-draft.toml` (git-ignored) together with a hash of the draft content.
5. **Publish** — requires a successful validation and a successful verification of the **current** draft content (hash match). `allow_unverified` exists for toolchains that are not installed locally and is always reported. Publish prepends the `CHANGELOG.md` entry, commits `Release X.Y.Z`, creates the annotated tag `vX.Y.Z`, pushes the branch and tag to `origin` (when there is one and "Push on publish" is on), updates the entry in the first local registry (commit, and push), and downloads the new version into the cache.
6. **Discard draft** resets the workspace clone to its latest tag (or deletes it when the skeleton was never published).

### 5.7a Preview

`[preview]` in the manifest (command key, optional `url`, `path`, `message`) lets the owner try a skeleton without creating a project. fastDev sets up a throwaway copy once per skeleton content (a version's commit or a draft's files) and selection in `…/fastDev/previews/<id>/`, runs the command there (a dev server, or a build for desktop/CLI stacks) and shows what to do next: the link to open or the folder with the build output, plus the skeleton's message and the live output. The preview keeps running until it is stopped; "Delete preview files" runs the options' `cleanup` (e.g. removes Docker volumes and images) and removes the copy; outdated copies of an edited draft are removed the same way. Available in the UI (Skeleton → Create project → "Preview without creating"), through MCP (`preview_skeleton`, `stop_preview`) and the CLI (`preview`).

### 5.8 Forks

- A fork is a new independent skeleton: new `id`, own name, own version line starting at `1.0.0`.
- `forked_from = { id, version }` is kept forever; the catalog shows "Fork of Node + Vue 1.0.0".
- When the parent has a newer version than `forked_from.version`, the fork's page shows a notice with the parent's changelog. Nothing is applied automatically.

### 5.9 Dependency updates

For skeletons with an `[updates]` ecosystem:

1. **Check updates** (job): materialise the latest version (or the draft) in a temp folder, install, run the ecosystem's outdated check (`npm outdated --json` for npm workspaces), return a table: package, workspace, current, wanted, latest, update type (patch/minor/major).
2. **Apply updates** (job): creates a draft from the latest version if none exists, applies the selected updates in a temp copy (`npm install <pkg>@<version> -w <workspace>`), copies the changed manifests and lockfile back to the draft. The author then verifies and publishes as usual.
3. Adapters for `cargo`, `composer`, `pip` are added later; the adapter interface lives in `fastdev-core::updates`.

## 6. Creating a project

### 6.1 Inputs

| Field | Default | Notes |
|---|---|---|
| Skeleton + version | latest published | any published version can be chosen |
| Project name | — | required, ≤ 80 chars, any language; not both `'` and `"` (no `.env` quoting reads the same in every loader) |
| Slug | derived from the name | ASCII, lowercase, `-`; Cyrillic is transliterated (`Мой магазин` → `moi-magazin`); editable; unique among registered projects (it also names Docker resources): a derived slug that is taken gets `-2`, `-3`…, a taken slug typed by the owner is an error |
| Parent folder | settings → projects folder | final path is `<parent>/<slug>`; must not exist or must be empty |
| Git | on | `git init -b main` + initial commit |
| Install dependencies | on | runs `[[setup]]` steps |
| Features | manifest defaults | one switch per `[features.*]` |
| Choices (e.g. Docker mode, data storage) | manifest defaults | one radio group per `[choices.*]`; hidden when its `when` is not met |
| AGENTS.md | on | renders `AGENTS.md.tmpl` |
| SPEC.md | on | renders `SPEC.md.tmpl`; the initial prompt goes into its "Initial brief" section |
| CLAUDE.md | on | writes `@AGENTS.md` (disabled when AGENTS.md is off) |
| Initial prompt | empty | multi-line text for the agent |

Before creation the app checks the requirements of the selection (base requirements that apply, plus those of features and choice options) against installed tools and shows ✓/✗ with the found versions. For Docker it also checks that the daemon is running. Missing tools are listed with their name, the problem and an install hint: tools needed by the setup steps **block** creation while "Install dependencies" is on; the others are shown as **"Needed later"** warnings (also in the job and returned by MCP).

### 6.2 Steps (a job with a live log)

The target folder is reserved for the job from the request on (a second creation into it is refused), and the ports it allocates count as taken until the project is registered, so concurrent creations never share a folder or a port.

Phase A — materialise (on failure the created folder is removed):
1. Create the target folder.
2. Copy files (skipping disabled features and agent files), rename `_gitignore`.
3. Render `*.tmpl` files.
4. Apply `[[set]]` edits.
5. Allocate ports: first free port ≥ the manifest value that is not used by another registered project and can be bound.
6. Write `.env.example` (manifest `[env]` + `[ports]` values over the skeleton's `.env.example`, secrets empty) and `.env` (same + generated secrets).
7. Write `CLAUDE.md` when enabled.
8. Write `.fastdev.toml` (project metadata, skeleton origin, enabled features, a copy of the commands).

Phase B — setup (on failure the project is still registered, marked "setup failed", with "Run setup again" on its page):
9. Run `[[setup]]` steps.
10. `git init -b main`, `git add -A`, `git commit -m "Initial commit from fastDev skeleton <id> <version>"` (a missing Git identity becomes a warning).
11. Register the project and open its page.

### 6.3 Name propagation

The project name and slug end up in:

- `.env` and `.env.example` (`APP_NAME`, `APP_SLUG`, …) — **the single source of truth** that all skeleton code reads at runtime and build time;
- package/tool manifests that cannot read env (`package.json`, lockfile, `Cargo.toml`, `composer.json`) through `[[set]]` edits;
- rendered `*.tmpl` files (README, AGENTS.md, SPEC.md, compose project names).

After creation nothing in the project refers to "skeleton" or "template" except `.fastdev.toml`.

### 6.4 `.fastdev.toml` (committed in the project)

```toml
[project]
name = "My Shop"
slug = "my-shop"
created_at = "2026-09-30T10:00:00Z"

[skeleton]
id = "node-vue"
version = "1.0.0"

[features]
docker = true

[commands.dev]
label = "Dev"
run = "npm run dev"
long = true
url = "http://localhost:${WEB_PORT}"
```

It lets the app run the project without the library, show "created from 1.0.0 · 1.1.0 available", and re-import the project if the registry is lost. Owners may edit commands in it.

## 7. Projects

### 7.1 Registry

SQLite database `fastdev.db` in the app data folder: id, name, slug, path (unique), skeleton id/version, features, ports, setup status, created/opened timestamps. Projects whose folder disappeared are shown as "Missing" with actions "Locate…" and "Remove from list".

Import: "Add existing project" reads `.fastdev.toml`; without it, commands are derived from `package.json` scripts (`dev`, `build`, `start`, `test`) when present.

### 7.2 Runner

- Commands come from the project's `.fastdev.toml` (filtered by enabled features).
- Each run is spawned through `/bin/sh -c` in the project folder, in its **own process group**, with `PATH` resolved from the user's login shell (GUI apps on macOS do not inherit `.zshrc` PATH; `node` may live in `~/.local/bin`, nvm, Homebrew…).
- stdout/stderr are streamed to the UI in batches, ANSI codes stripped, last 5 000 lines kept per run.
- One active run per (project, command), also when several clients start it at once. **Stop** sends `SIGTERM` to the process group, then `SIGKILL` after 5 s, and returns once every process of the group is gone (the shell often exits before its children). Processes a run left behind after its shell exited are still stopped with it, and before the command starts again.
- `${VAR}` in command URLs is resolved from the project's `.env`; "Open" opens the URL in the default browser.
- Quitting the app stops all runs (the app asks for confirmation when something is running).

### 7.3 Project page

- Header: name, path, skeleton badge (`Node + Vue 1.0.0`), update notice when a newer version exists (with changelog).
- Command buttons from the manifest; long-running ones toggle Run/Stop and show a status dot; `primary` command is emphasised.
- Console with one tab per run (live output, copy, clear).
- Open in: editor, Finder, terminal, browser (when a URL exists).
- Setup: "Run setup again" (re-runs `[[setup]]`).
- Danger zone: "Remove from list" (files stay) and "Delete from disk" (typed confirmation; UI only, never via MCP).

## 8. MCP server

### 8.1 Architecture

```
Claude Code / Codex ──stdio JSON-RPC──▶ fastdev-mcp ──Unix socket──▶ fastDev app (single owner of state and processes)
```

- The app owns the library, registry, jobs and all processes. While it runs, it listens on `~/Library/Application Support/fastDev/fastdev.sock` (folder `0700`, socket `0600`). There is **no TCP port**, so websites cannot reach it.
- `fastdev-mcp` is a small binary bundled in `fastDev.app/Contents/MacOS/`. It always lists all tools (definitions live in `fastdev-protocol`) and forwards each call to the socket.
- If the app is not running, every call returns an error: *"fastDev is not running. Ask the user to start the fastDev app, then retry."* **The bridge never launches the app.**
- Changes made through MCP appear in the UI immediately (same process, same events).
- Registration (shown with a copy button in Settings → MCP):
  - Claude Code: `claude mcp add --scope user fastdev -- "/Applications/fastDev.app/Contents/MacOS/fastdev-mcp"`
  - Codex (`~/.codex/config.toml`): `[mcp_servers.fastdev]` with `command = "…/fastdev-mcp"`.

### 8.2 Tools

Long operations run as jobs. Tools that start a job wait up to `wait_seconds` (default 120, max 600) and return either the final result or `{ job_id, status: "running" }` for polling with `get_job`.

**General**
| Tool | Purpose |
|---|---|
| `fastdev_status` | App version, library path, projects folder, toolchain summary, running processes. |
| `get_job` | Status, steps, log tail and result of a job. |

**Library (read)**
| Tool | Purpose |
|---|---|
| `list_skeletons` | Filter by `query`, `language`, `category`. Returns id, name, description, languages, versions, latest, draft state, fork origin. |
| `get_skeleton` | Manifest details for a version (default latest): features, requirements with local check, commands, changelog, filesystem paths. |
| `get_authoring_guide` | Returns `docs/skeleton-authoring.md` and `docs/manifest.md` plus the library path, so an agent in any folder can author skeletons. |

**Projects**
| Tool | Purpose |
|---|---|
| `list_projects` | All registered projects with status and running commands. |
| `get_project` | Details, commands, runs, skeleton update availability. Accepts id, slug or path. |
| `create_project` | Same inputs as the UI form (§6.1) incl. `brief`. Job. |
| `import_project` | Register an existing folder. |
| `remove_project` | Remove from the list only; files are never deleted by MCP. |
| `run_project_command` | Start a command (`dev`, `build`, `test`, …). Short commands wait for exit and return output; long ones return after `wait_seconds` (default 5) with the first output and the URL. |
| `stop_project_command` | Stop a run. |
| `get_project_logs` | Log tail of a run (default: latest run of each command). |
| `run_project_setup` | Re-run setup steps. Job. |

**Skeleton authoring**
| Tool | Purpose |
|---|---|
| `create_skeleton_draft` | `mode`: `new` \| `edit` \| `fork`; returns the draft path to edit with normal file tools. |
| `validate_skeleton` | Validate a draft or version; returns errors and warnings with file/field. |
| `verify_skeleton` | Throwaway project + setup + verify commands. Job. |
| `publish_skeleton` | `bump` (`patch`\|`minor`\|`major`) or explicit `version`, `changes` (list), `allow_unverified`. |
| `discard_skeleton_draft` | Remove the draft. |
| `check_skeleton_updates` | Outdated dependencies table. Job. |
| `apply_skeleton_updates` | Apply selected (or all of a level) updates into the draft. Job. |

Not exposed through MCP: deleting projects from disk, changing app settings, launching the app.

## 9. User interface

### 9.1 Navigation and pages

A normal resizable window (min 980×640) with a translucent sidebar:

- **Library** — search, filter pills (languages and categories from manifests), favourites (★), cards with name, description, stack, latest version, fork/draft badges.
- **Skeleton page** — header with version selector and changelog; "Create project" form (§6.1) with requirement checks; details (stack, features, commands); "Maintenance" section: draft status, validate, verify, publish, discard, check/apply updates, fork.
- **Create progress** — a sheet with steps, live log, result; "Open project" on success.
- **Projects** — search, list with status dots (running/stopped/missing/setup failed), skeleton badge, path; "Add existing project".
- **Project page** — §7.3.
- **Settings** — Appearance (theme: Light / Dark / System; language: English / Русский / System), Library (registries, workspace, skeletons added by URL), projects folder, Tools (editor: VS Code / Cursor / Zed / custom command; terminal: Terminal / iTerm / Warp / Ghostty), Toolchain status, Behaviour (keep running in the menu bar when the window is closed; push on publish), MCP (socket status, bridge path, copy-ready registration commands).

### 9.2 Visual design — Liquid Glass

- Native window: overlay title bar with inset traffic lights, no visible title, content extends under the title bar.
- Window background: macOS 26 Liquid Glass (`NSGlassEffectView` via `tauri-plugin-liquid-glass`) when available, otherwise `NSVisualEffectView` vibrancy (sidebar material).
- Content: layered translucent "glass" surfaces — `backdrop-filter: blur() saturate()`, a thin inner specular highlight on the top edge, soft large shadows, large concentric radii (window 26 px → panels 20 px → controls 14 px / pill).
- Pill buttons and segmented controls; the active pill uses a soft blue fill (as in the reference screenshot); round icon buttons.
- Motion: short spring-like transitions (150–250 ms), respecting "Reduce motion" and "Reduce transparency" (solid fallbacks).
- Light and dark themes defined as CSS tokens; the Tauri window theme follows the selected theme so native materials match.

### 9.3 Internationalisation

- Source strings are English in code: `t('Create project')`, with `{placeholders}`.
- Russian dictionary in `src/i18n/ru.ts`; a check script fails when a `t()` string has no Russian translation.
- Texts written by skeleton authors (name, description, choice/option/feature/command labels, setup labels, preview messages) are translated by the skeleton itself in `[translations.<lang>]` of `template.toml`; the UI looks them up first, then in its own dictionary (common labels such as "No Docker"), then shows the English text. The registry index, the catalog summary, project metadata and preview info carry the relevant subset.
- Backend errors are returned as English messages with a stable `code`; the UI translates known codes.

### 9.4 Menu bar and lifecycle

- Tray icon (template image) with: Open fastDev, running projects count, Quit.
- Closing the window hides it when "keep running in the menu bar" is on (default); the MCP socket and runs stay alive.
- Quit with running processes asks for confirmation, then stops all process groups: project runs and the commands of running jobs (setup, verification).

## 10. Settings and data

App data folder: `~/Library/Application Support/fastDev/` (override: `FASTDEV_HOME`, used by tests).

| File | Content |
|---|---|
| `settings.json` | theme, language, registries, workspace, skeletons added by URL, projects folder, editor, terminal, favourites, behaviour flags |
| `fastdev.db` | project registry |
| `skeletons/<id>/` | downloaded skeletons: `repo.git` mirror and exported `versions/` |
| `registries/<hash>/` | clones of remote registries |
| `fastdev.sock` | control socket (exists while the app runs) |
| `logs/` | app log files |

Defaults: registries — `~/Documents/fastDev-library/registry` when it exists (an author's clone of the registry), otherwise the public registry `https://github.com/roma-vibe/fastdev-registry.git`; workspace — `~/Documents/fastDev-library/skeletons`.

Every project also gets `.fastdev.lock`: sha256 fingerprints of all generated files (except `.env`) and the skeleton commit — the base for safely reconfiguring options or upgrading the project to a newer skeleton version later.

## 11. Security

- The control socket is reachable only by the current macOS user; no network listener.
- MCP cannot delete files outside fastDev-owned drafts and temp folders, cannot change settings, cannot launch the app. Skeleton ids name folders, so an id that is not `lowercase-with-dashes` (a path, `..`) is rejected by every method, and registry entries with such ids are ignored.
- Project commands run only from `.fastdev.toml` of registered projects or from skeleton manifests during verification — never arbitrary shell passed through MCP.
- Secrets generated for `.env` are never logged or returned by MCP; `.env` is git-ignored by every skeleton.
- Tauri capabilities are limited to the commands the frontend uses; the webview loads only bundled assets.

## 12. First skeleton: `node-vue` 1.0.0

- npm workspaces: `server/` (Fastify 5, TypeScript executed natively by Node ≥ 24 type stripping) and `web/` (Vue 3, Vite, TypeScript, Vue Router, Pinia, Tailwind CSS 4).
- SQLite via built-in `node:sqlite` (no native addons); SQL migrations in `server/src/db/migrations/` applied on start and by `npm run db:migrate`; the database file path comes from `DB_PATH`.
- One `.env` in the root: `APP_NAME`, `APP_SLUG`, `APP_PORT`, `WEB_PORT`, `DB_PATH`, `LOG_LEVEL`. Vite reads it via `envDir`; only whitelisted values reach the browser bundle.
- `npm run dev` starts API and web together (small Node script, no extra dependency); Vite proxies `/api` to the API. `npm run build` type-checks and builds the web app; `npm start` serves API + built SPA on `APP_PORT`.
- The app name from `.env` is shown in the UI and returned by `GET /api/health`.
- Example feature "Notes" end-to-end: migration → repository → service → route → API client → Pinia store → page, with tests (Vitest + Fastify `inject` on an in-memory database; store test).
- Quality scripts: `lint` (ESLint 9 flat config), `format` (Prettier), `typecheck` (`tsc` + `vue-tsc`), `test` (Vitest), `check` (all of them).
- Docker modes (§14 stage 10): **none**; **run** — multi-stage `Dockerfile` and `docker-compose.yml` (`app` service) for the production container, development on the host; **full** — additionally a `dev` image stage and `dev` service (profile `dev`) with the project mounted, `node_modules` in volumes, hot reload, and every command run through `docker compose`. Data storage: `DOCKER_DATA=data` (named volume) or `./data` (project folder), switchable in `.env`.
- `AGENTS.md.tmpl` (project map, layering rules, commands, how to add a feature/migration/page, testing, Docker section when enabled) and `SPEC.md.tmpl` (name, initial brief, goals, scope, open questions — to be completed by the agent).
- Lockfile included, so every project from 1.0.0 gets the same dependency versions.

## 13. Documentation for agents

| File | Audience / content |
|---|---|
| `AGENTS.md` | Agents developing the app: architecture map, commands, rules (English, i18n, no casual dependencies, tests). |
| `AGENTS.md` of each skeleton repository | Written by fastDev: what the repository is, what to edit, never move tags, publish through fastDev. |
| `docs/skeleton-authoring.md` | Step-by-step: new skeleton, change a skeleton, fork, dependency updates, versioning rules, how to write `AGENTS.md.tmpl`/`SPEC.md.tmpl`, definition of done, MCP and CLI equivalents. |
| `docs/manifest.md` | Every manifest key with examples. |
| `docs/mcp.md` | Connection, every tool with arguments and results, typical workflows. |
| `docs/architecture.md` | Crates, data flow, events, jobs, runner, control socket. |

`fastdev-cli` provides the same library operations headless (`sync`, `registry-init`, `add-source`, `validate`, `verify`, `publish`, `draft`, `create`) for CI and for agents working without the app.

## 14. Implementation stages

Each stage ends with `make check` green.

| # | Stage | Done when |
|---|---|---|
| 1 | **Foundation** — Cargo workspace, Tauri app, Vue + Tailwind, tooling, Makefile, AGENTS.md/CLAUDE.md, docs stubs | `make dev` opens an empty window; `make check` passes |
| 2 | **UI shell** — design tokens, Liquid Glass, light/dark/system themes, sidebar navigation, i18n EN/RU, settings persisted, tray & window lifecycle | All pages reachable; theme and language switch live and persist |
| 3 | **Library** — manifest model, scanning, validation, checksums, toolchain detection with login-shell PATH, catalog and skeleton page | Library lists skeletons from the folder with filters, favourites, requirements check |
| 4 | **Skeleton `node-vue` 1.0.0** | Verified through fastDev: install, check, build pass; project runs and shows its name |
| 5 | **Project creation** — generator, ports, env, agent files, setup, git, jobs, progress sheet | A project is created from the UI and runs |
| 6 | **Projects** — registry, runner (process groups, logs), project page, open-in actions, import, missing folders | Run/Stop/Build/Test work with live logs; quitting stops runs |
| 7 | **Versioning** — drafts (new/edit/fork), validate, verify, publish, CHANGELOG, library git commit, fork notices, dependency updates (npm) | A new version and a fork are produced end-to-end; old versions untouched |
| 8 | **MCP** — protocol crate, control socket, bridge, all tools, Settings → MCP, `docs/mcp.md` | Claude Code connected to the bridge can create and run a project; closed app yields the "not running" error |
| 9 | **Docs & packaging** — authoring guide, manifest reference, JSON Schema, architecture, `fastdev-cli`, icons, `.app`/`.dmg` with bundled bridge | `make build` produces an installable app; an agent can add a skeleton using only the docs |
| 10 | **Docker modes and data storage** — `[choices]` in the manifest (options add files/env/requirements, drop requirements, replace setup, override commands, verify cleanup), `[verify] variants`, radio groups in the create form, `choices` in MCP/CLI; node-vue offers Docker none/run/full and data volume/local | A project can be created in each mode; in `full` only Docker is needed on the host, dev hot-reloads in the container, the production container runs; data lands where selected |
| 11 | **Skeletons as git repositories** — one repository per skeleton, versions as `vX.Y.Z` tags, registries (`index.json`), download on demand into a mirror cache, "Update library", add by URL, workspace clones as drafts, publish = changelog + commit + tag + push + registry update, `.fastdev.lock` fingerprints in projects; local registry and repositories until GitHub access is set up | A fresh data folder lists node-vue without downloading it; creating a project downloads the version; publishing a new version makes it appear after "Update library"; old versions stay available |
| 12 | **Dependency warnings and preview** — install hints and Docker daemon check in requirement statuses, blocking vs "needed later" banner in the create form, `[preview]` in the manifest, preview sheet (URL/path, next steps, live output, stop, delete), `preview_skeleton`/`stop_preview` MCP tools and CLI `preview`, CLI `publish --replace` for development | node-vue previews its dev server from the create form; missing tools are explained with install hints |
| 13 | **Configurable skeletons and translations** — conditional files (`path@choice=option`), `[[set]]` array selectors and lockfile formats (`Cargo.lock`, `uv.lock`, `composer.lock`), option `cleanup` and `preview` overrides, verify commands added by options, `[translations.<lang>]` with validation, full description on the skeleton page, `create_project` from a draft, content-keyed previews with their own Compose names, `[preview] cleanup` and option cleanups (with the copy's `.env`), previews announced once their URL answers, CLI preview output and Ctrl+C, validation of untranslated texts, unverified options and deprecated keys, `[[package]]` order kept in `Cargo.lock`/`uv.lock`, per-option `ports`, verification that skips selections whose tools are missing and runs from a snapshot, unmanaged `.env` lines kept verbatim, `xcode-clt` and `uv` requirements, single-quoted `.env` values, `CI=true`; a configurable `laravel-inertia` skeleton (frontend, styles, database, Docker) | A project can be created from `laravel-inertia` with each frontend, CSS library and database; the UI shows skeleton texts in Russian |
| 14 | **Command inputs and workspace skeletons** — `inputs` on commands (a form in the app, `inputs` in `run_project_command`, passed as environment variables), `[updates] hold` ranges, `.env` copies of nested `.env.example` files, `chrome` and `zip` requirements, per-command CLI help; the `chrome-extensions` workspace skeleton (many extensions, one shared `node_modules`) | "New extension" asks for a name and creates it inside the workspace; held packages are not offered as updates |

Versions stay at app 0.1.0 and manifest schema 1 while the product is in development. The skeleton repositories are public on GitHub since 2026-10-05: their tags never move, so every change of a skeleton is a new version (`publish --replace` refuses a tag that was pushed). Before that, versions were re-published in place in the local repositories (the tag recreated and the downloaded cache refreshed); a local repository still may, which is why syncing refreshes the downloads of local repositories only.

Implementation notes (0.1.0):

- Output events (`jobOutput`, `runOutput`) carry the absolute index of their first line; the UI merges them with fetched logs by position.
- Ports are checked on IPv4 and IPv6 (Vite binds `::1`).
- The control socket falls back to a per-user temp path when the data folder path is too long for a Unix socket.
- The npm update adapter uses `npm outdated --json --long` at the root (it covers the root and every workspace) and `dependedByLocation` for the workspace folder.
- "Delete from disk" refuses the root, the home folder and its direct children.
- Files written by fastDev get mode `0644` (or keep the replaced file's mode), so Docker images running as a non-root user can read them.
- A missing tool blocks project creation only when setup needs it (a base requirement that applies, or a tool named in the setup steps); other missing tools are warnings.

## 15. Future

- Skeletons (published today: node-vue, node-cli, rust-cli, tauri-vue, telegram-bot, laravel-inertia, chrome-extensions, threejs-voxel-pixel, threejs-voxel, phaser-2d, flutter-app, react-native-expo, rust-macos-widget): Rust web (Axum), Python (FastAPI, CLI).
- Update adapters for cargo, composer, pip.
- Diff between skeleton versions and guided upgrade of existing projects.
- Library sync helpers (pull/push of the library repository from the app).
