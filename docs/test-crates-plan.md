# Plan: move the test-only crates into the crates they test (#316)

Status, 26 Nov 2026. Refs #316, finding 6 of `docs/audits/2026-11-25-rust-modular-monolith.md`, ADR-0011, decision 35. Ten of the 45 crates move in the stack below; 35 stay, each with its reason in the table, and what would let them move.

## Why

45 crates hold only tests (about 67,400 lines, each with at most two lines of source): `-tests`, `-props` and `-scenarios`. They were made because the crate under test stood at its ceiling, and several manifests say so. ADR-0011 rejected "move tests out of the count", and these crates do exactly that. With the ceiling at 50,000 lines (decision 42, standard v1.3) the reason is gone. `cargo xtask mm-check` rule MM6 already fails a new one and lists each existing one in `[allow_test_crate]` of `xtask/monolith.toml`; an entry only goes down, so each move removes its entry.

ADR-0011 and decision 35 carry a status note: superseded for the test crates. The decision to split the core along real seams stands; only the tests-in-crates-of-their-own part goes.

## The rules of a move

- **A pure move.** The test files go to `tests/` of the crate under test with `git mv`. The diff of a moved file is paths and imports only: an `include_str!` or `#[path = ...]` that climbs to the repository root or to another crate gets the new number of `..`, and `mod common;` that would clash with a `common` already there is given a `#[path]` to the renamed directory. No test is rewritten, split or merged; no test is added or dropped. (Two exceptions to "paths only", both a name in a message or a comment that would point at a crate that no longer exists, are named in the commit that has them.)
- **Dev-dependencies.** What the tests used (`[dev-dependencies]` or `[dependencies]` of the old crate) becomes `[dev-dependencies]` of the crate under test, minus what it already has. The `wasm-bindgen-test` pin, with its comment, moves with the `wasm32` dev-dependency table. A crate that already has a `tests/` directory keeps its files; a clash of two file names or two directories is settled by renaming the incoming one with a prefix (recorded in the commit body), never by merging.
- **Layering.** `cargo xtask deps-check` does not read dev-dependencies: `xtask/src/metadata.rs` skips every dependency of kind `dev` ("dev-dependencies do not ship, so they do not count for layering"). So a test that lived in a `tool` crate (allowed to depend on anything) can sit in a `core`, `feature` or `app` crate with no violation, and `build-matrix` and `portable-check` build no dev-dependency of a crate for the product. What a dev-dependency may not do is make a cycle (next point). One case is a judgment call and is recorded here: a test that needs another feature crate. `variables-import` (`roundtrip.rs`, 2 tests) and `html-import` (the round trip files, 2 tests) read what `export` wrote, so each gets `export` as a dev-dependency. The matrix says a feature crate never imports another feature crate unless it is composite; that is about shipped code, which a dev-dependency is not (a product without `export` still builds, and `build-matrix` builds `export` as a workspace crate whatever the server's features are). The owner can instead ask to keep those test files out of the crates (they would need a home of their own, which is what these crates were), or to declare the edge with `composite`; neither was done.
- **No dev-dependency cycle.** A move is refused when a dev-dependency of the tests depends, directly or not, on the crate under test (cargo accepts such a cycle for integration tests, but it makes the crate and its tests two crates that need each other and hides the real order of the layers). It was checked for every one of the 45 with a script over `cargo metadata` (normal and build dependencies of the workspace crates, the transitive closure, then each dev-dependency of the old crate against the target). The check is not in CI; an xtask rule is the follow-up if the owner wants it.
- **Size, two ceilings.** `cargo xtask size-check --base origin/develop` must pass: no moved file is over 500 lines (checked), no receiving crate is near 50,000 lines (the largest, `html-import`, goes from 4,276 to about 15,000), so `size-allow.toml` gets no entry. The second is a line in `xtask/monolith.toml` that works as a ceiling: a crate above 4,000 lines with its tests counted is a monolith, and `mm-check` applies MM1, MM2 and MM5 to its source (a root of at most 150 lines with no item in it, a one-sentence `//!` on every module, no module cycle). A move that takes a receiving crate over that line was run through `mm-check` first, and when it finds something in the crate's own source the test crate stays and the finding is said here: fixing it is a change of code, not a move. Nine test crates stay for this reason, in three target crates (next section).
- **Counts.** `cargo test --workspace --no-fail-fast` result lines are listed per binary before and after in the body of the commit that moves; the totals are equal (3,381 passed, 0 failed, 2 ignored on develop and on every tip), and the property tests keep their seeds (`MELDFRAME_SEEDS` and the defaults in the test files are not touched). A binary count that goes down is the empty library and doc-test binary of a crate with a `src/lib.rs` that is gone. The wasm tests of each moved crate are counted the same way, from `cargo test --target wasm32-unknown-unknown --tests -p ...`.
- **Lists that name crates, in the same branch as the move that makes them stale:** `justfile` (`wasm-test`), `.github/workflows/nightly.yml` and `.gitea/workflows/nightly.yml` (`wasm-test` and the `property-tests` seeds run), `AGENTS.md` (the `just wasm-test` paragraph and the sentence on the importer), `tools/mutation-check-sync.sh` (the `crates=` list and the path of the mutant that edits the simulated server), `docs/architecture.md` (the entry of each moved crate is folded into the entry of the crate under test), pointers in other documents and comments, and the entry in `[allow_test_crate]`. The three `wasm-test` lists are changed together every time.
- **Under WASM.** A test file keeps its `wasm_bindgen_test` lines. A crate that becomes the target of wasm-listed tests joins the list in the place of the old names, and its own unit tests then run under WASM too, as those of the crates already in the list do (a test without the attribute prints `no tests to run!` and passes).

## The stack

Linear, one commit on the tip of the one before, each passes the gates alone. "Files" counts the files that move (renames); the edits besides are a manifest, `Cargo.lock`, `xtask/monolith.toml`, and where they apply the `justfile`, two workflows, `AGENTS.md`, `docs/architecture.md` and pointers.

| # | Branch | Moves | Into |
|---|---|---|---|
| 0 | `docs/test-crates-plan` | this note, the status notes of ADR-0011 and decision 35 | |
| 1 | `refactor/tests-in-batch` | `meldframe-batch-tests` (1 file) | `meldframe-batch` |
| 2 | `refactor/tests-in-layout` | `meldframe-layout-tests` (9 test files, a common module and a fixture) | `meldframe-layout` |
| 3 | `refactor/tests-in-history` | `meldframe-history-props` (3 test files and a common module, renamed) | `meldframe-history` |
| 4 | `refactor/tests-in-sync` | `meldframe-sync-scenarios` (10 files) | `meldframe-sync` |
| 5 | `refactor/tests-in-variables-import` | `meldframe-variables-import-tests` (12 test files, a common module and a fixture) | `variables-import` |
| 6 | `refactor/tests-in-html-import` | `meldframe-import-html-tests`, `-css-tests`, `-style-tests`, `-paint-tests`, `-roundtrip-tests` (41 test files and 6 helper modules, three of them renamed) | `html-import` |

Branches 1 to 3 prove the mechanism on one crate, a crate with a fixture and a wasm list entry, and a crate that has a `tests/` directory and a clashing `common` already. Branch 4 takes the mutation check script (and was run with `tools/mutation-check-sync.sh resync-seq`: killed). Branch 6 is the largest: 47 renames (41 test files and 6 helper modules) and 16 files of edits, a pure move for the size rule.

## What stays, and why

- **`meldframe-app-harness` and `meldframe-store-contract`** are libraries with source of their own that several test crates share (MM6 does not list them): the shell that plays the platform for several app instances against the real in-memory server, and the `Store` seam cases with their `store_contract!` macro. They stay as they are.
- **Over the monolith threshold (9 test crates, in three target crates).** `meldframe-export-tests` takes `export` from 1,569 to about 4,100 lines, and `mm-check` then finds 18 things in `export` (`src/lib.rs` holds a const, enums, structs and impls in 196 lines, and `tw.rs` has no `//!` sentence). `meldframe-sync-props` takes `meldframe-sync` from 3,817 (with the scenarios moved) to about 5,600, and `mm-check` finds 8 things in `src/replica.rs` (a module root of 500 lines with a const, structs, impls and a fn). The seven app test crates (`meldframe-app-unit-tests`, `-export-tests`, `-inspector-tests`, `-layout-tests`, `-review-tests`, `meldframe-variables-panel-tests`, `-manage-tests`; 15,000 lines) take `meldframe-app` from 3,103 to about 18,500, and the smallest of them alone (1,318) passes 4,000; with the threshold lowered to 3,000 in a scratch copy `mm-check` finds 18 things in `meldframe-app`. What would let them move: giving those three crates the shape MM1 and MM2 ask for (`replica.rs`, `export/src/lib.rs` and `tw.rs`, and the roots of `meldframe-app`), or raising the threshold for tests (the owner's call; this note does not touch `xtask`).
- **The tests of the server stack** (17 crates, table below) stay in this stack. Each dev-depends on `meldframe-server`, which depends on every integration crate, so a move into `meldframe-http`, `meldframe-http-*`, `meldframe-http-live` or `meldframe-host` would make a cycle. What moves instead is a separate stack, after the owner agrees on the destination: `apps/server/tests` (the composition root; nothing depends on it, and the tests are end to end tests of the wired server), with `required-features` on the files that need a cargo feature so that `cargo test -p meldframe-server --no-default-features` still proves that a feature left out removes its tools. They share helpers by `#[path]` across the crates (`meldframe-http-tests/tests/common`, `meldframe-mcp-tests/tests/common`, `meldframe-server-tests/tests/common`, `meldframe-activity-pg-tests/tests/world`, `meldframe-live-tests/tests/common`); the move has to take the group together so that those paths turn into one `tests/common`, and it is about 20,000 lines of tests for a crate of 2,183. `meldframe-assets-http-tests` is the one of them without a cycle (its target is `meldframe-http-assets`), but it includes the `common` of `meldframe-http-tests` by path, so it moves with the group.
- **`meldframe-app-tests` and `meldframe-activity-app-tests`** include `meldframe-app-harness`, which depends on `meldframe-app`: a cycle if they moved into the app. What moves instead is both into `crates/core/meldframe-app-harness/tests` (the harness is what they drive); not done here because it makes the harness a crate under test, which the issue names as staying.
- **`meldframe-activity-tests` and `meldframe-checkpoint-tests`** each hold the tests of a rules crate and of its store contract (`activity-store`, `checkpoint-store`), and the store crates depend on the rules crates: a cycle if the whole crate moved into either. What moves instead is a split by file, checked to be free of a cycle and under the monolith threshold: for checkpoints `roll.rs`, `rules.rs` and `run.rs` (763 lines) into `meldframe-checkpoints` (645 lines) and `memory_store.rs` (31 lines) into `checkpoint-store`; for activity `memory_store.rs` and `store_text.rs` (187 lines) into `activity-store` and the other 16 files (about 2,040 lines, with a `common` module only they use) into `meldframe-activity` (1,775, so about 3,800, just under 4,000). The store files need the `contract` feature of their own crate, which a test of the crate gets only with a dev-dependency of the crate on itself with that feature (or `required-features`, which would drop the test from a run without the feature); that is the part the owner should look at, and why it is not done here.
- **`meldframe-gate-tests`** (the admission gate of `meldframe-host`) needs `meldframe-rooms` (which depends on the host) and `meldframe-server`: a cycle. It stays with the server stack.
- **`meldframe-variables-tests`, `-limits-tests`, `-props` and `-rename-tests`** drive one variable model through several crates (check, history, sync, host, rooms, render, the app): a target in the model crate is a cycle (check and history depend on it), and the honest target is the highest crate of each file's seam, which splits `common` between crates. `meldframe-variables-props` and `meldframe-sync-props` both own a `tests/sim` with the same file names, so they could not join one crate without a renamed directory and a decision about the duplicated simulator (MM3 will want one). They stay until the owner settles where the simulator lives.

## The 45 crates

"Stack" is the branch above. The lines and files are those of the crate before the move, counted as `size-check` counts them (its rs, css and other source files, tests and its one or two lines of source).

| Crate | Lines (files) | Goes to | Stack or reason |
|---|---|---|---|
| `meldframe-batch-tests` | 250 (1) | `meldframe-batch` | 1 |
| `meldframe-layout-tests` | 1,624 (10) | `meldframe-layout` | 2 |
| `meldframe-history-props` | 539 (5) | `meldframe-history` | 3 |
| `meldframe-sync-scenarios` | 1,760 (11) | `meldframe-sync` | 4 |
| `meldframe-variables-import-tests` | 1,903 (14) | `variables-import` | 5 (with `export` as a dev-dependency) |
| `meldframe-import-html-tests` | 1,744 (7) | `html-import` | 6 |
| `meldframe-import-html-css-tests` | 3,234 (12) | `html-import` | 6 |
| `meldframe-import-html-style-tests` | 1,245 (6) | `html-import` | 6 |
| `meldframe-import-html-paint-tests` | 3,595 (19) | `html-import` | 6 |
| `meldframe-import-html-roundtrip-tests` | 943 (3) | `html-import` | 6 (with `export` as a dev-dependency) |
| `meldframe-export-tests` | 2,537 (11) | `export` | stays: `export` would pass 4,000 lines, 18 findings |
| `meldframe-sync-props` | 1,751 (9) | `meldframe-sync` | stays: `meldframe-sync` would pass 4,000 lines, 8 findings in `replica.rs` |
| `meldframe-app-unit-tests` | 4,994 (23) | `meldframe-app` | stays: `meldframe-app` would pass 4,000 lines, 18 findings |
| `meldframe-app-export-tests` | 1,507 (8) | `meldframe-app` | stays: same |
| `meldframe-app-inspector-tests` | 1,525 (4) | `meldframe-app` | stays: same |
| `meldframe-app-layout-tests` | 2,108 (10) | `meldframe-app` | stays: same |
| `meldframe-app-review-tests` | 1,760 (6) | `meldframe-app` | stays: same |
| `meldframe-variables-panel-tests` | 1,318 (6) | `meldframe-app` | stays: same |
| `meldframe-variables-manage-tests` | 2,166 (10) | `meldframe-app` | stays: same |
| `meldframe-app-tests` | 1,704 (7) | `meldframe-app-harness` | stays: harness cycle |
| `meldframe-activity-app-tests` | 679 (3) | `meldframe-app-harness` | stays: harness cycle |
| `meldframe-activity-tests` | 2,229 (18) | `meldframe-activity`, `activity-store` | stays: store cycle, split by file proposed |
| `meldframe-checkpoint-tests` | 795 (5) | `meldframe-checkpoints`, `checkpoint-store` | stays: store cycle, split by file proposed |
| `meldframe-gate-tests` | 837 (6) | `meldframe-host` | stays: rooms and server cycle |
| `meldframe-variables-tests` | 1,774 (11) | several seams | stays: no single seam |
| `meldframe-variables-limits-tests` | 402 (2) | `meldframe-history` | stays: two seams (`limits.rs` is about the checker, `undo_modes.rs` about history) |
| `meldframe-variables-props` | 915 (4) | `meldframe-sync` | stays: `sim` clash with `sync-props` |
| `meldframe-variables-rename-tests` | 1,417 (5) | several seams | stays: eleven dev-dependencies, no single seam |
| `meldframe-server-tests` | 1,965 (13) | `meldframe-server` | stays: its `common` is shared by four server crates |
| `meldframe-http-tests` | 1,977 (10) | `meldframe-server` | stays: server cycle |
| `meldframe-serve-tests` | 232 (1) | `meldframe-server` | stays: server cycle |
| `meldframe-assets-http-tests` | 661 (2) | `meldframe-http-assets` | stays: its helper is the `common` of `meldframe-http-tests` |
| `meldframe-live-tests` | 1,791 (8) | `meldframe-server` | stays: server cycle |
| `meldframe-live-slot-tests` | 227 (1) | `meldframe-server` | stays: server cycle |
| `meldframe-snapshot-tests` | 577 (3) | `meldframe-server` | stays: server cycle |
| `meldframe-mcp-tests` | 3,311 (22) | `meldframe-server` | stays: server cycle |
| `meldframe-mcp-asset-tests` | 211 (1) | `meldframe-server` | stays: server cycle |
| `meldframe-export-http-tests` | 371 (1) | `meldframe-server` | stays: server cycle |
| `meldframe-variables-mcp-tests` | 1,209 (5) | `meldframe-server` | stays: server cycle |
| `meldframe-review-tests` | 1,045 (4) | `meldframe-server` | stays: server cycle |
| `meldframe-capture-tests` | 1,407 (6) | `meldframe-server` | stays: server cycle |
| `meldframe-activity-pg-tests` | 1,674 (7) | `meldframe-server` | stays: server cycle |
| `meldframe-activity-review-tests` | 672 (4) | `meldframe-server` | stays: server cycle |
| `meldframe-activity-tool-tests` | 568 (2) | `meldframe-server` | stays: server cycle |
| `meldframe-checkpoint-pg-tests` | 2,240 (8) | `meldframe-server` | stays: server cycle |

The 35 that stay keep their entries in `[allow_test_crate]` (the reason there, "moves into the crate it tests (#316)", stays true: this note says where and what is in the way); each of those entries goes in the pull request that moves its crate. After the stack, 35 entries are left.

## Checks per branch

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --no-fail-fast` (the per binary result lines before and after), `cargo xtask deps-check`, `cargo xtask size-check --base origin/develop`, `cargo xtask mm-check --base origin/develop`, `cargo xtask portable-check`, the wasm tests of the crates that moved (`cargo test --target wasm32-unknown-unknown --tests -p ...`, one run per crate, counted before and after), `scripts/ci/message-patterns.sh`, and at the end of the stack `cargo xtask build-matrix` (the feature crates `variables-import` and `html-import` are touched).
