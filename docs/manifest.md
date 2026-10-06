# `template.toml` reference

Every skeleton version and draft has a `template.toml` next to its `files/` folder. The format is strict: unknown keys are errors, so typos are caught by `validate_skeleton`. A JSON Schema for editors is in [`schemas/template.schema.json`](../schemas/template.schema.json) (add `#:schema ../../../schemas/template.schema.json` as the first line if your editor uses Taplo).

The version number is **not** in the manifest: it is the name of the version folder.

## Top-level keys

| Key | Type | Required | Description |
|---|---|---|---|
| `schema` | integer | yes | Manifest format version. Always `1`. |
| `id` | string | yes | Must equal the skeleton folder name. Lowercase letters, digits, single dashes, starts with a letter. |
| `name` | string | yes | Display name, e.g. `"Node + Vue"`. |
| `description` | string | no (warning) | 2–4 sentences: what the skeleton is, what is inside, who it is for. Shown in full at the top of the skeleton page; catalog cards show the first two lines, so start with the essence. |
| `category` | string | no (`other`) | `web`, `desktop`, `cli`, `library`, `mobile` or `other`. |
| `languages` | string[] | no (warning) | Catalog filters: `node`, `rust`, `php`, `python`, `go`, … The first one picks the card icon. |
| `tags` | string[] | no | Search keywords. |
| `stack` | string[] | no | Human-readable stack list, e.g. `["Node.js 24+", "Vue 3", "SQLite"]`. |
| `forked_from` | `{ id, version }` | no | Origin of a fork. Set by fastDev when forking; keep it. |
| `translations` | table | no (warning) | UI translations of the manifest texts, see [`[translations.<lang>]`](#translationslang). |

## `[requirements]`

Tool name → semver requirement, checked against the owner's installed tools (found on the login-shell PATH with `<tool> --version`).

```toml
[requirements]
node = ">=24"
npm = ">=10"
```

Creating a project with "Install dependencies" on is blocked while a tool needed for setup is missing: a base requirement that applies to the selection, or a tool named in the setup steps. Other missing tools (e.g. `docker` for a "Docker up" button) are warnings. Requirements use Cargo-style semver syntax (`">=24"`, `"^1.85"`, `">=8.4, <9"`).

## `[features.<name>]`

Optional parts of a skeleton, toggled when a project is created (one checkbox each).

| Key | Type | Description |
|---|---|---|
| `label` | string | Checkbox label. |
| `description` | string | One line under the label. |
| `default` | bool | Initial state of the checkbox (default `false`). |
| `files` | string[] | Paths or globs relative to `files/` that exist only when the feature is on. A folder path includes everything inside. Each pattern must match at least one file. |
| `requirements` | table | Extra tool requirements when the feature is on. |
| `env` | table | Extra `.env` keys when the feature is on (values are templates). |

```toml
[features.docker]
label = "Docker"
description = "Dockerfile and docker-compose.yml for running the app in a container."
default = false
files = ["Dockerfile", "docker-compose.yml", ".dockerignore"]
requirements = { docker = ">=24" }
env = { COMPOSE_PROJECT_NAME = "{{ project.slug }}" }
```

In `*.tmpl` files use `{% if features.<name> %}…{% endif %}`. Commands, setup steps and `[[set]]` edits can depend on a feature with `feature = "<name>"`.

## `[choices.<name>]`

Single-choice options shown as radio groups when a project is created — for example how the project uses Docker. The selected option can add files, env keys and requirements, drop base requirements, **replace the setup steps** and **override or add commands**.

| Key | Type | Description |
|---|---|---|
| `label`, `description` | string | Shown above the options. |
| `default` | string | Key of the option selected by default (required). |
| `when` | table | Show the choice only when earlier choices have one of the given values: `when = { docker = ["run", "full"] }`. Otherwise its value is `""` and none of its options apply. |
| `options.<key>` | table | The options, in display order (see below). |

Option keys:

| Key | Type | Description |
|---|---|---|
| `label`, `description` | string | Radio label and hint. |
| `files` | string[] | Paths/globs that exist only with this option. A file listed by several options is copied when any of them is selected. |
| `requirements` | table | Extra tool requirements. |
| `drop_requirements` | string[] | Base `[requirements]` that do not apply (e.g. no local Node when everything runs in Docker). |
| `env` | table | Extra `.env` keys (templates). |
| `ports` | table | Extra `[ports]` allocated only with this option (e.g. `FORWARD_DB_PORT = 5432` for a database server). |
| `setup` | array of `{ label, run, feature? }` | **Replaces** the base `[[setup]]` steps. |
| `commands.<key>` | partial command | Overrides the given fields of the base command `<key>` (e.g. only `run`), or adds a new command (then `run` is required). `run = ""` removes the base command for this option. |
| `cleanup` | string | Shell command run in the throwaway project after a verification with this option, and before a preview folder with this option is deleted (e.g. remove Docker volumes and images). The project's `.env` values are in its environment; guard every variable in destructive commands (`${APP_SLUG:?}`). `verify_cleanup` is accepted as an old name (validation warns). |
| `preview` | partial `[preview]` | Overrides `command`, `url`, `path` or `message` of [`[preview]`](#preview) with this option. |

```toml
[choices.docker]
label = "Docker"
default = "run"

[choices.docker.options.none]
label = "No Docker"

[choices.docker.options.run]
label = "Docker to run the app"
files = ["Dockerfile", "docker-compose.yml", ".dockerignore"]
requirements = { docker = ">=24" }
commands.docker-up = { label = "Docker up", run = "docker compose up --build", long = true }

[choices.docker.options.full]
label = "Everything in Docker"
files = ["Dockerfile", "docker-compose.yml", ".dockerignore"]
requirements = { docker = ">=24" }
drop_requirements = ["node", "npm"]
setup = [{ label = "Install dependencies", run = "docker compose run --rm dev npm install" }]
commands.dev = { run = "docker compose up dev" }
commands.test = { run = "docker compose run --rm dev npm test" }
cleanup = "docker compose --profile dev down --volumes --remove-orphans --rmi local"
preview = { message = "Everything runs in containers. Open the link; stop the preview when you are done." }

[choices.data]
label = "Data storage"
default = "volume"
when = { docker = ["run", "full"] }
options.volume = { label = "Docker volume", env = { DOCKER_DATA = "data" } }
options.local = { label = "Project folder", env = { DOCKER_DATA = "./data" } }
```

In `*.tmpl` files use `{% if choices.docker == "full" %}…{% endif %}`. A name cannot be both a feature and a choice.

Selected options are applied **in the order the choices are declared**: when two selected options both set `setup`, the later choice's steps win; command overrides and env keys are applied in the same order. Use it deliberately for combinations — e.g. declare `database` before `docker`, let the PostgreSQL option set the host-mode steps (`docker compose up -d --wait db && …`), and let `docker = "full"` replace them with the in-container steps.

## Conditional files (`@choice=option`)

Configurable skeletons (frontend framework, CSS library, database…) often need **different versions of the same file**. Instead of listing them in `files`, put the condition into the path: a path segment may end with `@<name>=<value>`; the condition is removed from the output path.

```
files/
  resources/js/app.ts@frontend=vue            → resources/js/app.ts       (frontend = vue)
  resources/js/app.tsx@frontend=react         → resources/js/app.tsx      (frontend = react)
  resources/js/Pages@frontend=vue/Home.vue    → resources/js/Pages/Home.vue
  vite.config.ts.tmpl@frontend=react          → vite.config.ts            (rendered)
  resources/css/app.css@css=tailwind|none     → resources/css/app.css     (tailwind or none)
  config/queue.php@db=postgres@redis=on       → config/queue.php          (both conditions)
```

- `<name>` is a choice (value: an option key; `a|b` means any of them) or a feature (`on`/`off`).
- Several `@` conditions in one segment must all hold. A condition on a folder applies to everything inside.
- Conditions come last in a segment: `vite.config.ts.tmpl@frontend=react` (rendered as a template), `_gitignore@docker=none` (renamed as usual).
- Segments that do not parse as conditions of known names are literal (`src/@types/x.d.ts`, `a@b.c`).
- `validate_skeleton` rejects unknown names and values, and two files that produce the same project path for the same selection. Make the conditions of alternatives exclusive.
- `files` of features and options keep working; use them for files that exist only with one option, and `@` conditions for alternatives of one path.
- `[[set]]` and everything else refer to the **output** path (`package.json`, not `package.json.tmpl@frontend=vue`).

## `[env]`, `[ports]`, `[secrets]`

Written into the project's `.env.example` and `.env`, merged into the skeleton's own `files/.env.example` (comments, order and other keys are kept; missing keys are appended). The skeleton file may be a template, `.env.example.tmpl`, when its lines depend on choices (e.g. `DB_CONNECTION` per database).

```toml
[env]                                   # values are minijinja templates
APP_NAME = "{{ project.name }}"
APP_SLUG = "{{ project.slug }}"
DB_PATH = "data/{{ project.slug }}.sqlite"

[ports]                                 # unique free port per project, starting at the value
APP_PORT = 3000
WEB_PORT = 5173

[secrets]                               # N random bytes (hex) in .env, empty in .env.example
APP_SECRET = 32
```

- `[ports]` picks, per key, the first port ≥ the value that no other registered project uses and that is free on IPv4 and IPv6. Env values may use `{{ ports.APP_PORT }}`.
- A key must not be in both `[env]` and `[ports]`.
- Secrets are never shown or logged by fastDev.
- Values with spaces or quotes are written single-quoted (`APP_NAME='Coffee "Shop"'`), which dotenv loaders, Docker Compose, Laravel and shells all read the same way; values with `'` are double-quoted (`"Tom's Shop"`). A project name may not contain both `'` and `"` (fastDev refuses it); other values with both are double-quoted with `\"` escapes, which PHP, Python, Rust, Docker Compose and shells read, while Node's dotenv, `parseEnv` and Vite `loadEnv` cut the value at the first `\"`. Vite's `loadEnv` (dotenv-expand) expands `$VAR` even inside single quotes, so a name like `Price $5` changes there: read names with Node's `process.loadEnvFile`/`util.parseEnv` (no expansion) when they must be exact. Read `.env` with a dotenv library, not with `source`/`cut`.
- Every nested `.env.example` (e.g. one per workspace package) also gets a `.env` copy next to it when the project is created; only the root files get the `[env]`, `[ports]` and `[secrets]` values. Ignore every `.env` in `_gitignore` (`.env`, `*/.env`), because the first commit includes everything else.

## `[[set]]`

Structured edits of JSON or TOML files after copying — for files that must contain the name but cannot read env. Formatting and comments are preserved (TOML) or re-indented like the original (JSON).

| Key | Type | Description |
|---|---|---|
| `file` | string | Output path relative to the project root. |
| `path` | string[] | Keys to the value; missing objects are created. Empty strings are valid keys. `key[field=value]` selects the element of an array of objects/tables whose `field` equals `value`. |
| `value` | string | Template for the new string value. |
| `format` | `json` \| `toml` | Needed only when the name does not tell: `*.json`, `composer.lock` are JSON; `*.toml`, `Cargo.lock`, `uv.lock`, `poetry.lock`, `pdm.lock` are TOML. |
| `feature` | string | Apply only when this feature is on. |

```toml
[[set]]
file = "package-lock.json"
path = ["packages", "", "name"]
value = "{{ project.slug }}"

# Keep Cargo.lock in sync with the renamed package, so `cargo build --locked` works.
[[set]]
file = "Cargo.lock"
path = ["package[name=app]", "name"]
value = "{{ project.slug }}"

# uv.lock of a Python project named "app".
[[set]]
file = "uv.lock"
path = ["package[name=app]", "name"]
value = "{{ project.slug }}"
```

Rename the package in the lockfile whenever you rename it in the manifest (`package.json`, `Cargo.toml`, `pyproject.toml`); otherwise the first install rewrites the lockfile and `--locked`/`--frozen` installs fail. After editing `Cargo.lock` or `uv.lock`, fastDev moves the `[[package]]` entries back into name order (with their sub-tables), as cargo and uv write them, so the new project's lockfile stays unchanged by the first build.

## `[[setup]]`

Steps run in the new project when "Install dependencies" is on (and again by "Run setup again"). Each runs through `/bin/sh -c` with `CI=true` and no terminal (stdin is closed); a non-zero exit stops setup and marks the project "setup failed". Use non-interactive flags (`--no-interaction`, `-y`) and avoid tools that wait for input.

```toml
[[setup]]
label = "Install dependencies"
run = "npm install"

[[setup]]
label = "Prepare database"
run = "npm run db:migrate"
```

Optional `feature = "<name>"`.

## `[commands.<key>]`

Buttons on the project page and targets of `run_project_command`. The key is the stable identifier (`dev`, `build`, `test`, `check`, `start`, `docker-up`…); it is also copied into the project's `.fastdev.toml`.

| Key | Type | Description |
|---|---|---|
| `label` | string | Button label (translated through `[translations.<lang>]`). |
| `run` | string | Shell command, run in the project folder in its own process group. |
| `description` | string | Optional hint. |
| `long` | bool | Long-running (dev server, watcher). Shows Stop; must exit on SIGTERM. |
| `url` | string | URL to open; `${VAR}` is replaced from the project's `.env`. |
| `primary` | bool | The main command (highlighted, quick Run/Stop in lists). At most one; it cannot have `inputs`. |
| `feature` | string | Show only when this feature is enabled. |
| `inputs` | array of `{ name, label, placeholder?, default?, optional? }` | Values asked for before each run: a form in the app, `inputs` in `run_project_command`. Each value is passed as the environment variable `name` (UPPER_CASE) — never spliced into the script — so use it quoted: `"$NAME"`. Commands with inputs cannot be verify or preview commands. |

```toml
[commands.dev]
label = "Dev"
run = "npm run dev"
long = true
url = "http://localhost:${WEB_PORT}"
primary = true

[commands.new]
label = "New extension"
run = 'npm run new -- "$NAME"'
inputs = [{ name = "NAME", label = "Extension name", placeholder = "Tab Organizer" }]
```

## `[verify]`

```toml
[verify]
commands = ["check", "build"]
variants = [{ docker = "full", data = "local" }]
```

`verify_skeleton` creates a throwaway project with all features on and the default choices, runs the (resolved) setup steps and the `commands` (resolved for the selection, so a Docker option runs them in containers), then does the same for every entry of `variants` (choices over the defaults) and runs the options' `cleanup`. Must not include long-running commands. Publishing requires a passing verification of the current draft content.

- A selection whose setup or verify commands need tools that are missing on this machine (e.g. `docker = "none"` needs PHP, which the owner does not have) is **skipped with a warning** instead of failing, as long as at least one selection was verified. The skipped selections are stored with the verification and written into the changelog on publish ("Not verified on the publishing machine: …").
- A draft is verified from a snapshot taken at the start, so editing files during a verification does not mix into the result (the next publish then asks for a new verification).
- `commands` may name commands that only some options add (e.g. `docker-build` from `docker = "run"`); a variant without the command skips it and logs that.
- `validate_skeleton` warns about every choice option that neither the defaults nor a variant select.
- A configurable skeleton lists one variant per alternative that changes files or setup, e.g. every frontend, every CSS library and every database at least once. Combine them so the list stays short: `[{ frontend = "react", css = "bootstrap", db = "postgres" }, { frontend = "vue", css = "none", db = "mysql" }]`.

## `[preview]`

How fastDev previews the skeleton without creating a project: a throwaway copy is set up once per content and selection in `~/Library/Application Support/fastDev/previews/`, then the command runs there. The owner sees the URL or path and the message. A copy is reused while the skeleton content is the same (a version's commit, a draft's files); a changed draft or a re-published version gets a fresh copy, and the old one is removed after running the options' `cleanup`.

```toml
[preview]
command = "dev"                       # command key; default: the primary command
url = "http://localhost:${WEB_PORT}"  # what to open; default: the command's url
path = "target/release/bundle/macos"  # file or folder to show after a build (optional)
message = "Open the link to try the app. Stop the preview when you are done."
cleanup = 'rm -rf "$HOME/Library/WebKit/${APP_SLUG:?}" "$HOME/Library/Caches/${APP_SLUG:?}"'  # optional
```

- Long-running commands keep running until the owner stops the preview; fastDev announces the preview once its `url` answers (up to 5 minutes, e.g. while Docker builds images). Short commands (builds, CLIs) run to completion and the `path` is shown.
- Each selection's preview copy (and each verification) gets its own slug (`preview-<id>-<hash>`, `verify-<id>-<random>`), so their Docker Compose projects, images and volumes never collide. Name Docker resources and app data folders after `COMPOSE_PROJECT_NAME`/the slug, never with fixed names.
- `cleanup` runs in the copy, with the copy's `.env` values in its environment, before the copy is deleted — when the owner deletes the preview files, or when an edited draft replaces an outdated copy. Use it for what the preview leaves outside its folder, such as the `~/Library` folders of desktop apps. The selected options' `cleanup` runs as well.
- `${VAR}` in `url` and `message` comes from the preview's `.env`; `${PREVIEW_DIR}` is the preview folder.
- The selected Docker mode applies (e.g. `docker = "full"` previews inside containers). Options can override any field with `preview = { … }` (see choices).
- Every skeleton should have a `[preview]` with a clear `message` that says what to open and what to try.

## `[translations.<lang>]`

fastDev's interface is English and Russian; manifest texts are English. Translate every user-facing text of the manifest — `name`, `description`, labels and descriptions of features, choices and options, command labels and descriptions, setup step labels, preview messages — per UI language, keyed by the exact English text:

```toml
[translations.ru]
"Laravel + Inertia" = "Laravel + Inertia"
"A Laravel 13 app with an Inertia frontend…" = "Приложение на Laravel 13 с фронтендом на Inertia…"
"Frontend" = "Фронтенд"
"No Docker" = "Без Docker"
"Everything in Docker" = "Всё в Docker"
"Dev" = "Dev"
"Install PHP dependencies" = "Установка PHP-зависимостей"
```

- Missing translations fall back to the app's own dictionary (common labels such as "No Docker", "Dev", "Build") and then to English. `validate_skeleton` warns about texts without a translation and about stale keys that are no longer texts of the manifest.
- Keep technical names as they are ("Docker", "Vue", "SQLite"). Include untranslated-looking texts too (`"Dev" = "Dev"`), so the warning stays empty.
- UI languages today: `ru`.

## `[updates]`

```toml
[updates]
ecosystem = "npm"
hold = { typescript = "<6.1", "@types/node" = "24" }   # optional
```

`hold` keeps packages within a semver range on purpose (e.g. until typescript-eslint supports a newer TypeScript): newer versions are listed as held and skipped by level updates; naming the package explicitly still updates it.

Enables `check_skeleton_updates` / `apply_skeleton_updates`. Supported: `npm` (including workspaces). Other ecosystems are planned.

## Template context

Available in `*.tmpl` files and in `[env]`, feature and option `env` and `[[set]]` values. Undefined variables are errors.

| Variable | Example |
|---|---|
| `project.name` | `Мой магазин` |
| `project.slug` | `moi-magazin` |
| `project.brief` | the initial prompt (may be empty) |
| `project.created_at` | `2026-09-30` |
| `skeleton.id`, `skeleton.name`, `skeleton.version` | `node-vue`, `Node + Vue`, `1.0.0` |
| `features.<name>` | `true` / `false` |
| `choices.<name>` | selected option key, e.g. `"full"`; `""` when the choice does not apply |
| `options.git`, `options.install`, `options.agents_md`, `options.spec_md`, `options.claude_md` | booleans |
| `env.<KEY>` | final values of `[env]`, feature env and ports (secrets are empty) |
| `ports.<KEY>` | allocated ports |

Rendering uses minijinja with default settings (no `trim_blocks`), keeps the trailing newline, and supports the usual filters (`trim`, `upper`, `default`, …).

## Files produced for the project

Besides the skeleton files, fastDev writes:

- `.env` and `.env.example` (see above);
- `CLAUDE.md` containing `@AGENTS.md` when the option is on;
- `.fastdev.toml` — project metadata: name, slug, skeleton id/version/repository/commit, enabled features, selected choices, the setup steps and commands **resolved for that selection**, and the translations of their labels. It is committed and the owner may edit the commands.
- `.fastdev.lock` — sha256 of every file as created (used to tell the owner's changes from skeleton updates later).
