# React web client: plan (E7.3, issue #308)

Status: draft for owner review, 25 Nov 2026. Nothing here is decided until the owner answers the questions in section 9. Slice S0 (toolchain) and slice S1 (the pilot: primitives, Tooltip and Menu on Base UI, `/?states&react`) are written; their results are in sections 5, 7 and 8. Scope is the web client only: the desktop client (#309, E7.4) is not planned here and waits for the web client to be fixed (decision 42, ADR-0026).

This note is the first box of E7.3 in `docs/epics.md`: the options, the choice and why, the component library that replaces Bits UI, how the Pixi canvas is mounted, the test strategy and the order of panels with sizes. Facts about the current code come from `docs/audits/2026-11-25-ts-thin-ui.md` (section 3 is the Svelte surface) and were re-checked against the repository on 25 Nov 2026.

## 1. What changes and what stays

Changes: the 60 `.svelte` files (4,836 lines; 4,677 product, `TestHost` 10, `Harness` 149), Bits UI, the Svelte checks and lint, the Svelte parts of the literal scan, and the 21 + 4 tests that need a Svelte runtime or read `.svelte` source.

Stays, as ADR-0026 says: the Rust app layer and the WASM bridge, the framework-free TypeScript (about 7,000 lines of rules and view models, canvas, paint, input, transport, tokens), Tailwind CSS v4 and the tokens, the gate and the events, and the 30 browser scripts under `packages/*/scripts` as the parity check (their 234 `data-testid` values are kept).

## 2. Tailwind stays

Yes. Tailwind v4 is applied by `@tailwindcss/vite` (`apps/web/vite.config.ts`), which is not tied to a framework. Tailwind reads source files as plain text and scans every file that is not git-ignored, so `.tsx` files need no configuration (source: tailwindcss.com/docs/detecting-classes-in-source-files). What the port does to it:

- `class=` becomes `className=` (476 strings in `.svelte` today); the strings themselves are portable.
- A class name must exist in full in the source. Svelte's `class:` directive and string building are not used for colours today; in React a class is never built from a prop (`bg-${x}`), a prop is mapped to complete strings. The review checklist says so; a lint rule is optional.
- The `@source` list in `packages/web-tokens/src/tailwind.css` (the packages that draw with classes) stays as it is. No new package is added for the port (view files sit beside their model files, as U3 is practised today).
- The 25 `style:` directives become `style={{ ... }}` objects and the one `<style>` block (`Canvas.svelte`) becomes a CSS file. Colours, font sizes and durations still come from the tokens module (U4).
- `@custom-variant hov` and the `data-preview` states used by `/?states` are CSS and do not change.

## 3. Facts that size the work

Svelte lines by package (sum 4,836):

| Package | Svelte lines | Notes |
|---|---|---|
| `apps/web` | 1,168 | `Editor` 374, `Canvas` 457, `App` 115, `TextEditor` 73, `Harness` 149 (dev) |
| `web-panels` | 891 | 13 files; `BindPicker` uses Bits `Popover` |
| `web-variables` | 641 | 7 files; `ManageDialog` uses Bits `Dialog` |
| `web-chrome` | 554 | primitives; `Menu`, `Tooltip` use Bits; `StateMatrix` is the `/?states` page |
| `web-activity` | 432 | 5 files |
| `web-shell` | 430 | `Layout` 165 (four snippets), `Palette` (Bits `Dialog`), `LayoutDemo` |
| `web-variables-io` | 344 | two dialogs (Bits `Dialog`) |
| `web-export` | 227 | dialog (Bits `Dialog`) |
| `web-feedback` | 149 | banners, empty states, toast |

Rune and template surface: 76 `$state`, 84 `$derived`, 23 `$effect`, 19 `untrack`, 35 snippets, 25 `tick()` focus handoffs, 23 `bind:this`, one `bind:` of substance (`camera` on `Canvas`). There are no stores, transitions, actions, context or `{@html}`, so none of those needs a port. Bits UI is used in 10 files for four families (Dialog in 5, DropdownMenu, Tooltip, Popover; 53 element uses), with two Bits-and-Svelte-specific helpers (`rememberFocus` and `takeFocusBack` in `web-chrome/src/focus.ts` sit on top of Bits; `composeProps` exists only to merge Bits trigger props with Svelte attachments).

How the page reads state today: `engine.snapshot()` returns the current snapshot and `engine.subscribe(fn)` returns an unsubscribe (`packages/web-link/src/engine.ts`); `Editor.svelte` keeps `snap = $state.raw(engine.snapshot())` and updates it from `subscribe`. The engine replaces the snapshot only when something changed (a heartbeat leaves it as it is). That is exactly the shape `useSyncExternalStore(subscribe, getSnapshot)` asks for.

## 4. Options for the shape of the migration

| Option | What it is | For | Against |
|---|---|---|---|
| A. Rewrite on a branch | One long-lived branch, switched at the end | One toolchain at a time | Breaks the rule of small pull requests, no parity check until the end, the editor changes under it (it is not frozen), large merge risk |
| B. React islands inside the Svelte shell | Panels and dialogs become React roots mounted from `Editor.svelte` and `Layout.svelte` snippets; the shell goes last | Panel by panel, each slice small and shippable; both versions can be run against the same browser script; no freeze | Two frameworks and two plugins in one bundle for a while; leaf primitives (`Button`, `Field`, ...) must exist twice until the last Svelte consumer is gone; two portal and focus systems at once (Bits and the new library) |
| C. New app directory with a parity list | `apps/web-react` beside `apps/web`, switched when the list is green | Clean toolchain | Two editors to keep in step while features keep landing (every change twice), parity only at the switch |
| D. React shell with Svelte islands | The reverse of B | The composition root moves early | The most coupled file (`Editor.svelte`, 374 lines: camera, preview, text edit, overlay, toast) moves first, and Svelte `mount()` inside React is the less trodden path |

Proposed: **B**, leaf panels first and the shell last. Reasons: the audit found no framework import outside view files and no store or context to replace, so an island needs only the engine, which both frameworks read through the same `subscribe` and `snapshot`; the dialog open state is app state (`state.export.dialog`, the variables dialogs "open while the app holds the text"), so no state crosses between frameworks; `Layout.svelte` already takes its panels as snippets, so a React root fits a snippet.

Transitional duplication, with an end: the primitives of `web-chrome` (about 380 lines of `Button`, `Checkbox`, `Field`, `Icon`, `IconButton`, `Section`, `Segmented`, `Tabs`) get a React copy first, and the Svelte copy goes with the last Svelte file that uses it. The class strings and the logic files (`focus.ts`, the view models) are shared, so only the markup is twice. The `/?states` page checks both copies in six states and two themes.

Optional switch (question 2 in section 9): while a panel exists twice, a query parameter such as `/?react=export,activity` picks the React version, so the same browser script runs on both before the Svelte one is deleted. It is a temporary switch like `/?canvaslayout=`, removed in the last slice.

## 5. Component library that replaces Bits UI

The product uses four families: Dialog (5 places), Menu, Tooltip, Popover. Tabs, Segmented, Checkbox, Field and Switch are hand-written or native already. UI language is English (decision 4), so translated strings, calendars, tables and trees are not needed.

| Library | Fit | Notes |
|---|---|---|
| Base UI (1.x) | Covers the four families; unstyled, part-based like Bits (Root, Trigger, Portal, Popup, Title, Description, Close); MIT; 37 components, 5 direct dependencies, tree-shakable | 1.0 in December 2025, so younger (10 minor releases by October 2026); no translated strings (not needed); the default of shadcn/ui since July 2026; needs a stacking context on the app root for portals (its quick start) |
| Radix Primitives | Same families, the most downloaded, mature | Not deprecated; shadcn/ui still ships both; the part API Base UI copies |
| React Aria Components | Strongest accessibility record and tests, hooks layer | Larger surface; its extra strength (dates, colours, tables, trees, 32 languages) is unused here |

Proposed: **Base UI**, with Radix as the fallback if the pilot dialog fails the focus checks of section 7. The reasoning is fit and size, not popularity: the mapping from Bits is close to mechanical, and only four families are used. Caveat on the evidence: the comparison I read (untitledui.com, October 2026) is by a vendor that builds on React Aria, and its numbers are as of that month; the plan does not depend on them beyond "Base UI covers Dialog, Menu, Popover, Tooltip" and the pilot checks the rest. Focus return after a close stays our own helper (`rememberFocus`, `takeFocusBack`), rewritten against the new library.

Evidence from the pilot (S1, 25 Nov 2026; Tooltip and Menu only, the Dialog family waits for S2). `@base-ui/react` 1.9.0 is installed with an exact pin; it adds `@base-ui/utils` 0.5.0, `@floating-ui/react-dom` 2.1.9, `@babel/runtime` 8.1.3, `reselect` 5.3.0 and `use-sync-external-store` 1.7.0, all MIT, and `npm run licences` passes with the policy unchanged. Documentation relied on: the Tooltip and Menu pages of base-ui.com (the provider's `delay` and `timeout`, the trigger's `render` prop, the popup's `finalFocus`), and the type declarations of the installed package where a page was silent. `packages/web-chrome/scripts/overlays-e2e.mjs` puts the Svelte copies (Bits UI) and the React copies (Base UI) through the same assertions in Chromium and both pass:

- Tooltip: opens on hover about 400 ms after the pointer entered (read in the page: 405 ms Svelte, 414 ms React), waits again after a pause, opens at once on a neighbour inside the 300 ms skip window, shows the label and the shortcut in the mono face, 8 px from its trigger, closes on leave and on Escape, opens on keyboard focus, and is gone while the menu of the same button is open. The trigger keeps its own `aria-label`.
- Menu: opens by click (the popup takes the focus), Enter, Space and ArrowDown (first row highlighted and focused); ArrowDown, ArrowUp, Home, End and a typed letter move and wrap; Enter or a click on a row calls its handler, closes and returns the focus to the button; Escape closes and does the same; an outside click closes; `opensDialog` leaves the focus where the dialog took it (`finalFocus` returning false); placement 8 px from the button with `side` and `align`; `data-owns-keys` on the popup.
- Gap 1, a disabled row stays in the arrow-key order. Base UI keeps a disabled item focusable (the menu root passes an empty `disabledIndices`; `MenuItem` has no prop to change it) and Bits UI skips it. Workaround, in `Menu.tsx`: a disabled row is drawn by hand as a `role="menuitem"` with `aria-disabled` that is not a `Menu.Item`, so no arrow, hover, typeahead or click reaches it (a reader still finds it named and disabled). The script fails without it. It departs from the ARIA practice of leaving a disabled item reachable; it keeps what the Svelte menu does today.
- Difference accepted, not worked around: after a click outside, Base UI returns the focus to the trigger, Bits UI leaves it on the page.
- Nothing needed Radix. The fallback stays open for S2: the Dialog family is not checked yet.

## 6. State, the canvas and tests

**State.** One hook, `useAppState(select)`, wraps `useSyncExternalStore(subscribe, getSnapshot)` over the engine. Rules from the React reference (react.dev/reference/react/useSyncExternalStore): the snapshot must be immutable and the same value while nothing changed (the engine already guarantees it); `subscribe` is declared outside the component, or it resubscribes at each render; no suspending on a store value. React holds no editor state; `useState` is only for view state that the audit leaves in the page (hover, open menu, a field's draft). Findings T10 (#323) and T11 (#324) (view state with rules) move to the app layer first, which shrinks what React has to hold.

**Pixi canvas.** Mount the existing framework-free canvas from a ref in an effect, as `Canvas.svelte` does today, and keep `camera` as a controlled prop plus a callback. Do not use `@pixi/react` (declarative JSX over Pixi): the scene comes from the app layer as data (`App::scene` through `sceneFromBoxes`) and a second reconciler over it would hold drawing rules twice, against U1. For the record, `@pixi/react` is production-ready, supports React 19 and Pixi 8, and its `useApplication` works only in a child of `<Application>` (its README); one project's changelog mentions turning React strict mode off to avoid problems with it (snippet only, not read in full, so not relied on). Effects must be idempotent anyway, since React's strict mode runs them twice in development.

**Tests.** Unit tests keep `environment: 'node'`: the 21 tests that render with `svelte/server` are rewritten against `renderToStaticMarkup` of `react-dom/server`, a like-for-like change. The 4 tests that scan `.svelte` source text (`ownskeys`, `textedit`, `shortcuts`, `feedback`) are rewritten to scan `.tsx`, or better replaced by the boundary check. Interactive behaviour (focus, keyboard, dialogs, drag) is covered only by the Playwright scripts, run by hand: they stay the parity check, and every slice records in its pull request which script ran against which version and the result. Not verified: Testing Library with React 19 under Vitest's jsdom or happy-dom (the pages I found were generic); it is not needed for the first slices and is a question for the slice that first wants a non-SSR unit test.

## 7. Toolchain and the pilot

Toolchain (slice 0, no change a person can see; what S0 does, with what reality showed):

- React 19.3.1 and `react-dom`, `@types/react` and `@types/react-dom`, and `@vitejs/plugin-react` 6.3.0 (it targets Vite 8, uses Vite's own JSX transform and needs no Babel; the React Compiler is not used). It is given `include: /\.tsx$/`, so `svelte()` takes `.svelte` and `react()` takes `.tsx` and the two never see the same file. The packages under `packages/` have no `node_modules` of their own, so `resolve.dedupe: ['react', 'react-dom']` makes Vite find React from `apps/web` (without it the dev server and `vite build` fail on the first React file of a package, while Vitest does not notice). `apps/web/src/react-build.test.ts` builds a throwaway page against the real `vite.config.ts` to keep it so.
- One `tsconfig.json`, not a second one: `"jsx": "react-jsx"`, `.tsx` in `include`, and the `paths` of `react` and `react-dom` (as `bits-ui` has them). `svelte-check` reads `.ts` and `.tsx` files from that config as well, so `npm run typecheck` covers both with no extra `tsc` step and the 18 hand-kept aliases are not copied into a second file. This corrects the earlier text, which proposed `tsc --noEmit` beside `svelte-check`.
- `eslint-plugin-react-hooks` 5.2.0, exact, for `**/*.tsx`: only `rules-of-hooks` and `exhaustive-deps`, both as errors (the compiler rules of version 7 assume the React Compiler). Version 7.1.1 supports ESLint 10 but depends on `@babel/core`, whose `caniuse-lite` (CC-BY-4.0) is not on the allowed set of `npm run licences`; 5.2.0 has no dependencies, declares ESLint up to 9, and works on ESLint 10.15 through an `overrides` entry in `apps/web/package.json` (`"eslint-plugin-react-hooks": { "eslint": "$eslint" }`); a conditional hook call and a missing effect dependency were both reported on a throwaway file. Move to 7.x when the licence question is settled.
- `eslint-plugin-jsx-a11y` is not installed: 6.10.2 declares ESLint up to 9 and its dependencies `axe-core` (MPL-2.0) and `language-subtag-registry` (CC0-1.0) are not on the allowed set. Until a plugin exists the review checklist covers accessibility, and the 7 `svelte-ignore a11y_*` comments are re-evaluated by hand with the panel that holds them. Open for the owner: allow those two licences for development tools, or choose another a11y check.
- Base UI (S1): `@base-ui/react` is added to `resolve.dedupe` and to the `paths` of `tsconfig.json` (as React is: the packages have no `node_modules`), is a row of the framework table of the boundary check (kit, view and bin, in `.tsx` files only; a test covers it), and `#app` has `isolation: isolate` in `apps/web/src/app.css`, the stacking context its quick start asks for so that its popups, rendered into the body, sit above the page. The Svelte shell is unchanged by it: the Bits UI popups are in the body too and were above the page already.
- `ui-literals` reads `.tsx`: the strict script reads JSX style objects (also a bare number as a length or a millisecond value), `text-[13px]`-style class strings (an `_` in an arbitrary value is a space) and JSX comments; `xtask/src/ui_literals.rs` takes `tsx` into its scope for `apps/web` and for the web packages (kind `ui`) and reads `fontSize`, `text-[..]`, `transitionDuration` and trailing `//` comments. `size-check` already counts `tsx`.
- Vitest includes `.test.tsx`. React files are `.tsx` even when they hold no markup (`useAppState.tsx`, `island.tsx`), so that the hook lint, the literal scan and the boundary check, which go by extension, see every file that imports React.
- Place: `packages/web-chrome/src/react/` (the kit, as proposed). The hook takes a structural `Store` type instead of importing `Engine`, so the kit still imports no other package. Adding React to the framework allow-list of the `view` kind later is the one line of the T18 table; React files exist in `kit` only until then.

The boundary check of the audit (T18, kinds of web package with a framework allow-list per kind) is built beside S0, so React is confined to the kit and view packages; it is part of #307 and #314, not of this plan.

Pilot: the issue suggests the Import HTML dialog, but it does not exist and decision 41 keeps it out of scope. Proposed pilot: the **primitives of `web-chrome` as React copies, with `Tooltip` and `Menu` on Base UI and the `/?states` page (`StateMatrix`) rendered by React**. It needs only the Vite dev server, as `AGENTS.md` says of `/?layout` and the measurer script (checked for `/?states` on 25 Nov 2026: the header of `states-e2e.mjs` says the page needs no Meldframe server, and the script passed against the Svelte version with 47 controls in each theme), exercises Tailwind, tokens, `ui-literals` for `.tsx`, lint, type check and SSR tests, puts two of the four Bits families on the new library, and its parity script is strict: `packages/web-chrome/scripts/states-e2e.mjs` checks every control in six states in both themes (focus ring, hit area of 24 px or more, names). A second pilot step, `web-export` (the Dialog family: focus into the dialog and back to the File menu button, Escape, an open dialog rebuilt when an agent's change arrives), checked by `export-e2e.mjs`, decides whether Base UI stays or Radix takes over. The pilot is done when both pass on React with the Svelte version still in place.

Result of the first step (S1, 25 Nov 2026). The React copies are in `packages/web-chrome/src/react/` (Button, Checkbox, Field, Icon, IconButton, Menu, Section, Segmented, StateMatrix, Tabs, Tooltip with its provider; class strings, `data-testid` values, ARIA roles and names copied from the Svelte markup, the icon paths moved to `icons.ts` and shared). The Svelte copies stay. `/?states&react` draws the React `StateMatrix` through the island helper from a thin Svelte host (`apps/web/src/react-pilot/`: one switch in `pilot.ts`, one line in `main.ts`, both removed with the last Svelte page); the default page is still Svelte. Results: `states-e2e.mjs` passes on both (47 controls in each theme, 44 and 35 reached by Tab, the same on the two); `MELDFRAME_UI=react` selects the React page. Screenshots of the two pages at 1440x900, device scale factor 2, full page (2880 x 3224 px) differ in 0 of 9,285,120 pixels, also after one Tab press (the drawn ring is the same), and the Svelte page is unchanged against the shot taken before the slice. `overlays-e2e.mjs` passes on both the Svelte and the React fixture (development-only pages `/?overlays=svelte` and `/?overlays=react`), and on the real `/?layout`. The unit tests render each primitive with `renderToStaticMarkup` and repeat the assertions of `chrome.test.ts` and `forms.test.ts`. What the pilot did not settle: the second step (`web-export`, the Dialog family) and so the final word on Base UI against Radix.

## 8. Order of slices and sizes

A port slice adds the React files and later deletes the Svelte ones, so its changed lines are about twice the Svelte lines plus tests; the rule is 1,000 changed lines (ceiling 1,500) and 15 files (ceiling 20). Slices are therefore cut at about 350 Svelte lines or fewer, and a slice that only adds a React copy (the pilot) counts once. Deleting the Svelte side is the same pull request when the script already passed on the React side, otherwise the next one.

| Slice | Content (Svelte lines) | Parity script | Waits for |
|---|---|---|---|
| S0 | Toolchain, `useAppState`, island helper, `.tsx` in the literal scans (the boundary check lands beside it) | none (no visible change; Node tests and two build checks) | T18 check (#307, #314) before S1 |
| S1 | `web-chrome` primitives and `/?states`, React copy (about 540); written 25 Nov 2026, Svelte copies kept (see section 7) | `states-e2e.mjs` (`MELDFRAME_UI=react`), `overlays-e2e.mjs` | S0 |
| S2 | `web-export` (227), the pilot dialog | `export-e2e.mjs` | S1 |
| S3 | `web-feedback` (149) | `editor-e2e.mjs` (banners, notices) | S1, #319 (U9) |
| S4 | `web-shell` palette and shortcut sheet (191) | `palette-e2e.mjs`, `layout-e2e.mjs` | S1 |
| S5 | `web-panels` a: layers, variables panel, bind picker, picker list (252) | `variables-e2e.mjs`, `editor-e2e.mjs` | #319, #320 |
| S6 | `web-panels` b: sections and properties panel (about 480, in two pull requests: sections, then the panel) | `text-style-e2e.mjs`, `border-remove-e2e.mjs`, `ellipse-e2e.mjs` | #322 |
| S7 | `web-panels` c: image fill picker (156) | `image-fill-e2e.mjs` | S6 |
| S8 | `web-activity` (432, in two: panel and messages, then batches and checkpoints) | `activity-e2e.mjs`, `checkpoints-e2e.mjs` | #323 |
| S9 | `web-variables` (641, in two: lists and forms, then detail, body and dialog) | `manage-e2e.mjs` | #322, S2 |
| S10 | `web-variables-io` (344) | `io-e2e.mjs` | S2, S9 |
| S11 | `Canvas` as a React wrapper over the unchanged Pixi code (457) | `editor-e2e.mjs`, `measurer-e2e.mjs`, `layout-interaction-e2e.mjs` | S5 to S8 |
| S12 | The shell flips: `Layout`, `LayoutDemo`, `TextEditor` (312), then `Editor`, `App`, `Harness`, `main.ts` (about 640, in two); one React root; the island helper and the switch go | all scripts | S2 to S11 |
| S13 | Svelte is gone: the plugin, `svelte`, `bits-ui`, `svelte-check`, `eslint-plugin-svelte`, the Svelte paths of `ui-literals`, the `.svelte` scans; docs (`AGENTS.md`, `docs/architecture.md`, `docs/ui-design.md`, ADR-0008 and ADR-0026 get a status note) | all scripts | S12 |

The slices of `web-panels`, `web-activity` and `web-variables` wait for the audit findings that move their rules into `meldframe-app` (P1 and P2 of #307: #319 to #323), so no rule is ported twice; the findings of #324 are decided before S12. About 17 pull requests; no freeze: new web UI after S0 is written in React as an island, never as a new `.svelte` file (question 4).

## 9. Questions for the owner

1. Component library: Base UI (proposed), Radix or React Aria Components. The pilot's second step can decide it with evidence.
2. Per-panel switch (`/?react=...`) so one script runs on both versions, or a straight replacement per panel (smaller code, weaker parity).
3. Browser scripts in CI. They run by hand today, so the parity check is as strong as the discipline of running them. The scripts that need only the Vite dev server (`layout-e2e`, `palette-e2e` and `measurer-e2e` per `AGENTS.md`; `states-e2e` is the same, checked 25 Nov 2026) could run in the nightly first. Not decided here.
4. A stop on new `.svelte` files once S0 is merged (recommended).
5. Whether the audit moves (#319 to #323) strictly come first, as the table assumes, or panel by panel in parallel with the port.
6. Accessibility lint: `eslint-plugin-jsx-a11y` needs `axe-core` (MPL-2.0) and `language-subtag-registry` (CC0-1.0), neither on the allowed set of `npm run licences`, and it declares ESLint up to 9. Allow the two licences for development tools, find another check, or keep the review checklist alone. Likewise `eslint-plugin-react-hooks` 7.x needs `caniuse-lite` (CC-BY-4.0) through Babel; S0 uses 5.2.0 until then.

## 10. Risks

- Two frameworks in one bundle until S13 (size and two focus and portal systems). Mitigation: islands are leaves, dialogs portal to the page root with one stacking context, and the pilot checks focus return.
- The 19 `untrack` calls and 23 effects were written for Svelte's dependency tracking; each needs a ref or a stable callback in React, and the 25 `tick()` focus handoffs become effects or `flushSync`. Reviewed one slice at a time against the script that covers it.
- Behaviour differences between Bits and Base UI (outside click, Escape, focus return, tooltip delay: today one provider with 400 ms). Covered by `states-e2e`, `export-e2e` and `palette-e2e`.
- The migration costs a rewrite of the chrome with no change a person sees (ADR-0026, Risks). This plan keeps each step shippable and reversible until S12.

## 11. References read for this note

- React: useSyncExternalStore (react.dev/reference/react/useSyncExternalStore).
- Tailwind CSS: detecting classes in source files (tailwindcss.com/docs/detecting-classes-in-source-files); installing with Vite (tailwindcss.com/docs/installation/using-vite, snippet).
- `@pixi/react` README (github.com/pixijs/pixi-react).
- Base UI quick start (base-ui.com/react/overview/quick-start); Base UI Tooltip and Menu (base-ui.com/react/components/tooltip, /menu); comparison of Base UI and React Aria Components (untitledui.com/blog/base-ui-vs-react-aria), a vendor source that builds on React Aria.
- Read only as search snippets, not relied on: Radix and shadcn/ui comparisons, tldraw and Excalidraw architecture pages (the Excalidraw page was blocked by a bot check). A reader who wants a canvas editor built on React as a reference should start with those two projects' own documentation.
