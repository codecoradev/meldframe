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

# Every CI check that exists today; checks whose tooling is not there yet are skipped
check:
    @if [ -f Cargo.toml ]; then cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings; else echo "check (rust fmt, clippy): the workspace arrives in E1.x"; fi
    @if [ -d xtask ]; then for c in deps-check build-matrix portable-check size-check ui-literals "licences --check"; do cargo xtask $c || exit 1; done; else echo "check (xtask gates): arrive in E0.1 and E0.2"; fi
    @if [ -f apps/web/package.json ]; then cd apps/web && npm run check; else echo "check (web): the web client arrives in E1.x"; fi
    @bash -n scripts/pg-start.sh scripts/dev-seed.sh && echo "check (scripts): syntax ok"
