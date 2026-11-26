# Meldframe: audit of the Rust workspace against standard v1.3 (modular monolith, U9)

Scope: read-only audit of `github/develop` at `05a6da8` ("feat(html-import): paint stage ... (#304)", 25 Nov 2026), in a detached worktree. Line counts follow the checker: every `rs ts tsx js mjs css scss svelte html` file under a package directory, tests included, with `node_modules`, `target` and `apps/web/dist` skipped.

Definitions used below.
- "Product" means lines outside tests.
- "Test" means files under `tests/`, `benches/`, `examples/` and `scripts/`, files named `*.test.*`, `*-e2e.*` and `tests.rs`, and the tail of a file from `#[cfg(test)] mod ... {`.
- Per-crate figures come from `cargo metadata` plus a line-count script. They match the checker's totals, and the seven crates the checker names above 2,500 are the same seven listed in 1.1.

Version note: the repository mirror of the standard is v1.2 (`docs/standards/modular-thin-ui.md`, D1: ideal 1,500 to 2,500, hard ceiling 4,000). v1.3 (hard ceiling 50,000, modular monolith above that, U9) was not in the repository yet; the audit treats v1.3 as in force, as the owner decided on 25 Nov 2026.

## 0. Headline

1. The workspace is shaped for the old ceiling.
 - It has 127 packages: 108 Rust crates plus 19 TypeScript packages with a `Cargo.toml` stub.
 - The largest Rust crate is 2,524 lines, nothing is above 4,000, and `xtask/size-allow.toml` is empty.
 - All Rust product code is about 50,700 lines, close to what one v1.3 crate may hold.
 - Many crate boundaries exist only because of the size ceiling (`docs/architecture.md`, `docs/import-html.md`, `docs/adr/0011-small-core-crates.md`).
2. 44 crates are test-only (56,879 lines, each with at most 2 product lines). They were created "because X is at its ceiling". ADR-0011 rejects "move tests out of the count", and the test crates do exactly that.
3. U9 is not met.
 - Outside text is made safe in about a dozen places by 6 different predicates.
 - Those predicates cover 4 distinct character sets, and two of them are verbatim copies in two files each.
 - Caps are counted in bytes in one place, in characters in another, and not at all elsewhere.
 - Node names and text, mode names, the actor of a live frame and `OpRejected.code` reach the app layer with a length cap at most.
4. Copy-paste is measurable and almost entirely cross-crate. A token hash over function bodies finds 25 identical bodies of 25 tokens or more, and 24 of them span two or three crates. The count is 38 at 15 tokens and 11 at 60. Two copies are safety relevant: `touches` in `meldframe-batch` and `meldframe-host`, and the invisible-character predicate in `meldframe-mcp-defs` and `apps/server`.
5. `deps-check` enforces L1 to L4 correctly on what `cargo metadata` shows (127 crates, no violations, verified).
 - It cannot see TypeScript imports.
 - It exempts dev-dependencies.
 - It does not validate `composite`.
 - It does not tie kind to directory.
 - It has no check for U2.
 - Kinds `engine`, `integration` and `os` are used by no crate. 12 adapter crates are declared `bin` and 3 app-layer crates are declared `core`.

## 1. Crates: kind, product lines, test lines, modules, biggest files

Totals by kind (packages / product / test):
- tool 51 / 2,392 / 62,300
- core 27 / 21,700 / 11,783
- ui 17 / 10,124 / 13,357
- bin 14 / 13,199 / 1,206
- feature 14 / 13,075 / 1,303
- feature-ui 2 / 2 / 0
- app 1 / 2,471 / 0
- ffi 1 / 334 / 1,147

The total is 127 packages with 63,297 product lines and 91,096 test lines. The full table is in section 8.

### 1.1 Flags

Above 4,000 lines: none.

Above 2,500 lines (seven, identical to the note `cargo xtask size-check` prints):

| crate | kind | total | structure |
|---|---|---:|---|
| meldframe-web-e2e | tool | 3,545 | 20 Node scripts (`scripts/*-e2e.mjs`, biggest `editor-e2e.mjs` 485); no library code |
| meldframe-web-panels | ui | 3,418 | TypeScript and Svelte, 34 files (1,014 product, 2,404 test); one file per panel and per test |
| meldframe-web | bin | 3,344 | the Svelte app, 30 files (`Canvas.svelte` 457, `Editor.svelte` 374); composition root |
| meldframe-import-html-css-tests | tool | 3,234 | 12 test files, one per reader (selector, style, cascade, Tailwind); no product code |
| meldframe-web-document | ui | 2,684 | 35 files (1,159 product, 1,525 test); pure gesture and key to action code |
| meldframe-export-tests | tool | 2,537 | 11 test files; no product code |
| meldframe-app-model | core | 2,524 | the only Rust product crate over 2,500; flat, 16 files (see 1.2) |

Rust product crates between 1,500 and 2,500:
- `meldframe-app` 2,471
- `meldframe-server` 1,970 (+196 test)
- `meldframe-http` 1,939
- `html-import-css` 1,912
- `meldframe-activity` 1,722
- `variables-import` 1,599
- `export` 1,578
- `html-import-style` 1,568
- `meldframe-store-pg` 1,524
- `meldframe-app-rules` 1,546

`meldframe-app-parts` is at 1,485, just under the band.

### 1.2 Structure, and is it already a modular monolith?

`meldframe-app-model` (2,524, kind core) is a flat `src/` with one file per capability of the app layer's data:
- `action` 472, `activity` 395, `state` 276, `assets` 226, `variables` 174, `variables_io` 150, `event` 142
- `checkpoint` 103, `effect` 101, `export` 94, `property` 88, `frame` 80, `preset` 71, `layout` 66, `classify` 36, `lib` 50 (a mod list)

No file is near 500. The only copied function is `is_asset_id` (`assets.rs:30`, see D3). It is the right shape for one module of a monolith, but it is its own crate only "for the size ceiling".

The app layer is four crates that were one:
- `meldframe-app` (app, 2,471, 20 files)
- `meldframe-app-model` (core)
- `meldframe-app-parts` (core, 1,485, 13 files)
- `meldframe-app-rules` (core, 1,546, 11 files)

Together that is 8,026 product lines. The matrix does not let an `app` crate depend on an `app` crate (`xtask/src/rules.rs:151`: `Feature | App => [Core, Engine, Integration, Os, Feature]`). The other three are therefore declared `core` although they hold app-layer logic, and `docs/architecture.md` says so for `app-parts`.

The same capability names recur across these crates: `variables`, `manage`, `panel`, `picker`, `assets`/`image`, `checkpoints`, `review`/`retry`/`link`, `document`, `layout`, `property`, `export`. For example, `manage` is `meldframe-app/src/manage.rs` (103) plus `meldframe-app-parts/src/manage.rs` (210), plus model types in `variables.rs`.

Other Rust crates, as structured today. Each is a flat module per concept with no directory modules per capability. The only copy inside a crate is `db_i64` in `meldframe-store-pg` (`batches.rs:65`, `checkpoints.rs:49`).

| crate | product | capability modules (lines) |
|---|---:|---|
| meldframe-server (bin) | 1,970 | `lib` 484, `http` 496 (+`http/past` 70), `main` 319, `live` 261, `limits` 81, `calls` 79, `error` 76, `approval` 47, `modules` 29, `owners` 28 |
| meldframe-http (bin) | 1,939 | `lib` 437, `mcp/{mod 292, tools 384, variables 43}`, `conn` 206, `reply` 196, `ws` 133, `auth` 115, `slots` 63, `rest` 70 |
| html-import-css (feature) | 1,912 | `css/{mod 465, selector 443, cascade 367, scan 275, matching 144, decl 74}`, `tw` 163 (already a directory module) |
| meldframe-activity (core) | 1,722 | `reducer` 459, `view` 248, `feed` 237, `note` 169, `summary` 131, `panel` 120, `incoming` 101, `header` 92, `in_effect` 60, `sink` 36 |
| variables-import (feature) | 1,599 | `scan` 490, `plan` 432, `log` 194, `value` 190, `lib` 179, `names` 83, `text` 31 |
| export (feature) | 1,578 | `decl` 485, `emit` 434, `vars` 245, `lib` 196, `names` 100, `tw` 74, `report` 44 |
| html-import-style (feature) | 1,568 | `style` 437, `shorthand` 312, `custom` 193, `vars` 172, `value` 161, `defaults` 153, `computed` 120 |
| meldframe-store-pg (bin) | 1,524 | `lib` 451, `batches` 332, `review` 195, `checkpoints` 196, `calls` 175, `backing` 116, `state` 53 |
| meldframe-protocol (core) | 1,271 | `wire` 247, `patch` 224, `node` 200, `variables` 191, `command` 117, `enums` 116, `decoration` 102, `snapshot` 39 |
| meldframe-sync (core) | 1,231 | `replica` 500 (+`replica/{confirm,rebuild,resync}`), `correction` 306, `types` 125, `footprint` 107 |
| meldframe-host (core) | 1,221 | `gate` 425 (+`gate/permit`), `room` 334, `store` 113 (+`store/memory`), `refusal` 107, `op` 70 |
| html-import (feature) | 1,215 | `paint/{mod 206, border 252, gradient 253, fill 125, radius 115, shadow 106, lex 102, misc 33}` |
| meldframe-variables (core) | 1,201 | `check` 425, `apply` 217, `alias` 143, `error` 138, `resolve` 101, `footprint` 71, `bindings` 66 |
| html-import-tree (feature) | 1,141 | `build` 372, `tag` 165, `lib` 160, `text` 134, `report` 119, `rules` 107, `limits` 84 |

Today the crates are small single-seam crates with one file per concept. The monolith shape (a crate of 5,000 to 50,000 lines made of directory modules with a `mod` list) exists nowhere. Natural capability directories already exist inside crates: `html-import-css/src/css/`, `html-import/src/paint/` and `meldframe-http/src/mcp/`.

Copies between modules are listed in section 3. 24 of the 25 identical function bodies cross a crate boundary. This is what one expects when capability boundaries follow line count: both sides need the helper and neither crate may depend on the other.

### 1.3 Candidate merges under v1.3 (product lines; moves, not rewrites)

| group | crates | product (test) | note |
|---|---|---|---|
| app layer | app, app-model, app-parts, app-rules | 8,026 (0) | one `app` crate with a directory module per panel; removes the `core`-for-`app` mislabel |
| HTML import | html-import, -css, -style, -tree | 5,836 (25) | one crate with modules `tree`, `css`, `style`, `paint`; same group, no edge to another group |
| export / variables import | export + export-dialog; variables-import + -dialog | 1,813; 1,764 | the dialogs are 235 and 165 lines |
| history/sync | undo, history, sync, revert | 3,497 (3,031) | already a chain |
| doc model | protocol, order, check, css, layout, variables, core, render, variant | 5,868 (4,625) | protocol must stay light for ts-rs; needs an owner call |
| host/rooms | host, rooms, approvals, stream, batch, calls, ratelimit, module, mcp-defs | 4,418 (4,127) | |
| activity | activity, checkpoints, activity-store, checkpoint-store | 4,040 (0) | straddles core and feature today |
| http adapters | meldframe-http, -live, -snapshot, -activity, -assets, -capture, -checkpoints, -export, -review, -variables-import, -variant, store-pg | 8,811 (1,990) | each tool module behind its own cargo feature keeps M3 |
| test-only crates | 44 `-tests` / `-props` / `-scenarios` | 0 (56,879) | move to `tests/` of the crate they test; keep a crate only where a harness is shared |

## 2. U9: where outside text enters and how it is made safe

Rule: text from a model, the server or a file is made safe to display and capped at the point it enters the app layer, in one reusable helper. Today there is no such helper. The nearest is `meldframe_activity::is_hidden_text` (`crates/core/meldframe-activity/src/note.rs:13`). It is imported by `meldframe-checkpoints` (`label.rs:4`), `checkpoint-store` (`lib.rs:23`), `activity-store` and `meldframe-http-activity`.

### 2.1 The predicates (all verified by reading)

| # | location | what it removes or refuses | differs from P1 |
|---|---|---|---|
| P1 | `meldframe-activity/src/note.rs:13` `is_hidden_text` | controls, U+00AD, 034F, 061C, 115F-1160, 17B4-17B5, 180B-180F, 200B-200F, 2028-202E, 2060-206F, 2800, 3164, FE00-FE0F, FEFF, FFA0, FFF9-FFFB, 1BCA0-1BCA3, 1D173-1D17A, E0000-E007F, E0100-E01EF | the reference |
| P2 | `meldframe-mcp-defs/src/lib.rs:20` `is_invisible`, and a verbatim copy at `apps/server/src/limits.rs:30` | 200B-200F, 202A-202E, 2028, 2029, 2066-2069, FEFF; no `is_control` | misses 00AD, 034F, 061C, 180B-180F, 2060-2065, 206A-206F, 2800, 3164, FE00-FE0F and tag characters (the comment at `note.rs:7` says so) |
| P3 | `variables-import/src/text.rs:8` `invisible` | controls plus a Cf list: 00AD, 0600-0605, 061C, 06DD, 070F, 0890-0891, 08E2, 180E, 200B-200F, 202A-202E, 2060-206F, FE00-FE0F, FEFF, FFF9-FFFB, 110BD, 110CD, 13430-1343F, 1BCA0-1BCA3, 1D173-1D17A, E0000-E007F, E0100-E01EF | keeps U+2028, 2029, 034F, 3164 and FFA0; removes Arabic number marks and 13430 that P1 keeps |
| P4 | `html-import/src/paint/mod.rs:72` `hidden`, verbatim copy at `html-import-tree/src/report.rs:112` | controls, 00AD, 034F, 200B-200F, 202A-202E, 2060-206F, FE00-FE0F, FEFF, E0000-E007F | keeps 2028, 2029, 061C, 180B-180F, 2800, 3164 and E0100-E01EF |
| P5 | `capture/src/error.rs:79` `clean` | `char::is_control` only | everything else is shown to the agent |
| P6 | `char::is_control` only, as the whole rule | idempotency key: `meldframe-http/src/mcp/tools.rs:70`, `meldframe-http-variant/src/lib.rs:106`, inline `meldframe-http-variables-import/src/lib.rs:131-133`; capture `nodeId` `meldframe-http-capture/src/tool.rs:108` | a key may hold U+202E or U+200B, while a file name may not |

Related copies:
- `design-review/src/lib.rs:148` `bad_char` is P2 plus controls except `\n`. `check_batch` at `:196` repeats it without the newline exception.
- `meldframe-http/src/lib.rs:166` `valid_file` is P2 plus `/` and `\`.
- `apps/server/src/limits.rs:33` `check` repeats the file-name rule, although `limits.rs:54` says it is "the one place workspace and file names are checked".

Behaviour that differs today for the same character:
- U+2028 and U+2029 are refused in a file name and a note, but kept by the HTML importer's report text.
- U+061C is stripped in a note and in the CSS import report, but kept in the HTML import's report (`paint/mod.rs:72`, `report.rs:112`).
- An idempotency key containing U+202E is accepted by the tool check (P6). `activity-store/src/lib.rs:206` and `checkpoint-store/src/lib.rs:124` refuse the same text as a key prefix or key. I did not trace a call end to end, so treat "passes the tool check, fails the store check" as likely, not shown.

### 2.2 Caps are not shared either

- Plain-text check, bytes versus characters:
- `meldframe-activity/src/incoming.rs:59` `plain` counts `s.len()` (bytes).
- `meldframe-http-activity/src/tool.rs:124` `plain` counts `chars().count()`.
- The rule text is the same. A 128-character non-ASCII actor passes one and fails the other.
- 128 appears as:
- `MAX_ACTOR` (`activity/src/lib.rs:49`, bytes)
- `MAX_NAME_BYTES` (`server/src/limits.rs:15`)
- a literal in `meldframe-http/src/lib.rs:168`
- literals in `meldframe-http-capture/src/tool.rs:21,23,102,108`
- 100 appears as:
- `MAX_BATCH` and `MAX_KEY` (`mcp-defs/src/lib.rs:13,16`)
- `MAX_BATCH_OPS` (`server/src/limits.rs:21`, "the same number as the MCP batchLimit", kept equal by a comment)
- `MAX_KEY` (`checkpoint-store/src/lib.rs:27`)
- `MAX_KEY_PREFIX` (`activity/src/lib.rs:47`)
- The capture tool accepts a `nodeId` of up to 128 bytes of any non-control text (`tool.rs:108`). A node id in the file is at most 64 characters of `[A-Za-z0-9_-:.]` (`meldframe-check/src/validate.rs:59`, `Limits::max_id` 64).

### 2.3 Entry points, server side

| entry | where | check today |
|---|---|---|
| workspace and file name (REST, WS, MCP) | `meldframe-http/src/lib.rs:166` `valid_file`, then `apps/server/src/limits.rs:33` | two copies; P2 plus the slash rule |
| idempotency key | `mcp/tools.rs:70`, `http-variant:106`, `http-variables-import:131` | P6, three copies |
| batch note | `Note::parse`, `activity/src/note.rs:95` | P1, 500 characters, 3 breaks, combining-mark run cap |
| design review text | `design-review/src/lib.rs:148,196` | P2 plus control |
| checkpoint label, actor, key | `meldframe-checkpoints/src/label.rs:54,133`; `checkpoint-store/src/lib.rs:124` | P1 |
| batch header actor, tool, key prefix | `activity-store/src/lib.rs:192,206,212` | P1 |
| client message in a capture failure | `capture/src/error.rs:79` | P5, cut at 200 bytes |
| CSS text echoed in an import report | `variables-import/src/text.rs:17`; `html-import-tree/src/report.rs:108`; `html-import/src/paint/mod.rs:56` | P3, P4, P4 |
| token actor (dev tokens) | `meldframe-http/src/auth.rs:46` | non-empty only |
| node `name` and `text`, patch `name` and `text` | `meldframe-check/src/validate.rs:186-187,226-230` | length only |
| mode names | `meldframe-variables/src/check.rs`, `mode_name` | length only |

### 2.4 Entry points, app layer

| entry | where | check today |
|---|---|---|
| activity batch header (route or frame) | `RawHeader::check`, `activity/src/incoming.rs:59-` | P1 on actor, key prefix and tool; `Note::clean`; node names through `clean_name` (`summary.rs:26`) |
| checkpoint row | `meldframe-app-rules/src/checkpoints.rs:40` `clean` | P1 through `meldframe-checkpoints` |
| assets page | `AssetPage::is_valid`, `app-model/src/assets.rs` | allow-list of id form and 4 mime types; no free text |
| live op frame `Frame::Event {actor, key, command}` | `meldframe-app/src/frames.rs:46-100` | actor stored raw in `AppState.last_actor` (`frames.rs:98`) and used raw as a lane key (`activity/src/reducer.rs:156-170`); commands pass `meldframe-check` (length only) |
| `Frame::Rejected {code}` | `frames.rs:26`, `retry.rs:39` | `code: String` copied unbounded into `Event::OpRejected` |
| node names and text in the document | replica nodes | length only |

The app layer has a second check for the activity and checkpoint feeds, as ADR-0021 and ADR-0023 intended. It has none for op actors, lanes, node labels and refusal codes. The checks that exist call P1 only because the app crates happen to depend on `meldframe-activity` and `meldframe-checkpoints`.

### 2.5 Design notes for the one helper

- **Home.** It must be a small core module that every layer can depend on without a cycle and that stays portable.
- `meldframe-activity` cannot be it, because it depends on `meldframe-revert`.
- `meldframe-mcp-defs` cannot be it, because it depends on `meldframe-host` and `meldframe-batch`.
- `html-import-tree` has no dependencies today. `meldframe-protocol` (depends on nothing, already shared by every crate) or a `text` module of the core monolith would both work.
- **Contents.**
- `is_hidden_text` as the one set, plus the Cf ranges only P3 has (0600-0605, 06DD, 070F, 0890-0891, 08E2, 110BD, 110CD, 13430-1343F).
- `is_combining_mark`.
- `sanitize(&str, Profile) -> Cow<str>` (strip, cap in characters, collapse).
- `plain(&str, max_chars, max_bytes)`.
- The shared caps (`MAX_ACTOR`, `MAX_NAME`, `MAX_KEY`, `MAX_FILE_NAME`).
- One error type.
- **Two profiles are needed.**
- P1 removes U+200D (ZWJ) and U+FE0F (variation selector 16), so a layer name holding an emoji sequence (family, flag, heart with VS16) would be broken by the strict set.
- Identifiers, actors, keys and file names get the strict set.
- Display text copied from a node name gets a profile that keeps ZWJ and VS16 between emoji and strips bidi controls and tags.
- Document the choice once.
- **Where the app layer applies it.** Apply it where `AppState` and lanes are built: the `Frame::Event` actor, `Rejected.code`, lane keys, and label strings of nodes and modes. Do not apply it in the document, because node `text` is file content and must round-trip.
- **Server callers.** `valid_file`, `check_key`, `Note::parse`, `check_texts`, capture `clean` and the token parser all call the same helper. Then P2 to P6 and their copies disappear.

## 3. Duplicated helpers, with evidence

Method for exact copies: strip comments, drop the function name, and hash the token stream of every non-test function of 25 tokens or more. A group with the same hash in two files is a copy. On the workspace's Rust sources (tests excluded) this gives:
- 25 groups at 25 tokens (24 cross-crate, 1 in one crate)
- 38 groups at 15 tokens
- 20 groups at 40 tokens
- 11 groups at 60 tokens

The method misses renamed or restructured copies. D2 (partly), D4 and D11 were found by reading, not by the hash.

| id | helper | copies (file:line) | which should be shared | notes |
|---|---|---|---|---|
| D1 | invisible-character predicate | `meldframe-mcp-defs/src/lib.rs:20` and `apps/server/src/limits.rs:30` (verbatim, 99 tokens); `html-import/src/paint/mod.rs:72` and `html-import-tree/src/report.rs:112` (verbatim, 144 tokens); P1, P3, P5, P6 | `is_hidden_text` (`activity/src/note.rs:13`), moved to the shared text module | section 2 |
| D2 | hex colour check (`#` + 3, 4, 6 or 8 hex digits) | `meldframe-check/src/validate.rs:25`, `meldframe-variables/src/check.rs:39`, `meldframe-css/src/color.rs:21` (`is_hex`), `export/src/names.rs:92` (`hex_colour`), `meldframe-render/src/resolve.rs:277` (`is_hex_colour`), plus a separate parser at `resolve.rs:251` | `meldframe_css::color::is_hex` as the model (ADR-0024). `meldframe-check` and `meldframe-variables` do not depend on `meldframe-css`, so the home must be at or below `meldframe-variables` (the protocol module or the monolith) | five checks of one rule. Fixed (#313): `meldframe_css::color::is_hex` is the home (the crate has no dependency, so it is below `meldframe-check` and `meldframe-variables`; the protocol module was not needed); `meldframe-check`, `meldframe-variables`, `meldframe-render` and `export` call it, `[shared.hex_colour]` of `mm-check` bans a second copy |
| D3 | asset id (64 lowercase hex) | `meldframe-check/src/validate.rs:68-76`; `assets/src/lib.rs:132` and `meldframe-app-model/src/assets.rs:30` (both `is_asset_id`, identical, 56 tokens); `export/src/names.rs:98`; shard form `assets/src/disk.rs:133` | `assets::is_asset_id`, moved to the protocol module next to `ImageFill` | |
| D4 | id charset `[A-Za-z0-9_-:.]` with a length cap | `meldframe-check/src/validate.rs:59` (returns bool); `meldframe-variables/src/check.rs:71` (returns `VarError`); `meldframe-http-export/src/lib.rs:68` `valid_frame` (hard-coded 64) | one `valid_id(&str, max) -> bool` in the protocol module; each caller maps to its own error | capture `nodeId` (2.2) is a fourth, looser rule |
| D5 | CSS number reader (digits, `.`, sign, finite) | `html-import-style/src/value.rs:28` and `html-import/src/paint/lex.rs:41` (verbatim, 98 tokens); inline in `meldframe-css/src/length.rs:25-30`; with exponent in `meldframe-css/src/color.rs:73` `numeric` | `meldframe-css`, with a `number` module with and without exponent | |
| D6 | idempotency key check | `meldframe-http/src/mcp/tools.rs:70` and `meldframe-http-variant/src/lib.rs:106` (verbatim, 73 tokens); inline `meldframe-http-variables-import/src/lib.rs:131`; capture `nodeId` `tool.rs:108` | `meldframe-mcp-defs` already owns `MAX_KEY`; put `check_key` there on top of the shared text rule | none of them refuses invisible characters |
| D7 | file and workspace name rule | `meldframe-http/src/lib.rs:166` `valid_file`; `apps/server/src/limits.rs:33` `check` | one function in the shared text module; `valid_file` adds the slash rule | `limits.rs:54` says "the one place" |
| D8 | nodes touched by a command | `meldframe-batch/src/lib.rs:55` and `meldframe-host/src/room.rs:321` `touches` (verbatim, 138 tokens) | `meldframe-host` (batch already depends on host; make it `pub`) | the batch dry run says it counts "as `Room::accept` counts them"; if they drift, dry run and write disagree |
| D9 | sibling order comparator | `meldframe-order/src/siblings.rs:9` `sib_cmp`; `meldframe-layout/src/tree.rs:14`; `meldframe-render/src/resolve.rs:239` (65 tokens, 3 copies) | `meldframe_order::sib_cmp` (order depends only on protocol, so layout and render can depend on it) | |
| D10 | `is_layout` frame predicate | `export/src/decl.rs:71`; `meldframe-app-parts/src/layout.rs:8`; `meldframe-layout/src/tree.rs:8` (`is_layout_frame`) | `meldframe_layout::is_layout_frame` | |
| D11 | decimal number with one spelling per number | `meldframe-http-activity/src/review.rs:185` (allows `0`); `meldframe-http-checkpoints/src/state.rs:179` (no empty check); `meldframe-revert/src/revert.rs:50` (allows leading zeros); `meldframe-store-pg/src/lib.rs:67` | one `canonical_u32(&str)` in the shared module | the three differ on `0` and `01` |
| D12 | HTTP adapter glue (`error`, `failure`, `args`, `call`, `tool`, `now_ms`, `invalid`, `from`) | `meldframe-http` `mcp/tools.rs:25,38,70` and `http-variant:63,76,106` (verbatim); `failure` in `http-export:76`, `http-assets/src/tool.rs:52`, `http-variables-import:64`; `call` at `:212`, `:134`, `:233`; `http-checkpoints/src/routes.rs:83,92,98` and `http-activity/src/review.rs:99,108,114`; `http-review:259` and `http-activity/src/tool.rs:261` `call`; `now_ms` in `http-checkpoints:114` and `http-activity:150` | an `adapter` module inside the merged http crate | 12 of the 25 hash groups are here |
| D13 | HTML and CSS escape | single copy: `export/src/names.rs:42,78` | n/a (no duplicate found) | for completeness |
| D14 | colour keyword and function parsing | one implementation: `meldframe-css/src/color.rs`, used by `variables-import` and `html-import-style` | already shared; the model to follow | |
| D15 | store `db_i64` | `meldframe-store-pg/src/batches.rs:65` and `checkpoints.rs:49` | one `db.rs` | only same-crate copy |
| D16 | `fn node` test builders | `meldframe-app-harness/src/lib.rs:55` and `meldframe-store-contract/src/cases.rs:6` | a test-support module | |
| D17 | store `key` | `activity-store/src/store.rs:48` and `checkpoint-store/src/store.rs:31` | one `store_key` helper | |

## 4. deps-check: does it enforce the matrix?

I ran it (`deps-check: 127 crates, no violations`) and read `xtask/src/rules.rs`, `metadata.rs` and `main.rs`.

What it does correctly:
- Missing and unknown kind (L1).
- Feature crate without a `group`.
- The L2 matrix for every normal and build dependency among workspace packages. Optional dependencies count, and a renamed dependency is matched by package name.
- A feature crate importing a feature of another group without `composite` (L4).
- A UI toolkit in a kind that may not use one (L3), by a fixed list of 13 names.

The rules are a pure function with unit tests.

Gaps, each checked:

1. **TypeScript imports are invisible.**
 - The 17 `ui` packages (`packages/web-*`) are `Cargo.toml` stubs with `[lib] path = "rust/lib.rs"` and no dependencies (`packages/web-document/Cargo.toml`).
 - Their real edges go through `tsconfig` `paths` (`apps/web/tsconfig.json`) and `@meldframe/*` imports: 101 of `@meldframe/protocol`, 95 of `web-chrome`, 57 of `web-canvas`, 46 of `web-document`.
 - No `package.json` lists an `@meldframe/*` dependency.
 - So "ui to core, app, ui" is not enforced on 10,124 product lines, and U2 (no business state in UI crates) has no check at all.
2. **Dev-dependencies are exempt by design** (`metadata.rs:22`). I checked that no non-tool crate has a dev-dependency that would break the matrix today. The check would not stop one.
3. **`composite = [..]` is never validated.** No crate declares it today. A `group` on a non-feature crate, or a composite naming a group that does not exist, is accepted. All 8 feature-to-feature edges are inside one group (`html-import` x6, `export` x1, `variables-import` x1), so L4 holds today.
4. **Kind is not tied to location or role.**
 - 47 `tool` crates sit under `crates/core/`, and `meldframe-wasm` (ffi) is under `crates/app/`.
 - The 12 HTTP and Postgres adapters under `crates/integrations/` are declared `bin`. Kinds `engine`, `integration` and `os` have no crate.
 - As a result those 12 adapters (8,811 product lines) are outside `portable-check`, which is right for axum and sqlx. They are also outside the matrix, because `bin` may depend on everything (`rules.rs:155`).
 - `docs/architecture.md` says no kind exists for adapters yet. The `integration` kind does exist, but the matrix lets it depend only on core and engine, and these adapters depend on feature crates (`export`, `assets`, `design-review`). The matrix and the kinds disagree, and the disagreement is resolved by mislabelling.
5. **Three app-layer crates are declared `core`** (see 1.2). They cannot depend on feature crates, and `meldframe-app` does depend on the two dialog feature crates.
6. **Packages without a `Cargo.toml` are invisible** to deps-check and size-check: `packages/protocol` (37 hand-written lines plus generated types), `tools/fidelity-oracle` (329-line `fixtures.mjs`) and `experiments/fidelity-oracle`.
7. **M1, M2, M4 and G1 are not checked by `deps-check`.** They are enforced by tests and the `Permit` type (compile-fail tests in `meldframe-rooms`). That is acceptable, but the tool's name suggests more.
8. **Activity is a feature on the server only.** The `activity` cargo feature exists on the server, but `meldframe-app-model` and `meldframe-app` depend on `meldframe-activity` (core) unconditionally, so the web build cannot leave it out. M3 holds for the server build only.

## 5. size-allow.toml

`xtask/size-allow.toml` has the headers `[files]` and `[crates]` and no entry in either (verified). `cargo xtask size-check` reports no violations and notes the 7 crates above 2,500. For the 50,000 ceiling:

- **Entries that can be deleted:** none exist.
- **Entries that stay for the 500-line file limit:** none exist.
- The largest files are 500 (`meldframe-http-live/src/ws.rs`, `meldframe-sync/src/replica.rs`), then 499, 497, 497, 496 (`apps/server/src/http.rs`), 493 (`xtask/src/size.rs`), 490, 485, 485, 484, 484, 482 and 482.
- A file at 500 is allowed, because only more than 500 fails.
- **Crate entries become errors.** In `size.rs:judge`, a `[crates]` entry for a crate within its limit is a `Stale` failure. At 50,000 any `[crates]` entry would be stale, so remove the `[crates]` table, `Allow.crates`, the crate `Stale` and `Raised` handling, or keep the key inert.
- **Code and text to change:**
- `CRATE_LIMIT` 4000 to 50000 (`size.rs:16`).
- The test `limits_are_the_owner_decision_of_20_nov_2026` (`size.rs:318`), which pins `(500, 1500, 2500, 4000)`.
- The tests around `size.rs:294-300`, `305-310`, `326-345` and `366-383`, which use 4000 and 4001.
- The note text ("candidates for a planned split") must say that monoliths are exempt.
- The CI step names "500 lines per file, 4,000 per crate" in `.github/workflows/ci.yml:83` and `.gitea/workflows/ci.yml:73`.
- `docs/import-html.md:322`, `AGENTS.md:42` and `xtask/size-allow.toml:2`.
- **Stale text found:** `AGENTS.md:28` still says "a crate under 2,000", while `AGENTS.md:42` says 4,000.

## 6. Proposed `cargo xtask mm-check` (modular monolith rule)

Design: a new `xtask/src/monolith.rs` (about 350 lines, plus `monolith/tokens.rs`), wired as `Some("mm-check")` in `main.rs`. Run it in `just check` and `nightly.yml`, and the fast rules in `ci.yml`. Config goes in `xtask/monolith.toml`. The rules are pure functions over `CrateSrc { name, files: Vec<(path, text)> }` plus the `Package` list, unit-tested like `rules.rs`.

```
[monolith]            # crates above this many lines (tests included, same measure as size-check) are monoliths
threshold = 4000      # 2500 turns the single-seam note on for everything above the ideal band

MM1  root shape: for a monolith, src/lib.rs (or main.rs) holds only //! docs, `use`, `mod`, `pub mod`, `pub use`,
   attributes; at most 150 lines; no fn/struct/enum/impl/trait/const/static.
MM2  capability modules: every top-level `mod x;` is a file of at most 500 lines, or a directory with mod.rs that obeys
   MM1; a directory has at least 2 files; at most 40 top-level modules; each module file starts with a //! sentence
   naming the capability. `mm-check --list` prints the module list with that sentence.
MM3  no copies: tokenise every non-test fn (strip comments, drop the fn name), skip bodies under 25 tokens, hash the
   tokens; two files with the same hash fail, in any crate of the workspace. xtask/monolith.toml [allow_copy] lists
   "path::fn" pairs; entries only go down (`--base <ref>` rejects new ones, as size-allow does).
MM4  shared helpers: [shared.<name>] = { home = "path", ban = ["'\\u{202A}'..='\\u{202E}'", "0x202A..=0x202E",
   "b'0'..=b'9' | b'a'..=b'f'", "matches!(h.len(), 3 | 4 | 6 | 8)", "b'.' | b'-' | b'+'", "any(char::is_control)"] }.
   A banned pattern in any non-test .rs outside its home fails with "use <home>::<fn>".
MM5  module cycles: edges from `use crate::<mod>` between top-level modules of a monolith; a cycle fails.
MM6  tests: a crate named *-tests, *-props or *-scenarios with no src code fails unless [allow_test_crate] lists it
   (tests live in the crate they test); entries only go down.
Output: one line per violation (`crate: path:line: rule: message`), non-zero exit; unit tests per rule on made-up trees.
```

What would fail today, measured with a prototype of the MM3 hash and by grep:
- **MM1, MM2, MM5 at threshold 4,000:** nothing, because no monolith exists (the largest Rust crate is 2,524).
- **MM1, MM2, MM5 at threshold 2,500:** one crate, `meldframe-app-model`. It already has a 50-line mod-list `lib.rs` and 16 files under 500, so it passes.
- **MM1 after the merges:** it would bite. 21 of the 37 Rust crates of 600+ lines have items in `lib.rs`: `assets` (343 lines, 43 items), `meldframe-http-checkpoints` (307, 40), `design-review` (255, 37), `meldframe-checkpoints` (242, 35), `meldframe-server` (484, 34), `meldframe-http` (437, 34), `activity-store` (325, 30), `meldframe-store-pg` (451, 29). Each must move code into modules when merged. 15 files are at 480 lines or more (2 at exactly 500) and would need splitting before growth.
- **MM3:** 25 failures at 25 tokens (24 across crates), 38 at 15, 11 at 60. The first run would put them in `[allow_copy]`, and findings 2, 3, 7 and 8 then fix them.
- **MM4:** about 22 files hold a banned pattern outside a future home. That counts invisible-character sets in 6 files, hex colour in 5, CSS number in 3, asset id in 5 and control-only checks in 6, with overlaps.
- **MM6:** 44 test-only crates fail unless allow-listed.

Limits of the check, to state in its doc:
- MM3 finds exact copies only. It does not catch `valid_id` in two crates with different error types (D4).
- MM4 depends on the pattern list being kept up to date.
- None of it checks that a helper is good, only that there is one.

## 7. Findings

Priorities: P1 means a correctness or safety risk, or it blocks a v1.3 rule. P2 means needed to follow the standard. P3 means tidying. Sizes: S is under 200 changed lines, M is 200 to 1,000, L is over 1,000 (moves count, and the repository's 15-file and 1,000-line limits make L a stack).

| id | rule | evidence | proposed fix | size | priority |
|---|---|---|---|---|---|
| 1 | U9 (one helper, capped, at the app layer entry) | 6 predicates in about 12 places, 4 distinct sets (2.1); bytes in `incoming.rs:59`, characters in `http-activity/tool.rs:124`; no check on op actor (`frames.rs:98`, `reducer.rs:156`), `OpRejected.code` (`retry.rs:39`) or node names and text (`validate.rs:186`) | add the shared text module with two profiles and one error type (2.5); replace P2 to P6; apply it where `AppState` and lanes are built; add tests with U+202E, U+2028, ZWJ emoji and 4-byte text at each entry | M, a stack of 3 (module plus server and http; importers; app layer) | P1 |
| 2 | Duplicated rule: names and keys | `check_key` x3, control-only, against a file name with the invisible set (P6 vs P2); capture `nodeId` accepts 128 bytes, a node id is 64 chars of a charset (`tool.rs:108` vs `validate.rs:59`) | D4, D6, D7: one `valid_id`, one `check_key` (in `meldframe-mcp-defs` over the text module), one `valid_file`; capture uses `valid_id` | S to M | P1 |
| 3 | Duplicated rule that must not drift | `touches` in `meldframe-batch/src/lib.rs:55` and `meldframe-host/src/room.rs:321`, identical today | make host's `pub` and delete batch's; add a test that the dry run and `accept` agree on `ifSeq` for every `Command` variant | S | P1 |
| 4 | v1.3 modular monolith: app layer | 4 crates, 8,026 lines, three labelled `core` to get around app-to-app (`rules.rs:151`, `architecture.md` `app-parts`) | merge into `meldframe-app` (kind app) with directory modules per panel (`variables`, `assets`, `checkpoints`, `review`, `export`, `layout`, `link`); drop the three `core` entries | L, mostly moves (stack: model, parts, rules) | P2 |
| 5 | v1.3 modular monolith: importers | 4 crates in one group, 5,836 lines, with verbatim copies D1 and D5 caused by the split | merge into `html-import` (modules `tree`, `css`, `style`, `paint`); use `meldframe-css` and the text module; `meldframe-css` takes the number reader | L (stack of 2) | P2 |
| 6 | Tests outside the count | 44 test crates, 56,879 lines, none with product code; ADR-0011 "move tests out of the count: rejected"; manifest texts "split so that it stays under the ceiling" (`meldframe-import-html-tests`, `meldframe-gate-tests`, `meldframe-app-unit-tests`) | move each to `tests/` of the crate under test (shared harness crates stay: `app-harness`, `store-contract`); add MM6; supersede ADR-0011 and decision 35 | L (stack by area); `just wasm-test` lists crates by name and needs care | P2 |
| 7 | Duplicated helpers: colour, id, number, order | D2, D3, D5, D9, D10, D11 (section 3) | one home each: colour, asset id and id charset in the protocol module; number in `meldframe-css`; order in `meldframe-order`; `is_layout_frame` in `meldframe-layout`; canonical number in the text module | M | P2 |
| 8 | Duplicated adapter glue | D12: 12 of the 25 hash groups are in the HTTP crates | one `adapter` module in the merged http crate; each tool a module behind its cargo feature (keeps M3) | M | P2 |
| 9 | Kinds do not describe the code | 12 adapters `bin`, 3 app crates `core`, 47 `tool` under `crates/core/`, engine, integration and os unused (section 4, gap 4) | revisit the matrix (an `integration` kind that may depend on features, or document `bin` for adapters); tie kind to directory in `deps-check`; needs an owner decision first | S for the check | P2 |
| 10 | L2 and U2 on the web side | TS imports and state are outside `cargo metadata` (section 4, gap 1) | a `deps-check` step that reads each `ui` package's imports against the matrix, and a small lint for module-level mutable state in `ui` packages (allow-list, entries only go down) | M | P2 |
| 11 | deps-check minor gaps | dev-deps exempt, `composite` unvalidated, `group` on non-feature, fixed toolkit list, packages without a manifest (section 4, gaps 2, 3, 6) | validate `composite` against existing groups; fail a `group` on a kind that is not feature or feature-ui; count `tools/*` and manifest-less `packages/*` in size-check; optionally check dev-deps for L4 | S | P3 |
| 12 | D1 text and tooling for v1.3 | `CRATE_LIMIT` 4000 (`size.rs:16`); tests pinning 4000; CI step names; `AGENTS.md:28` says 2,000 and `AGENTS.md:42` says 4,000; `docs/import-html.md:322`; the mirror is v1.2 | set 50,000, keep the 500-line file limit, update tests and docs, bump the mirror to v1.3 with U9, remove the empty `[crates]` table | S | P2 |
| 13 | mm-check | section 6 | implement MM3 and MM4 first (they find real defects), then MM1, MM2 and MM5 with the merges, MM6 with finding 6 | M (about 350 lines plus tests) | P2 |
| 14 | Files at the limit | 15 files at 480 lines or more, 2 at exactly 500 (section 5) | split before the next addition into a directory module (`replica.rs`, `ws.rs`, `http.rs`, `size.rs`, `scan.rs`, `decl.rs`) | S each | P3 |
| 15 | Fat `lib.rs` | 21 of 37 crates of 600+ lines have items in `lib.rs` (section 6) | as each crate is merged, move code out so `lib.rs` is a module list | S each | P3 |
| 16 | M3 for the app layer | the `activity` feature is detachable on the server but not in `meldframe-app` (section 4, gap 8) | decide: state in the standard that app-layer state of a feature is not detachable at build time, or put the activity panel behind a cargo feature of the app crate | S (decision), M (if built) | P3 |

## 8. Appendix: all 127 packages

Columns: crate | kind | product | test | total | rs files under src / all counted files | biggest file and lines | flag (blank means 2,500 or under).

meldframe-web-e2e|tool|1|3544|3545|0/20|editor-e2e.mjs 485|>2500
meldframe-web-panels|ui|1014|2404|3418|0/34|image-fill-e2e.mjs 342|>2500
meldframe-web|bin|2419|925|3344|0/30|Canvas.svelte 457|>2500
meldframe-import-html-css-tests|tool|0|3234|3234|0/12|tailwind.rs 414|>2500
meldframe-web-document|ui|1159|1525|2684|0/35|edit.test.ts 382|>2500
meldframe-export-tests|tool|0|2537|2537|0/11|variables_export.rs 380|>2500
meldframe-app-model|core|2524|0|2524|16/16|action.rs 472|>2500
meldframe-app|app|2471|0|2471|20/20|app.rs 311|
meldframe-checkpoint-pg-tests|tool|1|2239|2240|1/8|checkpoint_state.rs 441|
meldframe-web-transport|ui|759|1458|2217|0/10|shell.ts 466|
meldframe-web-variables|ui|1000|1215|2215|0/15|manage-e2e.mjs 497|
meldframe-web-canvas|ui|1032|1151|2183|0/12|interaction.test.ts 397|
meldframe-server|bin|1970|196|2166|11/13|http.rs 496|
xtask|tool|1197|912|2109|9/9|size.rs 493|
meldframe-app-layout-tests|tool|0|2108|2108|0/10|layout_drop.rs 401|
meldframe-sync|core|1231|827|2058|8/12|replica.rs 500|
meldframe-check|core|975|1020|1995|7/14|validate.rs 280|
meldframe-rooms|core|564|1429|1993|5/20|registry.rs 399|
meldframe-web-shell|ui|853|1137|1990|0/21|layout.test.ts 268|
meldframe-host|core|1221|764|1985|8/13|gate.rs 425|
meldframe-core|core|379|1576|1955|3/15|sibling_order.rs 301|
meldframe-server-tests|tool|1|1949|1950|1/13|pg_workspace.rs 356|
meldframe-http|bin|1939|0|1939|10/10|lib.rs 437|
html-import-css|feature|1912|25|1937|8/8|mod.rs 465|
meldframe-app-unit-tests|tool|0|1913|1913|0/9|document.rs 478|
meldframe-mcp-tests|tool|1|1911|1912|1/16|surface.rs 317|
meldframe-http-tests|tool|1|1905|1906|1/10|ws.rs 410|
meldframe-activity-tests|tool|1|1883|1884|1/13|reducer_hostile.rs 277|
meldframe-history|core|513|1324|1837|5/16|groups.rs 202|
meldframe-live-tests|tool|2|1789|1791|1/8|mod.rs 470|
meldframe-protocol|core|1271|509|1780|10/15|wire_compat.rs 329|
meldframe-variables-tests|tool|0|1774|1774|0/11|check.rs 340|
meldframe-variables-import-tests|tool|0|1773|1773|0/13|scan.rs 419|
meldframe-sync-scenarios|tool|1|1759|1760|1/11|pending_undo.rs 302|
meldframe-sync-props|tool|2|1749|1751|1/9|mod.rs 476|
meldframe-activity|core|1722|0|1722|11/11|reducer.rs 459|
meldframe-app-tests|tool|0|1704|1704|0/7|robust.rs 322|
meldframe-activity-pg-tests|tool|1|1669|1670|1/7|pg_batches.rs 482|
meldframe-import-html-tests|tool|0|1666|1666|0/6|tree.rs 416|
meldframe-layout-tests|tool|0|1624|1624|0/10|findings.rs 305|
variables-import|feature|1599|0|1599|7/7|scan.rs 490|
meldframe-store-pg|bin|1524|69|1593|7/9|lib.rs 451|
meldframe-revert|core|711|880|1591|5/8|revert.rs 447|
export|feature|1578|0|1578|7/7|decl.rs 485|
html-import-style|feature|1568|0|1568|8/8|style.rs 437|
meldframe-app-rules|core|1546|0|1546|11/11|drop.rs 401|
meldframe-app-inspector-tests|tool|0|1525|1525|0/4|fills.rs 499|
meldframe-web-activity|ui|785|735|1520|0/16|panel.test.ts 277|
meldframe-approvals|core|617|880|1497|4/10|approvals.rs 449|
meldframe-app-parts|core|1485|0|1485|13/13|property.rs 275|
meldframe-wasm|ffi|334|1147|1481|2/8|bridge.rs 394|
meldframe-stream|core|749|697|1446|3/6|lib.rs 327|
meldframe-render|core|613|818|1431|4/8|resolve.rs 378|
meldframe-web-variables-io|ui|563|864|1427|0/13|io-e2e.mjs 427|
meldframe-variables-rename-tests|tool|0|1417|1417|0/5|replica.rs 408|
meldframe-capture-tests|tool|1|1376|1377|1/6|mod.rs 413|
assets|feature|727|544|1271|3/5|lib.rs 343|
meldframe-app-export-tests|tool|0|1267|1267|0/6|variables_import.rs 357|
meldframe-app-review-tests|tool|0|1265|1265|0/4|checkpoints.rs 460|
meldframe-import-html-style-tests|tool|0|1245|1245|0/6|computed_vars.rs 326|
html-import|feature|1215|0|1215|9/9|gradient.rs 253|
meldframe-variables|core|1201|0|1201|9/9|check.rs 425|
meldframe-variables-mcp-tests|tool|0|1196|1196|0/5|import.rs 359|
capture|feature|611|581|1192|5/8|relay.rs 403|
html-import-tree|feature|1141|0|1141|7/7|build.rs 372|
meldframe-import-html-paint-tests|tool|0|1138|1138|0/7|gradient.rs 300|
meldframe-variables-panel-tests|tool|0|1084|1084|0/4|panel.rs 497|
meldframe-variables-manage-tests|tool|0|1051|1051|0/5|remove.rs 361|
meldframe-undo|core|1042|0|1042|7/7|remote_vars.rs 238|
meldframe-review-tests|tool|0|1019|1019|0/4|design_review.rs 406|
meldframe-web-link|ui|391|602|993|0/10|engine.wasm.test.ts 251|
activity-store|feature|990|0|990|3/3|contract.rs 484|
meldframe-web-chrome|ui|652|327|979|0/19|forms.test.ts 147|
meldframe-web-tokens|ui|600|365|965|0/10|tokens.ts 228|
meldframe-http-activity|bin|964|0|964|6/6|tool.rs 271|
meldframe-web-export|ui|418|526|944|0/14|export-e2e.mjs 159|
meldframe-http-checkpoints|bin|927|0|927|4/4|routes.rs 307|
meldframe-variables-props|tool|2|913|915|1/4|mod.rs 482|
meldframe-http-live|bin|863|0|863|3/3|ws.rs 500|
meldframe-gate-tests|tool|0|837|837|0/6|gate_quotas.rs 309|
meldframe-checkpoint-tests|tool|1|781|782|1/5|roll.rs 360|
meldframe-web-tools|tool|113|636|749|0/9|ui-literals.test.mjs 179|
meldframe-web-feedback|ui|308|383|691|0/15|feedback.test.ts 96|
checkpoint-store|feature|688|0|688|3/3|contract.rs 374|
design-review|feature|623|64|687|3/4|lib.rs 255|
meldframe-activity-app-tests|tool|0|679|679|0/3|healing.rs 410|
meldframe-layout|core|679|0|679|8/8|place.rs 180|
meldframe-activity-review-tests|tool|0|672|672|0/4|setter.rs 391|
meldframe-http-capture|bin|662|0|662|5/5|hub.rs 202|
meldframe-assets-http-tests|tool|0|661|661|0/2|routes.rs 394|
meldframe-checkpoints|core|640|0|640|4/4|lib.rs 242|
meldframe-http-assets|bin|584|16|600|4/4|lib.rs 346|
meldframe-module|core|332|259|591|3/5|registry.rs 262|
meldframe-snapshot-tests|tool|2|575|577|1/3|snapshot.rs 312|
meldframe-web-paint|ui|249|292|541|0/9|shadow.test.ts 131|
meldframe-history-props|tool|2|537|539|1/5|reparent_ancestor.rs 188|
meldframe-order|core|297|232|529|3/5|fracindex.rs 257|
meldframe-activity-tool-tests|tool|0|524|524|0/2|get_batches.rs 330|
meldframe-css|core|302|174|476|3/5|color.rs 251|
meldframe-import-html-roundtrip-tests|tool|0|469|469|0/1|roundtrip.rs 469|
meldframe-store-contract|tool|359|104|463|2/4|cases.rs 305|
meldframe-variant|core|151|296|447|1/2|variant.rs 296|
meldframe-web-assets|ui|178|235|413|0/3|assets.test.ts 235|
meldframe-oracle-runner|tool|263|146|409|3/4|lib.rs 153|
meldframe-variables-limits-tests|tool|0|402|402|0/2|limits.rs 295|
meldframe-export-http-tests|tool|0|371|371|0/1|export.rs 371|
meldframe-http-variant|bin|341|0|341|1/1|lib.rs 341|
meldframe-mcp-defs|core|332|0|332|2/2|view.rs 175|
meldframe-calls|core|219|98|317|1/3|lib.rs 219|
meldframe-web-input|ui|148|138|286|0/3|hostinput.ts 147|
meldframe-app-harness|tool|281|0|281|1/1|lib.rs 281|
meldframe-http-review|bin|269|0|269|1/1|lib.rs 269|
meldframe-http-snapshot|bin|266|0|266|1/1|lib.rs 266|
xtask-licences|tool|159|98|257|1/1|lib.rs 257|
meldframe-batch|core|246|0|246|1/1|lib.rs 246|
meldframe-http-variables-import|bin|242|0|242|1/1|lib.rs 242|
export-dialog|feature|235|0|235|2/2|lib.rs 189|
meldframe-serve-tests|tool|0|232|232|0/1|serve.rs 232|
meldframe-http-export|bin|229|0|229|1/1|lib.rs 229|
meldframe-live-slot-tests|tool|0|227|227|0/1|wsslots.rs 227|
meldframe-mcp-asset-tests|tool|0|211|211|0/1|upload_asset.rs 211|
variables-import-dialog|feature|165|0|165|1/1|lib.rs 165|
meldframe-ratelimit|core|138|0|138|1/1|lib.rs 138|
example|feature|23|89|112|1/2|registration.rs 89|
meldframe-ui|ui|15|0|15|2/2|tokens.rs 11|
capture-ui|feature-ui|1|0|1|1/1|lib.rs 1|
example-ui|feature-ui|1|0|1|1/1|lib.rs 1|

Note on the table: "product" for `meldframe-web-e2e` is 1 because its scripts are classed as tests. `xtask` product counts exclude its inline test tail.