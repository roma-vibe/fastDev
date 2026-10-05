# fastDev developer commands. Run `make help` for the list.

TRIPLE  := $(shell rustc -vV | sed -n 's/host: //p')
SIDECAR := src-tauri/binaries/fastdev-mcp-$(TRIPLE)

.PHONY: help install dev build sidecar check check-rust check-web test library-check fmt mcp-check clean

help: ## Show this help
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F':.*## ' '{printf "  \033[1m%-15s\033[0m %s\n", $$1, $$2}'

install: ## Install npm dependencies
	npm install

sidecar: ## Build the fastdev-mcp bridge and place it where Tauri bundles it
	cargo build -p fastdev-mcp --release
	mkdir -p src-tauri/binaries
	cp target/release/fastdev-mcp $(SIDECAR)

dev: sidecar ## Run the app in development mode (hot reload for the UI)
	cargo build -p fastdev-mcp -p fastdev-cli
	npm run tauri dev

build: sidecar ## Build fastDev.app and the .dmg (target/release/bundle)
	cargo build -p fastdev-cli --release
	npm run tauri build

check: check-rust check-web ## Everything that must pass before finishing a change

check-rust: sidecar ## rustfmt, clippy (warnings are errors) and Rust tests
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace

check-web: ## ESLint, Prettier, vue-tsc, Vitest and the i18n check
	npm run check

test: ## Rust and frontend tests only
	cargo test --workspace
	npm test

library-check: ## Validate downloaded skeleton versions and workspace drafts (settings of this machine)
	cargo run -q -p fastdev-cli -- validate --all

fmt: ## Format Rust and frontend code
	cargo fmt --all
	npm run format

mcp-check: ## Ask the running app for its status through the MCP bridge
	cargo run -q -p fastdev-mcp -- --check

clean: ## Remove build outputs
	cargo clean
	rm -rf dist src-tauri/binaries
