# Daily commands. Install just: https://github.com/casey/just

default:
    @just --list

# Start Postgres (docker compose, or a local cluster without Docker) and seed it
dev-db:
    scripts/pg-start.sh
    scripts/dev-seed.sh

# Start the server against the dev database
dev-server:
    @if [ -f apps/server/Cargo.toml ]; then set -a; [ -f .env ] && . ./.env; set +a; cargo run -p meldframe-server; else echo "dev-server: the server arrives in E1.x"; fi

# Start the web client against the dev server
dev-web:
    @if [ -f apps/web/package.json ]; then cd apps/web && npm run dev; else echo "dev-web: the web client arrives in E1.x"; fi

# Run the test suite (Rust and web parts run when they exist)
test:
    @if [ -f Cargo.toml ]; then cargo test --workspace; else echo "test (rust): the workspace arrives in E1.x"; fi
    @if [ -f apps/web/package.json ]; then cd apps/web && npm test; else echo "test (web): the web client arrives in E1.x"; fi

# Everything to run locally before opening a pull request: the fast checks that pull request CI
# runs, plus the heavier gates that CI runs only on push to develop and nightly
check:
    sh scripts/ci/selftest.sh
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    for c in deps-check build-matrix portable-check size-check ui-literals; do cargo xtask $c || exit 1; done
    bash tools/check-protocol.sh
    bash scripts/dev-seed-test.sh
    @bash -n scripts/pg-start.sh scripts/dev-seed.sh && echo "check (scripts): syntax ok"
    @echo "migrations: run 'just dev-db' then 'bash tools/check-migrations.sh'; licences: 'cargo xtask licences --check' once it is merged"
