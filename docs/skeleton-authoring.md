# Skeleton authoring guide

This guide is for AI agents that create, change, update and fork skeletons in the fastDev library. The owner does not write skeletons by hand: you do. Read it completely before your first change. The manifest reference is in [`docs/manifest.md`](manifest.md); the MCP tools are described in [`docs/mcp.md`](mcp.md).

## 1. What a skeleton is

A skeleton is a **prepared, tested project** that fastDev copies to create a new project in seconds, without an AI regenerating boilerplate. It is not a code generator: every file is written once, verified, and reused many times.

A good skeleton gives a new project, on its first minute:

- a working app for its purpose (web app, desktop app, CLI…), runnable with one command;
- the chosen stack wired up correctly: build, dev server with reload, tests, linting, formatting, type checking;
- one small example feature that goes through every layer and shows where code belongs;
- configuration in one place (`.env`), with the project name taken from it everywhere;
- `AGENTS.md` and `SPEC.md` that tell the next agent how to work on **this project**.

## 2. How skeletons are stored, and the golden rule

Every skeleton is **its own git repository**. Its `vX.Y.Z` tags are the published versions:

```
fastdev-<id>/                 # e.g. github.com/<owner>/fastdev-node-vue
├── template.toml             # manifest
├── files/                    # what goes into new projects
├── CHANGELOG.md              # written by fastDev on publish
├── README.md, AGENTS.md      # about the skeleton repository (written by fastDev, may be improved)
└── .gitignore                # ignores .fastdev-draft.toml and node_modules
tags: v1.0.0, v1.1.0, …
```

- **Registries** (`index.json` in a git repository or local folder) list the repositories; fastDev shows the catalog from them and downloads a skeleton on first use.
- You work in the **workspace**: a folder with working clones of skeleton repositories (`get_authoring_guide` returns its path; default `~/Documents/fastDev-library/skeletons/<id>/`). The uncommitted state of a clone is the **draft**.
- The registry and the skeleton repositories are public on GitHub: `https://github.com/roma-vibe/fastdev-registry.git` and `https://github.com/roma-vibe/fastdev-<id>.git`. The owner's machine keeps a clone of the registry at `~/Documents/fastDev-library/registry` (fastDev uses it as the registry, updates it on publish and pushes it) and the workspace clones at `…/skeletons/<id>` with `origin` on GitHub. A published tag is public and permanent: every change is a new version.

**Golden rule: never move, delete or re-create a published tag, and never commit or tag by hand.** Every change goes through the draft → validate → verify → publish, which commits, tags, pushes and updates the registry, and keeps all old versions.

Never put build outputs or dependencies into `files/`: `node_modules`, `vendor`, `target`, `dist`, `.venv`, `__pycache__`, `.git` are rejected. Never commit `.env`; ship `.env.example`.

## 3. Tools you use

Everything is available in two equivalent ways:

| Step | MCP tool (app running) | CLI (`cargo run -q -p fastdev-cli --`) |
|---|---|---|
| Learn the library | `list_skeletons`, `get_skeleton` | `skeletons`, `show <id>` |
| Update registries and downloads | `sync_library` | `sync` |
| Start a draft | `create_skeleton_draft` | `draft new\|edit\|fork <id> …` |
| Check structure | `validate_skeleton` | `validate <id>` |
| Build a test project | `verify_skeleton` | `verify <id>` |
| Show it to the owner | `preview_skeleton`, `stop_preview` | `preview <id> [--version draft] [--choice k=v]… [--for <seconds>]` (streams the output; Ctrl+C stops it) |
| Try a real project from the draft | `create_project { skeleton, version: "draft", choices, … }` | `create <id> --version draft --name … [--choice k=v]…` |
| Publish | `publish_skeleton` | `publish <id> --bump … --change …` |
| Drop a draft | `discard_skeleton_draft` | `discard <id>` |
| Dependency updates | `check_skeleton_updates`, `apply_skeleton_updates` | `outdated <id>`, `update <id> …` |

File contents never travel through MCP: `create_skeleton_draft` returns the path of the workspace clone and you edit files there with your normal file tools.

If an MCP call answers that fastDev is not running, ask the owner to start the app (do not launch it yourself) or use the CLI from the fastDev repository.

## 4. Workflows

### 4.1 Create a new skeleton

1. Pick an `id`: lowercase, digits, single dashes, starting with a letter, describing the stack and purpose — `rust-desktop`, `node-cli`, `laravel-inertia`, `python-fastapi`. It is permanent. Stack parts the owner picks at creation (frontend, CSS, database) are choices, not part of the id (§6b).
2. `create_skeleton_draft { mode: "new", id, name, description }`. You get a new git repository in the workspace with a scaffold: minimal manifest, `files/AGENTS.md.tmpl`, `files/SPEC.md.tmpl`, `files/README.md.tmpl`, `files/.env.example`, `files/_gitignore`, and the repository's own `README.md`, `AGENTS.md`, `.gitignore`, `CHANGELOG.md`.
3. Build the project **outside the workspace first** when that is easier (a scratch folder): scaffold it with the stack's official tools, make it clean and complete, run it, test it. Then copy the files into `files/` of the draft — without `node_modules` & co. — and adapt them (§5).
4. Write `template.toml` (§6 and `docs/manifest.md`).
5. Write `AGENTS.md.tmpl` and `SPEC.md.tmpl` (§7).
6. `validate_skeleton` until it has no errors (and ideally no warnings).
7. `verify_skeleton`. Fix everything until it passes. Verification creates a real throwaway project with all features on, runs the setup steps and every `[verify].commands` entry.
8. Run the result yourself as well — verification proves it builds, you prove it works. `preview_skeleton { id, version: "draft", choices }` runs the preview command in a throwaway copy (a new copy after every change of the draft); `create_project { skeleton: id, version: "draft", name: "Test Имя 'x'", choices }` creates a real project to try odd names and non-default choices. Remove test projects and their containers when done.
9. `publish_skeleton { id, changes: [...] }` → version `1.0.0`: commit, tag `v1.0.0`, registry entry added. A new repository has no `origin` yet, so the registry entry points to the local folder: create the public repository `github.com/roma-vibe/fastdev-<id>` (ask the owner when you have no access), add it as `origin`, push `main` and the tag, and set the entry's `repo` in the registry clone to `https://github.com/roma-vibe/fastdev-<id>.git` (commit and push the registry). Add an MIT `LICENSE` to the repository root before the first publish, like every other skeleton repository.

### 4.2 Change an existing skeleton (new version)

1. `get_skeleton { id }` and read its changelog and files.
2. `create_skeleton_draft { mode: "edit", id }`. The workspace clone is created (or pulled) at the latest version.
3. Make the change in `files/` and `template.toml` of the clone.
4. `validate_skeleton` → `verify_skeleton` → `publish_skeleton { id, bump, changes }`.

Choose `bump` by impact on **new projects created from the skeleton**:

| Bump | When |
|---|---|
| `patch` | Dependency patch updates, bug fixes, wording in docs; project structure, commands and env keys unchanged. |
| `minor` | Dependency minor updates, new optional features or files, better docs/tests, new commands. |
| `major` | Major dependency updates, changed folder structure, renamed/removed commands or env keys, a different library for a core concern. |

Existing projects are never changed by publishing; the app only shows them that a newer version exists.

### 4.3 Fork a skeleton

Fork when the result is a **different product line** that should evolve separately (e.g. `node-vue` → `node-vue-auth` with authentication built in), not a new version of the same thing.

1. `create_skeleton_draft { mode: "fork", source: "<parent id>", id: "<new id>", name, description, from_version? }`.
2. fastDev clones the parent at that version into the workspace under the new id, removes the parent's tags, renames `origin` to `upstream`, and sets the new `id`, `name` and `forked_from = { id, version }` in the manifest. Keep `forked_from` forever. The fork needs its own repository (e.g. `fastdev-<new id>` on GitHub) as `origin`.
3. Change what makes the fork different, update `AGENTS.md.tmpl`/`SPEC.md.tmpl`, validate, verify, publish → `1.0.0`. The changelog notes the origin.
4. Later parent updates are not applied automatically. When `get_skeleton` shows `parent.newer`, read the parent's changelog and port what is relevant in a new version of the fork.

### 4.4 Update dependencies

1. `check_skeleton_updates { id }` lists outdated packages with `patch`/`minor`/`major` levels (npm ecosystem today).
2. `apply_skeleton_updates { id, level: "minor" }` (or a `packages` list) creates a draft from the latest version when needed and updates manifests and the lockfile in the draft.
3. For major updates, read the upstream migration notes and fix the code in the draft.
4. `verify_skeleton`, then `publish_skeleton` with a changelog listing the important version changes (`Vue 3.5 → 3.6`).

## 5. Rules for `files/`

- **Copied verbatim**, except:
  - `*.tmpl` files are rendered with minijinja and saved without `.tmpl`;
  - `_gitignore` becomes `.gitignore` (at any depth);
  - files listed in a disabled feature's `files` are skipped;
  - `AGENTS.md.tmpl` / `SPEC.md.tmpl` are skipped when the owner turns them off; `CLAUDE.md` (`@AGENTS.md`) is written by fastDev.
- Only files that must contain the project name or options get `.tmpl`. **Never** give `.tmpl` to Vue, Blade, Twig, Handlebars, Go templates or other files that use `{{ }}` themselves. Inside a `.tmpl`, write literal braces with `{% raw %}…{% endraw %}`.
- Template variables (strict: a typo fails validation): `project.name`, `project.slug`, `project.brief`, `project.created_at`, `skeleton.id`, `skeleton.name`, `skeleton.version`, `features.<name>`, `choices.<name>`, `options.git|install|agents_md|spec_md|claude_md`, `env.<KEY>`, `ports.<KEY>`.
- Alternatives of one file for different options are **conditional files**: `resources/js/app.ts@frontend=vue`, `resources/js/app.tsx@frontend=react` (see `docs/manifest.md`, "Conditional files"). Prefer them to big `{% if %}` blocks in source files; keep `{% if %}` for docs and small config differences.
- **The name comes from `.env`.** The code must read `APP_NAME` (and friends) at runtime/build time. Never hardcode the name, never rely on the folder name.
- Files that cannot read env and must contain the name (`package.json`, `package-lock.json`, `Cargo.toml`, `pyproject.toml`) use a neutral placeholder name (`app`) and a `[[set]]` edit in the manifest. Exception: leave `name` out of `composer.json` — a project is not a Composer package, and `composer.lock`'s `content-hash` covers the name, so renaming it would make every install warn that the lock file is out of date. **Rename it in the lockfile too**, or the first install rewrites the lockfile and `--locked`/`--frozen` installs fail: `package-lock.json` has `name` and `packages."".name`; `Cargo.lock` and `uv.lock` need `path = ["package[name=app]", "name"]`.
- The slug is lowercase letters, digits and dashes and may start with a digit. If the stack forbids that for package names (Cargo: no leading digit, no `build`/`deps`/…), derive a safe name in `[env]` and use it in `[[set]]`: `APP_SLUG = "{% if project.slug[:1] in '0123456789' %}app-{% endif %}{{ project.slug }}"`.
- Ship lockfiles (`package-lock.json`, `Cargo.lock`, `composer.lock`, `uv.lock`…) so every project from a version gets identical dependencies. When dependencies differ per option (see §6b), ship one lockfile per combination (`package-lock.json@frontend=vue@css=tailwind`).
- **Reading `.env`:** values with spaces or quotes are single-quoted (`APP_NAME='Coffee "Shop"'`). Read `.env` with the stack's dotenv loader (dotenv, Vite `loadEnv`, Node `process.loadEnvFile`, Laravel, python-dotenv, Docker Compose); never parse it with `cut`/`sed`. Vite's `loadEnv` expands `$VAR` even in single quotes — read the project name with `process.loadEnvFile`/`util.parseEnv` if it must stay exact. Test a name with spaces, quotes and Cyrillic.
- **Setup and commands run without a terminal:** `CI=true`, stdin closed, the login-shell PATH. Use non-interactive flags (`composer install --no-interaction`, `npx --yes`) and make sure nothing prompts. Recent npm versions warn about dependency install scripts; when setup prints such a warning, record the decision in `package.json` (e.g. `"allowScripts": { "fsevents": false }`) so a fresh project installs cleanly.
- **Dev servers must ignore runtime data:** exclude `data/`, SQLite files (`*.sqlite*`, WAL/SHM), logs and uploads from file watchers (Vite `server.watch.ignored`, nodemon/tsx `--ignore`, `cargo watch -i`), or every write reloads the page.
- `.env.example` lists every key with a safe default and a short comment. fastDev overwrites the keys from `[env]`, `[ports]`, `[secrets]` and enabled features' `env`, keeps comments, order and other keys, and copies the result to `.env` (with generated secrets).
- Ports: put every listening port in `[ports]` so each project gets free, non-conflicting ports; the code reads them from env.
- fastDev writes files with mode `0644` (or keeps the original mode); make scripts executable in `files/` if they need to be.
- Keep a `.gitignore` (as `_gitignore`) that ignores `.env` but not `.env.example`, dependency folders, build outputs, local databases.
- Include formatter/linter/test configuration and make the whole quality gate pass with **zero warnings** on a fresh project.
- Keep dependencies minimal and current; prefer the platform and the stack's standard tools.
- All code, comments, UI texts and docs are in **English**.

## 6. The manifest in practice

Minimal useful manifest (full reference: `docs/manifest.md`):

```toml
schema = 1
id = "node-cli"
name = "Node CLI"
description = "TypeScript command-line tool with commander, tests and npm packaging."
category = "cli"
languages = ["node"]
tags = ["cli", "typescript"]
stack = ["Node.js 24+", "TypeScript", "Vitest"]

[requirements]
node = ">=24"

[env]
APP_NAME = "{{ project.name }}"

[[set]]
file = "package.json"
path = ["name"]
value = "{{ project.slug }}"

[[setup]]
label = "Install dependencies"
run = "npm install"

[commands.dev]
label = "Dev"
run = "npm run dev"
long = true
primary = true

[commands.test]
label = "Test"
run = "npm test"

[commands.check]
label = "Check"
run = "npm run check"

[verify]
commands = ["check"]

[updates]
ecosystem = "npm"
```

Checklist for commands:

- `dev` (primary, long, with `url` when it serves something), `build`, `test`, `check` (the full quality gate), plus `start` for production-like runs and `docker-*` commands under the `docker` feature when relevant.
- Every command has a `label`. Long-running commands have `long = true` and must stop cleanly on SIGTERM to their process group.
- Generators that need a value (a new page, module or package in a workspace) are commands with `inputs`: the owner fills a small form, agents pass `inputs` to `run_project_command`. Translate the input labels and placeholders too.
- `[verify].commands` contains only short commands (usually `check` and `build`).
- `[verify].commands` may name commands that only some options add (e.g. `docker-build` from `docker = "run"`); variants without them skip them.
- `[preview]` names the command that shows the skeleton best (dev server, or a build for desktop/CLI stacks) and a `message` that tells the owner what to open and try next (`path` for build outputs). Options override it with `preview = { … }` when the steps differ (e.g. in Docker).
- Requirements list every tool the skeleton needs; fastDev shows missing ones with install hints (block when setup needs them, warn otherwise), so keep them precise. Known tools: `node`, `npm`, `pnpm`, `bun`, `deno`, `git`, `docker`, `php`, `composer`, `cargo`/`rustc`, `python3`, `uv`, `go`, `zip`, and the pseudo-tools `xcode-clt` (Apple Command Line Tools or Xcode, for native builds on macOS: Tauri, Swift, native npm modules) and `chrome` (the Google Chrome app, for browser extensions). Any other tool is looked up on the login PATH with `<tool> --version`.
- `description` has 2–4 sentences (what, what is inside, for whom) — it is shown in full at the top of the skeleton page.
- **Translate** every user-facing text into Russian in `[translations.ru]` (`docs/manifest.md`); `validate_skeleton` lists what is missing.

Optional parts of a skeleton are **features** (on/off, e.g. `[features.auth]`) and **choices** (one of several options, e.g. `[choices.docker]`). Their files are copied, env keys written, commands shown and requirements checked only for the selection; a choice option can also replace the setup steps, override, add or remove (`run = ""`) commands, and override the preview. Use `{% if features.<name> %}` / `{% if choices.<name> == "…" %}` in `.tmpl` files for the related documentation.

## 6a. Docker modes — the standard for every skeleton

The owner chooses per project how Docker is used and where containerised data lives. Every skeleton that can run in a container offers the same two choices (copy them from `template.toml` of the `node-vue` skeleton repository):

| `choices.docker` | Meaning | How to implement |
|---|---|---|
| `none` | Everything on the host. | No Docker files. |
| `run` | Develop on the host; Docker builds and runs the production container. | `Dockerfile` (multi-stage, runs as a non-root user), `docker-compose.yml` with an `app` service, `docker-up`/`docker-down`/`docker-logs` commands. |
| `full` | Everything in containers; only Docker on the host. | A `dev` stage in the `Dockerfile` and a `dev` service (profile `dev`) that mounts the project at `/app`, keeps dependency folders (`node_modules`, `vendor`, `.venv`, `target`…) in **named volumes** (Linux builds must not mix with host files), runs as the same non-root user, publishes the ports and binds servers to `0.0.0.0`. `setup` replaced by `docker compose run --rm dev <install>`, commands overridden to `docker compose up dev` / `docker compose run --rm dev <cmd>`, `drop_requirements` for host runtimes, `cleanup` removing containers, volumes and local images. |

| `choices.data` (`when = { docker = ["run", "full"] }`) | Meaning |
|---|---|
| `volume` | Data in a named Docker volume: `DOCKER_DATA=data`. |
| `local` | Data in the project folder, bind-mounted: `DOCKER_DATA=./data`. |

Use exactly these labels, so the choice reads the same in every skeleton (write your own `description` for the stack, and translate everything in `[translations.ru]`):

| Key | `label` | Russian |
|---|---|---|
| `choices.docker` | `Docker` (description `How the project uses Docker.`) | `Docker` (`Как проект использует Docker.`) |
| `options.none` | `No Docker` | `Без Docker` |
| `options.run` | `Docker to run the app` | `Docker для запуска` |
| `options.full` | `Everything in Docker` | `Всё в Docker` |
| `choices.data` | `Data storage` | `Хранение данных` |
| `options.volume` | `Docker volume` | `Том Docker` |
| `options.local` | `Project folder` | `Папка проекта` |

A stack may leave out an option that makes no sense for it. When an option means the same as in the table, use its label unchanged; when it genuinely does something else for the stack (e.g. a desktop app where Docker only runs the checks), give it a precise label of the same style (`Docker for checks` / `Docker для проверок`) and explain it in the `description`. Commands added by Docker options use the keys and labels `docker-up` "Docker up", `docker-down` "Docker down", `docker-logs` "Docker logs", `docker-build` "Docker build".

Compose mounts `${DOCKER_DATA:-data}:/app/data` (or the stack's data path), so the owner can switch later by editing `.env`. Stacks with database servers (PostgreSQL, MySQL, Redis) run them as services in every Docker mode and mount their data folder the same way. For stacks whose runtime is usually not installed on the owner's machine (PHP/Laravel, specific Python versions) make `full` the default.

Keep `.tmpl` only for the Docker files that differ per mode (`Dockerfile.tmpl`, `docker-compose.yml.tmpl`), document every mode in `AGENTS.md.tmpl`/`README.md.tmpl` (in `full` mode agents must run every command through `docker compose run --rm dev …`), and add `[verify] variants = [{ docker = "full", data = "local" }]` so verification also builds and tests inside containers. Name images and volumes after `COMPOSE_PROJECT_NAME` so `cleanup` (`docker compose … down --volumes --remove-orphans --rmi local`) removes everything a verification or preview created.

## 6b. Configurable skeletons — choosing the stack at creation

A skeleton may let the owner pick parts of the stack when the project is created: the frontend framework, the CSS library, the database, a queue driver… Each is a `[choices.<name>]` with one option per alternative.

**Choice or separate skeleton?** Make it a choice when the alternatives share most of the project (the backend, the structure, the commands, the docs) and differ in a bounded set of files. Make separate skeletons (or forks) when the alternatives change the language, the project layout or most of the docs — e.g. a different backend framework is a different skeleton; Vue vs React on top of the same Laravel backend is a choice.

How to build one:

1. **Names:** use `frontend`, `css`, `database` (and `docker`/`data`) as choice names, with option keys that are the lowercase technology names: `vue`, `react`; `tailwind`, `bootstrap`, `none`; `sqlite`, `postgres`, `mysql`. Labels: `Frontend`, `Styles`, `Database`; option labels are the product names (`Vue`, `React`, `Tailwind CSS`, `Bootstrap`, `Plain CSS`, `SQLite`, `PostgreSQL`, `MySQL`).
2. **Files:** shared files are plain; alternatives are conditional files (`app.ts@frontend=vue` / `app.tsx@frontend=react`, whole folders like `resources/js/Pages@frontend=vue/`). The validator rejects collisions.
3. **Dependencies:** render the dependency manifest from a template (`package.json.tmpl` with `{% if choices.frontend == "vue" %}` blocks and exact versions) and ship **one lockfile per combination** of the choices that change dependencies (`package-lock.json@frontend=vue@css=tailwind`, …). Generate them with a script in the skeleton repository (outside `files/`), never by hand. Dependency updates (`[updates]`) do not support templated manifests: leave `[updates]` out and update by regenerating the lockfiles.
4. **Env and services:** per-option `env` (e.g. `DB_CONNECTION`, `DB_HOST`, `DB_PORT`) and per-option `ports` (a database server's forwarded port, so projects do not collide and SQLite projects get no unused port), a templated `.env.example.tmpl` for keys that exist only with some options, and Compose services per option (`{% if choices.database == "postgres" %}`).
5. **Requirements:** per-option requirements (a local database server is rarely installed; prefer running server databases in Docker, and make `Everything in Docker` the default when the stack needs tools most owners lack).
6. **Docs:** `AGENTS.md.tmpl` and `README.md.tmpl` describe **only the selected stack** (`{% if choices.frontend == "react" %}`) — the new project must not mention the alternatives it did not choose.
7. **Verification:** the default selection plus `[verify] variants` that together cover **every option at least once** (selections whose host tools are missing here are skipped with a warning and noted in the changelog; you may still run them through Docker-based `php`/`composer` wrappers on a scratch PATH), combined to keep the list short (e.g. 3–4 variants for 2 frontends × 3 CSS × 3 databases). Every variant runs the whole quality gate, so every combination you verify must pass with zero warnings.
8. **Preview:** the default selection must preview well; options override `preview` when the steps differ.

## 7. Writing `AGENTS.md.tmpl` and `SPEC.md.tmpl`

These files are for the agent that will build the **new project**, not about the skeleton. After creation there must be no trace of "skeleton" or "template" wording except one line saying which skeleton version bootstrapped the project.

`AGENTS.md.tmpl` must contain, concretely for this stack:

1. `# {{ project.name }} — agent guide`, and "Read `SPEC.md` first" wrapped in `{% if options.spec_md %}`.
2. **Stack** — runtimes, frameworks, key libraries and why.
3. **Project map** — folder tree with one line per important file/folder.
4. **Architecture rules** — the layers and what each may and may not do (e.g. UI → store → API client → route → service → repository → DB), where business logic lives, how errors flow.
5. **Commands** — a table of every script (use `{{ ports.X }}` to show real ports).
6. **Configuration** — `.env` is the single source of truth, the name comes from `APP_NAME`, how to add a key (`.env.example` + config module + docs).
7. **How to add** a feature, a page/screen, an API route, a DB migration — step by step with file names.
8. **Testing** — frameworks, where tests live, how to write them.
9. **Definition of done** — the exact command(s) that must pass (e.g. `npm run check` with zero warnings), build succeeds, docs updated.
10. **Conventions** — typing strictness, naming, formatting, dependencies policy, English everywhere.
11. `{% if features.<x> %}` sections for optional features (Docker…).

`SPEC.md.tmpl` is the product specification the agent completes with the owner:

- title `# {{ project.name }} — Specification`, status and `{{ project.created_at }}`;
- `## Initial brief` with `{{ project.brief }}` (and a fallback sentence when empty: `{% if project.brief | trim %}…{% else %}…{% endif %}`);
- sections with short HTML-comment guidance: Goals, Users, Scope (in/out), Features (stories + acceptance criteria), Data model, API/Interfaces, UI, Non-functional requirements, Open questions, Milestones.

Look at `files/AGENTS.md.tmpl` and `files/SPEC.md.tmpl` of the `node-vue` skeleton repository as the reference quality level.

## 8. Definition of done for a skeleton version

- [ ] `validate_skeleton` has no errors and no warnings (translations included).
- [ ] `verify_skeleton` passes (fresh project, all features on, setup + verify commands).
- [ ] `preview_skeleton` works and its message tells the owner exactly what to do next.
- [ ] You ran the throwaway project: dev command works, the app shows the project name from `.env`, the example feature works, stopping leaves no orphan processes or busy ports.
- [ ] Production path works (`build` + `start`, or the platform's packaging).
- [ ] Every choice option is covered by the default selection or a `[verify] variants` entry (`validate` warns otherwise), and a project with a name like `Test "Shop" Магазин` works.
- [ ] Optional features and every Docker mode work: `run` — `docker compose up --build` serves the app; `full` — `docker compose up dev` hot-reloads and the production `app` container starts; data lands in the volume or the project folder as selected. Remove the containers, volumes and images you created. If something could not be tested, say so in the changelog.
- [ ] `AGENTS.md.tmpl` and `SPEC.md.tmpl` are specific and correct for this version.
- [ ] No hardcoded names, no secrets, no build outputs, lockfile included (and renamed by `[[set]]`).
- [ ] Nothing is left behind by your checks: test projects, containers, volumes, images, preview folders (`delete_preview`), app data folders in `~/Library` created by desktop runs.
- [ ] Changelog entries are short, factual, one change each.

## 9. Adding a new language

Nothing in the app needs to change: a new language is a new skeleton whose manifest declares `languages = ["<lang>"]`, `[requirements]` for its tools (e.g. `php = ">=8.4"`, `composer = ">=2"`, `cargo = ">=1.85"`, `python3 = ">=3.12"`, `uv = ">=0.5"`), setup steps and commands. Toolchain detection runs `<tool> --version` on the owner's login PATH. Install hints exist for the known tools (§6); for other tools the owner sees a generic hint, so mention the install command in `README.md.tmpl`.

Dependency updates currently support the `npm` ecosystem only. For other ecosystems, leave `[updates]` out (or set it; validation warns that it is not supported yet) and update dependencies by hand in a draft. Adding an adapter is a change to `crates/fastdev-core/src/updates.rs` (see `AGENTS.md`).

When a stack needs tools the owner does not have locally (e.g. PHP), make the Docker feature complete enough to run it, and publish with `allow_unverified: true` only when verification is impossible — say so in the changelog.
