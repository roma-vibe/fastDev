# fastDev

A local macOS app with a **versioned library of ready project skeletons**. Pick a skeleton, give the project a name and an idea, press **Create** — the project appears in the chosen folder with the name in `.env`, dependencies installed, Git initialised and `AGENTS.md` / `SPEC.md` ready for your AI agent. Created projects get their own page to run, stop, build and test them.

AI agents (Claude Code, Codex) use fastDev through its MCP server: they create projects for you and maintain the library — new skeletons, new versions, forks, dependency updates.

- Product specification: [`SPEC.md`](SPEC.md)
- Rules for agents developing fastDev: [`AGENTS.md`](AGENTS.md)
- Authoring skeletons: [`docs/skeleton-authoring.md`](docs/skeleton-authoring.md), manifest: [`docs/manifest.md`](docs/manifest.md)
- MCP tools: [`docs/mcp.md`](docs/mcp.md), architecture: [`docs/architecture.md`](docs/architecture.md)

## Requirements

- macOS 14+ (Liquid Glass on macOS 26+)
- Rust (stable) and Node.js 24+ with npm
- Tools needed by the skeletons you use (Node for `node-vue`, Docker for the Docker feature)

## Run and build

```bash
npm install
make dev      # development app with hot reload
make check    # all quality checks
make build    # target/release/bundle/macos/fastDev.app and a .dmg
```

Install the app by copying `fastDev.app` to `/Applications`. Skeletons are not bundled: each one is a git repository listed by a registry (`index.json`), and fastDev downloads a skeleton when it is first used. The default registry is the public one, [`roma-vibe/fastdev-registry`](https://github.com/roma-vibe/fastdev-registry); skeleton authors keep a clone of it in `~/Documents/fastDev-library/registry`, which fastDev then uses instead. Configure registries in **Settings → Folders**.

## Connect AI agents

With the app running, register the MCP server once:

```bash
claude mcp add --scope user fastdev -- "/Applications/fastDev.app/Contents/MacOS/fastdev-mcp"
```

For Codex, add to `~/.codex/config.toml`:

```toml
[mcp_servers.fastdev]
command = "/Applications/fastDev.app/Contents/MacOS/fastdev-mcp"
```

Then ask your agent, for example: *"Create a new Node + Vue project in fastDev called Coffee Shop, everything in Docker; the idea is …"*.

## Library

Every skeleton lives in its own repository (`roma-vibe/fastdev-<id>`); versions are its `vX.Y.Z` tags. Current skeletons:

| Skeleton | What you get |
|---|---|
| `node-vue` | Fastify 5 API + Vue 3 SPA, SQLite (`node:sqlite`), Tailwind CSS 4, Pinia, Vue Router, TypeScript, Vitest |
| `node-cli` | TypeScript command-line tool with commander, bundled by tsdown, ready for npm |
| `rust-cli` | Single-binary Rust command-line tool with clap, anyhow and tracing |
| `tauri-vue` | Native macOS desktop app: Tauri 2 with a Rust core and SQLite, Vue 3 + Tailwind CSS 4 interface |
| `rust-macos-widget` | Native AppKit widget in Rust (objc2) with Liquid Glass, a menu bar item and a global shortcut |
| `laravel-inertia` | Laravel 13 + Inertia 3 with Vue or React, Tailwind CSS / Bootstrap / plain CSS, SQLite / PostgreSQL / MySQL |
| `telegram-bot` | Python 3.14 Telegram bot on aiogram 3 with SQLite and a local playground |
| `chrome-extensions` | A workspace of Chrome Manifest V3 extensions with Vue 3, Vite and Tailwind CSS 4 |
| `flutter-app` | Flutter mobile app (Android, iOS, web preview) with Riverpod and go_router |
| `react-native-expo` | React Native app on Expo with Expo Router and native Liquid Glass |
| `phaser-2d` | Phaser 4 2D browser game with TypeScript and Vite; Vue, React or plain HTML menus |
| `threejs-voxel` | Three.js voxel action game foundation with a style-neutral look, PixiJS HUD and Vue menus |
| `threejs-voxel-pixel` | The same voxel engine with a blocky pixel-art look |

Most skeletons offer Docker modes (none / to run the app / everything in Docker) and keep the project name in `.env`.

New skeletons are added by agents following `docs/skeleton-authoring.md` (a new repository plus a registry entry); no app changes are needed for new languages or stacks.

## License

[MIT](LICENSE). Skeleton repositories are MIT-licensed too; projects created from them belong to you.
