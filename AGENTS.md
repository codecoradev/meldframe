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
- Merge only after the owner approves.

## Layers

Every crate declares its kind (core, engine, integration, os, feature, app, ffi, ui, feature-ui, bin, tool); the checker (`cargo xtask deps-check`, from E0) enforces the matrix in `docs/architecture.md` section 3b. Feature modules never import each other except composite crates; UI crates hold no business state and use tokens only; every change passes the admission gate. A file stays under 500 lines and a crate under 2,000; ceilings only go down.

## Commands

- `cargo xtask deps-check`: reads `cargo metadata` and enforces crate kinds and the dependency matrix (`docs/standards/modular-thin-ui.md` L1 to L4). Each crate declares `[package.metadata.meldframe] kind = "..."` (feature crates also `group`, and `composite = ["group"]` to import another group). Prints every violation and exits non-zero on any.
- Before a pull request: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo xtask deps-check`.

## CI

Gitea Actions in `.gitea/workflows/`: `ci.yml` (format, lint with warnings as errors, tests, the six xtask checks), `pr-checks.yml` (branch prefix, Message Standard, description sections What, Why, Changes, Testing) and `cora-review.yml` (config in `.cora.yaml`). A gate whose command does not exist yet is listed in `.gitea/ci-pending.txt` and passes without running; remove its line in the pull request that adds the command. Check scripts live in `scripts/ci/` (`sh scripts/ci/selftest.sh`).

## Rules that are easy to break

- Nothing public is created (accounts, domains, packages, registries) without an owner order.
- Do not carry PPTX, Flutter or Dart.
- Do not start new spikes; build the first vertical slice in this repository with exit criteria written first.
- Never invent users, metrics, testimonials or prices.

## Knowledge

Team notes live in uteke room `project:meldframe`; the spike history and its docs live in the `rust-core-spikes` repository.
