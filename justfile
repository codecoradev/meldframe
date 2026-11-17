# Daily commands. Install just: https://github.com/casey/just
# Cargo recipes honour an optional CARGO_TARGET_DIR from the environment (one per worktree when
# several build at once; see CONTRIBUTING.md). Unset, the target stays in target/.

default:
    @just --list

# Start Postgres (docker compose, or a local cluster without Docker) and seed it
dev-db:
    scripts/pg-start.sh
    scripts/dev-seed.sh

# One command for development: Postgres, seed, then the server on http://127.0.0.1:8080 (Ctrl-C stops
# the server; Postgres keeps running). The dev token is printed once on first run, saved in .dev-token.
dev:
    bash scripts/dev.sh

# Start the server against an already running dev database (reads .env; no seed, no token handling)
dev-server:
    @set -a; [ -f .env ] && . ./.env; set +a; cargo run -p meldframe-server

# The web client (Vite) against the dev server of `just dev`, through the guarded dev proxy: the token
# is read from the git-ignored .dev-token and added by the proxy, never sent to the browser.
web:
    @cd apps/web && { [ -f node_modules/.package-lock.json ] && [ ! package-lock.json -nt node_modules/.package-lock.json ]; } || npm ci
    @cd apps/web && if [ -s ../../.dev-token ]; then MELDFRAME_WEB_TOKEN="$(cat ../../.dev-token)" npm run dev; else echo "web: no .dev-token yet; run 'just dev' first to talk to the server (starting the client without a token)"; npm run dev; fi

dev-web: web

# Smoke test of the server binary on Postgres (skips without DATABASE_URL; run 'just dev-db' first)
dev-smoke:
    bash tools/dev-smoke.sh

# Run the test suite (Rust and web parts)
test:
    @if [ -f Cargo.toml ]; then cargo test --workspace; else echo "test (rust): the workspace arrives in E1.x"; fi
    @if [ -f apps/web/package.json ]; then cd apps/web && ({ [ -f node_modules/.package-lock.json ] && [ ! package-lock.json -nt node_modules/.package-lock.json ]; } || npm ci) && npm test; else echo "test (web): no web client"; fi

# The core crates' tests under WASM on Node (needs wasm-bindgen-test-runner; see AGENTS.md)
wasm-test:
    cargo test --target wasm32-unknown-unknown --tests -p meldframe-order -p meldframe-check -p meldframe-core -p meldframe-layout -p meldframe-history -p meldframe-sync -p meldframe-history-props -p meldframe-sync-scenarios -p meldframe-sync-props

# Build the wasm bridge for the web client into apps/web/.wasm (git-ignored); needs wasm-bindgen-cli, see AGENTS.md
wasm-build:
    bash tools/build-wasm.sh

# Build the wasm bridge for Node and drive it (the JSON boundary of the web client)
wasm-smoke:
    bash tools/check-wasm.sh

# Everything to run locally before opening a pull request: the fast checks that pull request CI
# runs, plus the heavier gates that CI runs only on push to develop and nightly
check:
    sh scripts/ci/selftest.sh
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    for c in deps-check build-matrix portable-check size-check ui-literals; do cargo xtask $c || exit 1; done
    bash tools/check-protocol.sh
    bash tools/check-wasm.sh
    cd apps/web && npm ci && npm run check
    bash scripts/dev-seed-test.sh
    @bash -n scripts/pg-start.sh scripts/dev-seed.sh scripts/dev.sh tools/dev-smoke.sh && echo "check (scripts): syntax ok"
    @echo "migrations: run 'just dev-db' then 'bash tools/check-migrations.sh'; licences: 'cargo xtask licences --check' once it is merged"
