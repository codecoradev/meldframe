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

# Start the server against an already running dev database (reads .env; no seed, no token handling). Needs
# MELDFRAME_DEV_TOKENS; set MELDFRAME_REVIEWERS (actors, comma separated) for anyone to be allowed to review a batch.
dev-server:
    @set -a; [ -f .env ] && . ./.env; set +a; cargo run -p meldframe-server

# The web client (Vite) against the dev server of `just dev`, through the guarded dev proxy: the token
# is read from the git-ignored .dev-token and added by the proxy, never sent to the browser.
web:
    @[ -f apps/web/.wasm/meldframe_wasm_bg.wasm ] || bash tools/build-wasm.sh
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
    cargo test --target wasm32-unknown-unknown --tests -p meldframe-order -p meldframe-check -p meldframe-css -p meldframe-text -p meldframe-core -p meldframe-layout -p meldframe-render -p meldframe-export-tests -p meldframe-history -p meldframe-revert -p meldframe-sync -p meldframe-sync-props -p meldframe-variables-tests -p meldframe-variables-limits-tests -p meldframe-variables-rename-tests -p meldframe-variables-panel-tests -p variables-import -p meldframe-activity-tests -p meldframe-app-unit-tests -p meldframe-app-layout-tests -p meldframe-app-export-tests -p meldframe-variables-manage-tests -p meldframe-variables-props -p meldframe-checkpoint-tests -p meldframe-app-review-tests -p html-import -p meldframe-headless

# Build the wasm bridge for the web client into apps/web/.wasm (git-ignored); needs wasm-bindgen-cli, see AGENTS.md
wasm-build:
    bash tools/build-wasm.sh

# Build the wasm bridge for Node and drive it (the JSON boundary of the web client)
wasm-smoke:
    bash tools/check-wasm.sh

# Run locally before opening a pull request: the fast checks that pull request CI runs (including
# the protocol drift check and the web client's `npm run check`), plus build-matrix, portable-check,
# size-check, mm-check, toolkit-swap, ui-literals, the wasm smoke test and the seed guards.
# Left to CI and the nightly: `just wasm-test` (nightly), `cargo xtask licences --check` (pull
# request CI and nightly) and `bash tools/check-migrations.sh` (the CI migrations job, needs
# Postgres); run them by hand when you touch what they cover
check:
    sh scripts/ci/selftest.sh
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    for c in deps-check mm-check toolkit-swap build-matrix portable-check size-check ui-literals; do cargo xtask $c || exit 1; done
    bash tools/check-protocol.sh
    bash tools/check-wasm.sh
    cd apps/web && npm ci && npm run check
    bash scripts/dev-seed-test.sh
    bash scripts/pg-start-test.sh
    bash scripts/mutation-check-portable-test.sh
    bash scripts/connect-claude-test.sh
    @for f in scripts/pg-start.sh scripts/pg-start-test.sh scripts/portable-shell-test.sh scripts/mutation-check-portable-test.sh tools/mutation-check-sync.sh scripts/dev-seed.sh scripts/dev.sh tools/dev-smoke.sh tools/connect-claude.sh scripts/connect-claude-test.sh; do bash -n "$f" || exit 1; done && echo "check (scripts): syntax ok"
    @echo "left to CI and the nightly: 'just wasm-test', 'cargo xtask licences --check', 'bash tools/check-migrations.sh' (after 'just dev-db')"
