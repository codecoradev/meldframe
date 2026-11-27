---
status: accepted
---
# Rune front end: a native desktop app and a web build from the same view crates, the React web client frozen

Accepted by owner decision on 27 Nov 2026 (decision 45 in `docs/decisions.md`; the work is planned as epic E8 in `docs/epics.md`). The date of the decision is the date of this record. It supersedes part 3 of ADR-0026 (the desktop on GPUIX with React) and its part 2 (the web client moves to React); the rest of ADR-0026 stands.

## Context

ADR-0026 (25 Nov 2026) put the UI behind a thin app layer, moved the web client from Svelte to React and planned the desktop client on GPUIX with React after the web client was fixed. Since then:

- The thin UI work of E7.1 and E7.2 and the moves of decision 43 (the hit test, the open state of dialogs and overlays, text editing, the PNG plan, the retry rules) put the rules of the editor into `meldframe-app` and the core. A front end that reads state, dispatches actions and carries out effects is now a realistic plan, not only a rule.
- E7.3 has merged slices S1 to S11 of `docs/react-web-plan.md` (the primitives, the dialogs, the panels and the canvas, each behind a switch with the Svelte page as the default). Slice S12 (the shell flips to React) and slice S13 (Svelte is removed) are not started.
- A spike of the Rune web target ran in seven steps on Rune revision `048528dc35f0`. It is not part of this repository and is not linked here. It measured: a wasm build of 4.4 MB gzip with the app layer linked; 4 frames in 180 s at rest, all at start; a first wheel event after idle that presents about 2.1 s late (upstream issue anomalyco/rune#2549); stalls of about 430 ms on Firefox on the path that draws one Shape; a p95 of 100 ms on the path of elements; and about 10 frames per second on a mid-range phone when the Shape is rebuilt every frame. The spike also found that resolving the layout on every frame is the one thing a Rune front end must not do. These numbers are the spike's, not measured again in this repository.
- Rune (`anomalyco/rune`, MIT) is pre-1.0, is used by git revision only, needs 8 `[patch.crates-io]` git entries (wgpu-hal, naga, winit, sctk-adwaita, parley_engine, accesskit_windows, accesskit_atspi, accesskit_macos), takes issues and not pull requests, and has one dominant maintainer.

## Decision

The owner decided on 27 Nov 2026:

1. **The desktop client is native Rune.** It is a Rust application that links `meldframe-app` and the core as libraries and draws with Rune. This supersedes the GPUIX with React plan of decision 42 part 3 and E7.4; decision 5 (Tauri 2) stays superseded for the desktop, now by this decision.
2. **The web client moves to Rune as well**, built for `wasm32` from the same `ui` and `feature-ui` crates as the desktop. Mobile is switched off for now. The web build starts after the desktop alpha, and the old web clients are removed only after the gates of E8.3 and E8.4 pass and the owner says so.
3. **The React web client is frozen.** Slices S1 to S11 of E7.3 stay merged; S12 and S13 are on hold. Svelte and React stay in the repository for now, the default page stays Svelte, and a check (E8.0.3) keeps them from growing: bug and security fixes stay possible inside an allow-list that only goes down. New rules of the editor are written in Rust at the app seam and the frozen clients are not rewired for them (the default of the plan; each legacy copy is listed for retirement in E8.6.1).
4. **The server stays central and the protocol is the only contract between a client and the product** (`/v1/ws`, REST, MCP). The thin UI (rules U1 to U9 of `docs/standards/modular-thin-ui.md`, and U10 to U12 of its version 1.4, which E8.0.2 mirrors: the toolkit swap stays cheap, one view codebase for Rune, the client kind comes from the credential) keeps every front end choice reversible.

Defaults the owner accepted with it:

- **A second Cargo workspace** under `frontends/rune/`, with its own `rust-toolchain.toml`, `Cargo.lock` and `deny.toml`. It takes path dependencies into the main workspace for the core, app, protocol and render crates only; no crate of the main workspace depends on it. The main workspace keeps its toolchain (1.97.0, wasm-bindgen 0.2.130) and its `deny.toml` (`unknown-git = "deny"`).
- **One Rune revision, `048528dc35f0`**, written in one place. A bump is its own pull request and runs the golden images, the element tree dumps and the frame benchmarks before it merges. A CI check fails a manifest that names a git source without `rev =`. Who owns bumps is not decided here.
- **An exception for a git source, scoped to that workspace**: its `deny.toml` lists the exact git URLs and revisions in `allow-git`, every patched crate's licence is checked and on the allow list, and `cargo xtask licences --check` lists every crate the workspace reaches.
- **Desktop first, on macOS, with a written spike result before any alpha**: E8.2 states its exit criteria (C1 to C10, covering box and text fidelity, paint, frame cost, the hit test, idle cost, input methods, accessibility, the upstream first wheel defect and dependencies) before any code, and a pass is input to the owner's decision, not the decision.
- **Chromium and WebKit as the web browsers.** Firefox shows an unsupported page naming the reason; so does a mobile device. Both pages are a courtesy. **Mobile cannot be blocked by credential**: a phone browser and a desktop browser carry the same web credential. The real controls are the token, the actor, quotas, rate limits per credential and revocation, which apply to every kind of client.

## Options considered

- **GPUIX with React for the desktop and React for the web** (decision 42). Rejected. It leaves two view codebases, and the desktop renderer rests on forks this project does not control (ADR-0026, Risks: whole-pixel text that wrapped differently in 2 of 12 widths against a 0.05 px contract, no rounded clip upstream, every element rewritten for pan and zoom). A React web client does not by itself give a desktop client.
- **React for the web and Rune for the desktop, as two front ends.** Rejected. Each view, state of a control and focus rule would be written and tested twice, and the React work of S12 and S13 would still have to finish. The thin UI would hold the rules once, but the views would not.
- **Rune everywhere, from shared view crates** (chosen). One view codebase, and the desktop is the first place where it is tested on a real GPU path. It rests on a pre-1.0 toolkit used by git revision, which the gates and tripwires below bound, and which the thin UI lets this project leave.

## Consequences

- Two workspaces. CI has a job for the Rune workspace (not a required check until the desktop spike passes; then the owner decides). `cargo xtask deps-check`, `ui-literals`, `size-check` and `mm-check` read both workspaces (E8.0.4). Rune toolkit code is imported only in `ui`, `feature-ui` and `bin` crates. The wasm-bindgen CLI is installed per workspace and equals the version of that workspace's lock.
- The exception for a git source is reviewed, and removed if it can be, when any of these happens: a tagged Rune release; a frozen API; input methods verified on each shipped platform; anomalyco/rune#2549 fixed; the 8 patches upstreamed or replaced by releases on crates.io. Each upstream issue is posted only with the owner's approval of its wording.
- Thin UI work comes first (E8.1) and needs no Rune: key input, creation and resize, the PNG plan, the camera and the pointer state machine, the shortcut table and palette entries, a cached scene with an epoch, the text measurer seam, the client kind in the credential, and a test that a toolkit swap touches no core, engine, integration, os, feature, app or ffi crate. E8.1 reverses the answers of decision 43 that kept the shortcut table and palette (Q2) and the camera and pointer machine (Q9) in TypeScript, for the second front end. The frozen clients keep their TypeScript copies until E8.6.
- **Fallback.** Because the UI is thin, the fallback is a front end swap and not a rewrite of the product: the frozen React client (resume S12 and S13) and the Svelte page stay until the owner retires them, and any desktop front end can speak the same protocol.
- **Tripwires** stop the Rune track at the spike or the alpha, with the fallback named and not a silent slip: input method composition fails on macOS with real input methods; the frame cost of the reference scenes is not met on a real GPU; the fidelity contract (0.05 px boxes, 1 px text ink, `docs/fidelity-contract.md`) cannot be met by painting from the core's own measure; a fork of Rune is needed beyond the 8 pins; Rune API churn costs more than one working day a bump over three bumps in a row.
- The exit criteria of the desktop spike are written in E8.2 of `docs/epics.md`, before any code.
- What the owner has not decided: who owns Rune bumps; the desktop sign-in design (the proposal is a code flow with PKCE and a loopback redirect through Gatehouse, with the token in the system keychain, in a server ADR that extends ADR-0022 and needs Gatehouse support); the distribution identities (signing, update host, accounts), none of which is created without an owner order; and the time to interactive bar of the web build.
- Costs accepted: two workspaces double part of the CI cost; the frozen clients drift from the Rust rules, bounded by the freeze check and by E8.6; a distributed desktop client brings abuse and support work, so the client kind in the credential and the central controls (E8.1.8, E8.5.5) come before any public build.
- ADR-0008 stands: the app layer compiles for the legacy web client through `meldframe-wasm`, and a Rune front end links it directly.
