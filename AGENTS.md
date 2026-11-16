# AGENTS.md: meldframe

Closed-source design canvas for humans and AI agents. Pre-alpha; docs first, no product code yet.

## Where to look

- Product context, before any design or copy work: `PRODUCT.md`.
- Terms, before naming anything or writing docs: `GLOSSARY.md`. Use its words; the `_Avoid_` words stay out of code, docs and tool text.
- Requirements, before scoping a change: `docs/prd.md`.
- Structure and seams, before adding a module or changing an interface: `docs/architecture.md`.
- Why a hard-to-reverse choice was made, before reopening it: `docs/adr/`.
- Open decisions and order of work: `docs/roadmap.md`. Milestones, epics, acceptance and exit criteria, before starting any work item: `docs/epics.md`. Owner decisions: `docs/decisions.md`.
- Layering, detachable modules, the app layer, the gate and live events, before adding a crate, module or transport: `docs/standards/modular-thin-ui.md` and ADR-0007 to ADR-0010.

## Workflow

Full contributor rules: `CONTRIBUTING.md`.

- Branch from `develop` with a prefix (`feat/`, `fix/`, `docs/`, `chore/`); rebase onto `develop`; one logical change per pull request; squash merge with a clean message.
- Sign off commits (`git commit -s`). Write repository artifacts in English.
- Test first for behaviour changes: a failing test at the agreed seam, then the minimal change.
- Put no AI attribution in commits, pull requests, docs or comments.
- Before opening a pull request, run Cora as an agent reviewer on the full diff and put the verdict in the description (`CONTRIBUTING.md`). The CI Cora review is disabled; the local agent review is the required one. Do not edit a description after opening: it restarts CI.
- Merge only after the owner approves.

## Layers

Every crate declares its kind (core, engine, integration, os, feature, app, ffi, ui, feature-ui, bin, tool); the checker (`cargo xtask deps-check`, from E0) enforces the matrix in `docs/architecture.md` section 3b. Feature modules never import each other except composite crates; UI crates hold no business state and use tokens only; every change passes the admission gate. A file stays under 500 lines and a crate under 2,000; ceilings only go down.

## Commands

- `cargo xtask deps-check`: reads `cargo metadata` and enforces crate kinds and the dependency matrix (`docs/standards/modular-thin-ui.md` L1 to L4). Each crate declares `[package.metadata.meldframe] kind = "..."` (feature crates also `group`, and `composite = ["group"]` to import another group). Prints every violation and exits non-zero on any.
- `cargo xtask build-matrix`: builds the workspace with default features off, then each declared feature of each crate alone (M3), then runs the tests of each crate with default features off (so `apps/server` proves that a feature left out removes its tools: `cargo test -p meldframe-server --no-default-features`). Fails if any step fails.
- `cargo xtask portable-check`: builds every portable-kind crate (core, engine, integration, os, feature, app, ffi) for `wasm32-unknown-unknown` (L5). `rust-toolchain.toml` installs the target; otherwise run `rustup target add wasm32-unknown-unknown`.
- `just wasm-test`: runs the unit and integration tests of the order, check, core, layout, history and sync crates, and the property tests of history and sync (`meldframe-history-props`, `meldframe-sync-props`), and the scenario tests of sync (`meldframe-sync-scenarios`), under WASM on Node, from the same test sources as the native run (E1.1). `.cargo/config.toml` sets `wasm-bindgen-test-runner` as the runner of the `wasm32-unknown-unknown` target, so the recipe is `cargo test --target wasm32-unknown-unknown --tests -p ...`. Needs Node and the runner, whose version must equal the `wasm-bindgen` crate in `Cargo.lock`: `cargo install wasm-bindgen-cli --version <that version> --locked`. A test file imports `wasm_bindgen_test::wasm_bindgen_test as test` on `wasm32` only, so a new test file in these crates needs the same two lines. Property tests default to 20 seeds under WASM (the environment does not reach a WASM test); the 600-seed runs stay native.
- `tools/check-protocol.sh`: regenerates the TypeScript types in `packages/protocol/src/generated` from `meldframe-protocol` and the app layer (`meldframe-app`: Action, Effect, Event, AppState), compiles a sample consumer with `tsc` and fails if the generated files differ from the committed ones. Run it and commit the result after any change to the protocol types.
- `tools/mutation-check-sync.sh [name...]`: the E1.1 mutation check. Switches one replica or history rule off at a time (dependency rule on the client and on the test server, per-actor undo, put-back of a refused undo, and more), runs the history, sync and property tests, and fails if a mutant survives. About a minute per mutation; `--list` prints the names. `MELDFRAME_SEEDS=n` sets the seeds of the property tests (600 by default; `MELDFRAME_SEED=n` replays one).
- `tools/check-migrations.sh`: creates a scratch database on the Postgres at `ADMIN_DATABASE_URL` (default `postgres://postgres@127.0.0.1:5433/postgres`), applies `migrations/*.sql` to it from empty, asserts the schema version and append-only rules, then drops it.
- `cargo xtask size-check [--base <git-ref>]`: 500 lines per file and 2,000 per crate (D1). `xtask/size-allow.toml` lists what is over; an entry must equal the current size, so entries only go down. With `--base` (use `origin/develop` in CI) an entry that is new or higher than on the base fails.
- `cargo xtask ui-literals`: rejects colour, font-size and duration literals in `ui` and `feature-ui` crates and in `apps/web` styles (U4). Read values from the tokens module (`crates/ui/meldframe-ui/src/tokens.rs`); `tokens.rs` and `tokens.css` are exempt.
- `cargo xtask licences --check`: every third-party crate's licence expression must be allowed by `[licenses] allow` in `deny.toml` (D2). The same list drives `cargo deny check` (install with `cargo install cargo-deny --locked`). Model weights have no check yet (no models are shipped).
- Compile-fail tests (`crates/core/meldframe-rooms/tests/ui`, `trybuild`) compare the exact compiler output: after a toolchain bump, or a change to the signature of `Permit::new` or `Room::accept` (the output quotes them), regenerate the `.stderr` files with `TRYBUILD=overwrite cargo test -p meldframe-rooms --test compile_fail` and review the diff.
- Before a pull request: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo xtask deps-check`.

## Parallel worktrees

When several worktrees build at once, give each its own target directory outside the worktree (`export CARGO_TARGET_DIR=<path>`), delete it when the worktree is removed, and watch the free disk space (a target directory is several GB). A shared target directory lets worktrees overwrite each other's `meldframe-*` artifacts: false "cannot find" errors (an `xtask` binary built for another worktree was a case of this, fixed in #101 for the Cargo workspace root, but the overwrite itself remains). The `just` recipes and `cargo xtask` honour the variable through cargo; unset, the target stays in the worktree.

## CI

Gitea Actions in `.gitea/workflows/`: `ci.yml` (the fast set in two jobs, `checks` and `migrations`: scripts self-test, format, lint with warnings as errors, tests, `deps-check`, the protocol drift check, and the migrations check; every pull request must pass it), `nightly.yml` (the heavier gates `build-matrix`, `portable-check`, `size-check`, `ui-literals`, `licences`, `wasm-test`, nightly and on demand; run them yourself before a pull request), `pr-checks.yml` (one job: branch prefix, Message Standard, description sections What, Why, Changes, Testing) and `cora-review.yml` (disabled; config in `.cora.yaml`; the local Cora agent review replaces it). A gate whose command does not exist yet is listed in `.gitea/ci-pending.txt` and passes without running; remove its line in the pull request that adds the command. Check scripts live in `scripts/ci/` (`sh scripts/ci/selftest.sh`).

## Rules that are easy to break

- Keep pull requests small: about 400 changed lines and 10 files, one kind of content. A large or mixed pull request is slow to review and hard to review well; split it by kind (workflow, scripts, code, docs).
- Nothing public is created (accounts, domains, packages, registries) without an owner order.
- Do not carry PPTX, Flutter or Dart.
- Do not start new spikes; build the first vertical slice in this repository with exit criteria written first.
- Never invent users, metrics, testimonials or prices.

## Development

Needs `just`, `psql` and either Docker or local Postgres 16 binaries. Copy `.env.example` to `.env` (development values only).

- `just dev-db`: start Postgres on `127.0.0.1:5433` (databases `meldframe_dev`, `meldframe_test`) and load the development seed. Uses `docker compose` when a Docker daemon is reachable, otherwise `scripts/pg-start.sh` starts a local cluster without Docker (idempotent; run it again after a sandbox or machine restart).
- `just dev-server`, `just dev-web`: start the server and the web client. The server is a composition root for now (registry and in-memory host, no transport): it prints the registered modules and tools and exits. The web client prints "arrives in E1.x" until it exists.
- Postgres tests: `apps/server/tests/pg_*.rs` (the shared store contract, the restart test and the schema checks for `PgStore`) skip with a printed `SKIPPED` unless `DATABASE_URL` is set (`MELDFRAME_REQUIRE_PG=1`, set in CI, makes that a failure); `PgStore` applies `migrations/` with the sqlx migrator and refuses a database loaded by `psql -f` (the dev seed), so do not point it at `meldframe_dev` yet; `DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5433/meldframe_test cargo test -p meldframe-server --test pg_contract --test pg_restore --test pg_isolation` runs them (each test uses a schema of its own).
- `just test`: run the test suite. `just check`: everything to run before opening a pull request (pull request CI runs only the fast part; the rest runs on push to `develop` and nightly).
- `DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5433/meldframe_dev` and `TEST_DATABASE_URL=...meldframe_test`; override the port with `MELDFRAME_PG_PORT`.
- Development seed (`scripts/dev-seed.sh`): workspace `ws_dev`, file `file_dev` and a random per-install token (printed once, saved to the git-ignored `.dev-token`, only its hash is stored). It is idempotent, only runs against `meldframe_dev` or `meldframe_test` on this machine (`scripts/dev-seed-test.sh` checks the guards), and refuses to run when `MELDFRAME_ENV` is `release`, `production` or `prod`, `MELDFRAME_RELEASE` is `1`, or `PROFILE` is `release`. The server must never load it in a release build.

## Knowledge

Team notes live in uteke room `project:meldframe`; the spike history and its docs live in the `rust-core-spikes` repository.
