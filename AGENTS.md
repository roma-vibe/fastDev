# fastDev — instructions for AI agents

fastDev is a local macOS app (Rust + Tauri 2, Vue 3 UI) with a **versioned library of project skeletons**. It creates named projects from skeletons, runs them, and exposes everything to agents through an MCP server. The product specification is [`SPEC.md`](SPEC.md) — read it before changing behaviour.

There are two kinds of work in this repository:

1. **Developing the app** — Rust crates, Tauri shell, Vue UI. This file.
2. **Authoring skeletons** — every skeleton is its own git repository (versions are `vX.Y.Z` tags), listed by a registry `index.json`; they are not in this repository. Read [`docs/skeleton-authoring.md`](docs/skeleton-authoring.md). Public on GitHub (`roma-vibe/fastdev-registry`, `roma-vibe/fastdev-<id>`); the owner's clones live in `~/Documents/fastDev-library/{registry,skeletons/<id>}`.

## Map

| Path | What it is |
|---|---|
| `crates/fastdev-protocol/` | Paths, control-socket messages, **MCP tool definitions** (`src/tools.rs`) |
| `crates/fastdev-core/` | All logic. `src/api.rs` is the one API used by the UI, MCP and CLI |
| `crates/fastdev-mcp/` | stdio MCP server that forwards to the app's Unix socket (bundled as a sidecar) |
| `crates/fastdev-cli/` | Headless commands for agents and CI |
| `src-tauri/` | Window, Liquid Glass, menu bar icon, lifecycle, the `call` command |
| `src/` | Vue UI: `api/` (transport, types, browser mock), `stores/`, `pages/`, `components/ui/` (kit), `i18n/`, `lib/`, `styles/main.css` (tokens) |
| `docs/` | `architecture.md`, `skeleton-authoring.md`, `manifest.md`, `mcp.md` |
| `schemas/template.schema.json` | JSON Schema of `template.toml` |

Details: [`docs/architecture.md`](docs/architecture.md).

## Commands

```bash
make dev            # app with hot-reloading UI (builds the MCP sidecar first)
make check          # everything below must pass before you finish
make check-rust     # rustfmt --check, clippy -D warnings, cargo test
make check-web      # ESLint, Prettier, vue-tsc, Vitest, i18n check
make library-check  # validate downloaded skeleton versions and workspace drafts
make build          # fastDev.app + .dmg in target/release/bundle
npm run dev         # UI alone in a browser with the mock backend (design work)
```

Useful while developing: `FASTDEV_HOME=/tmp/fd make dev` runs the app with an isolated data folder; `cargo run -q -p fastdev-cli -- --help`; `cargo run -q -p fastdev-mcp -- --check`.

## Rules

1. **One API.** New functionality is a method in `fastdev_core::api::Core::call`. The UI calls it through `src/api/index.ts`; if agents need it, add a tool definition in `fastdev-protocol/src/tools.rs` (same name, snake_case arguments) and document it in `docs/mcp.md`. MCP must never get: deleting projects from disk, changing settings, launching the app, running arbitrary shell.
2. **Core stays UI-agnostic.** No Tauri types in `fastdev-core`; the app plugs in through `EventSink`.
3. **Long work runs as a job** (`JobManager::start`) with declared steps, `ctx.step/log/warn`, and returns JSON. Processes that keep running belong to the `Runner`.
4. **Skeleton format changes** (`manifest.rs`, generator behaviour) must update `docs/manifest.md`, `schemas/template.schema.json`, `docs/skeleton-authoring.md` and keep existing published skeletons valid (`make library-check`). Bump `SCHEMA_VERSION` only with a migration path.
5. **UI strings** are English in code: `t('Text')`, `tn(n, '{n} item', '{n} items')`. Add Russian to `src/i18n/ru.ts` in the same change (`npm run i18n:check` enforces it). Backend messages stay English.
6. **Design:** use the tokens and utilities in `src/styles/main.css` (`glass`, `bg-fill`, `text-fg-2`, `rounded-card`…) and the `components/ui` kit; support light and dark (`data-theme`), and keep it looking like macOS 26 Liquid Glass.
7. **Frontend layering:** only `src/api` talks to Tauri. Shared state lives in Pinia stores (updated from core events in `main.ts`). `components/ui` are presentational (props in, events out, no stores, no API). Pages and feature components (`components/<feature>/`) call `api` for user actions and report errors through the toast store.
8. **Everything is English**: code, comments, docs, commit messages.
9. **Dependencies:** add only when clearly needed; justify in the commit message. TypeScript stays on 5.9 until vue-tsc/typescript-eslint support newer versions.
10. **Tests:** Rust unit tests next to the code (`#[cfg(test)]`), frontend tests as `src/**/*.test.ts`. Add tests for new logic.

## Pitfalls

- GUI apps on macOS do not inherit the shell PATH; always run tools through `shell::command`/`toolchain::child_env`, never `Command::new("npm")` directly.
- Kill project processes by **process group** (`Runner::stop`), or dev servers leave orphans.
- `*.tmpl` rendering is strict: an unknown variable is an error by design.
- The control socket path must stay under 104 bytes (see `fastdev_protocol::socket_path`).
- `docs/skeleton-authoring.md` and `docs/manifest.md` are embedded into the binary (`get_authoring_guide`); rebuild after editing them.
