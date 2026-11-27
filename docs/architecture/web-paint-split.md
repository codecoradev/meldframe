# Split of `packages/web-canvas`: the new package `web-paint`

Plan for issue #251 (E3.1, the render model and the canvas). The owner has agreed to carry it out: this page is the plan, and the move follows in the next commit of the same pull request. `AGENTS.md` and `docs/standards/modular-thin-ui.md` (D1) set the size rule it answers: a crate or package aims for 1,500 to 2,500 lines, may pass 2,500 only as a candidate for a planned split, and never passed 4,000 (the ceiling of the time; it is 50,000 since decision 42).

Status, 27 Nov 2026 (T16, decision 43): since T16 the reach is computed in `meldframe-render` (`PaintedBox.paint_bounds`, read by the capture rectangles as the scene's `paintBounds`); `borderReach`, `shadowBounds`, `blurReach` and the `PaintBox` type named below are gone from `web-paint`, whose shapes stay here. The rest of this page is the history of the split and is not changed.

## Why now

`meldframe-web-canvas` is 2,716 lines on `develop` at 61727f8 (`cargo xtask size-check`), 216 above the ideal band. The package grew by the border and shadow slices; the next E3.1 slices (more paint on a node) would grow it again at the same place. No file is over 500 lines, so only the package is split, not a file.

## The seam

The package holds two kinds of code:

- **The scene and the host input.** `scene.ts` (the scene as plain data, its resolution, the diff, hit testing, the capture rectangles), `camera.ts`, `overlay.ts`, `interaction.ts` (gestures), `textstyle.ts` and `geometry.ts` (points and rectangles). They know the scene, the camera and the pointer.
- **The paint of one box.** `border.ts`, `shadow.ts`, `gradient.ts` and `image.ts`. Each defines the plain-data description of one kind of paint (`SceneBorder`, `SceneShadow`, `SceneGradient`, `SceneImage`), its equality (`sameBorder` and so on) and the shapes it covers worked out as CSS and the export do (`borderRing`, `borderReach`, `shadowShape`, `shadowBounds`, `blurFilterSettings`, `gradientLine`, `radialRadius`, `fitRect`). They are pure, take numbers and give numbers, and import nothing but one rectangle type.

The second kind is the seam. It is the part that grows with E3.1 (the fidelity contract adds paint, not gestures), the part the export has to agree with, and the part no other code in the package reaches into: the scene only compares and measures it.

Checked in the imports on `develop` (not assumed):

| File | Imports inside the package |
|---|---|
| `border.ts`, `gradient.ts`, `image.ts` | nothing |
| `shadow.ts` | `type Rect` from `geometry.ts` |
| `scene.ts` | `geometry`, `border`, `gradient`, `image`, `shadow` |
| `textstyle.ts` | `type SceneTextAlign` from `scene.ts` |
| `camera.ts` | `geometry` |
| `overlay.ts` | `camera`, `geometry` |
| `interaction.ts` | `camera`, `geometry`, `overlay` |

Nothing in the paint files imports Pixi, the camera, the overlay or the gestures. Nothing under `packages/` or `apps/web` outside `web-canvas` is imported by `web-canvas`. The importers of the paint files are `scene.ts` (inside), `packages/web-link/src/scene.ts` (types) and in `apps/web/src/canvas`: `stage.ts`, `bodycanvas.ts`, `shadowparts.ts` and two stage tests.

### Where this differs from the issue

The issue names border, shadow and the capture rectangles (`paintedRect`, `captureRectOf`, `captureBounds`) as the seam. Two changes, both from the imports:

- **Gradient and image join.** They have the same shape as border and shadow (a description, an equality, pure geometry, no imports) and are painted on the same box. Leaving them would split "the paint of a box" across two packages. The cost is 153 more moved lines, all with their tests.
- **The capture rectangles stay.** `captureRectOf` and `captureBounds` walk a `ResolvedScene` (visibility and `clipRect`) and use `unionRects` and `intersectRects` from `geometry.ts`, which `camera`, `overlay`, `interaction`, `web-export` and `web-document` also import. Moving them either makes the new package import the canvas (a cycle with `scene.ts`, which needs `borderReach` and `shadowBounds`) or moves `geometry.ts` into a package called paint, and with it a dozen unrelated import paths. They keep calling `borderReach` and `shadowBounds` from the new package. If `geometry.ts` ever becomes a package of its own, the capture rectangles can follow in a later change.

## The new package

`packages/web-paint` (`@meldframe/web-paint`), kind `ui` (it is TypeScript used as source by `apps/web`, like every `packages/web-*`; `Cargo.toml` and `rust/lib.rs` only make it a unit of the size and layering checks). The name follows the glossary and the layering rules: `web-` is the prefix of the web packages, and the render step already speaks of painted boxes (`packages/web-link` reshapes them), so `paint` names what the package describes. Nothing in `GLOSSARY.md` or the layering rules calls for another name.

It depends on nothing, inside or outside the repository. Its modules are imported by path, as the other web packages are: `@meldframe/web-paint/border`, `/shadow`, `/gradient`, `/image`.

`shadow.ts` needs a rectangle type only for the box it measures. It takes a local `PaintBox` (`x`, `y`, `w`, `h`), which `Rect` of the canvas satisfies as it stands, so the package stays a leaf and no call changes.

## What moves, and the public surface each package keeps

Moved with `git mv`, tests with their sources, no edit to a line of code except the import in `shadow.ts` and the type that replaces `Rect` there:

| From `packages/web-canvas/src` | To `packages/web-paint/src` | Tests |
|---|---|---|
| `border.ts` (61) | same name | `border.test.ts` (75, 9 tests) |
| `shadow.ts` (113; 120 after the change below) | same name | `shadow.test.ts` (131, 17 tests) |
| `gradient.ts` (31) | same name | `gradient.test.ts` (45, 7 tests) |
| `image.ts` (36) | same name | `image.test.ts` (41, 6 tests) |

`web-paint` exports what those files export today.

`web-canvas` keeps `scene`, `camera`, `overlay`, `interaction`, `textstyle` and `geometry`, with the exports they have. `scene.ts` keeps re-exporting the types of the paint descriptors (`SceneBorder`, `SceneBorderPosition`, `SceneGradient`, `SceneImage`, `SceneImageFit`, `SceneShadow`), because they are the field types of `SceneNode`: an importer of `@meldframe/web-canvas/scene` changes nothing. The old module paths `@meldframe/web-canvas/{border,shadow,gradient,image}` are not kept (no shim): their importers are in this repository and the move updates them.

## Who imports whom after the move

```
apps/web (canvas/stage, bodycanvas, shadowparts) ──► web-paint
apps/web ... ─► web-link ──► web-paint (types)
web-link, web-document, web-input, web-shell, web-export, apps/web ──► web-canvas ──► web-paint
web-paint ──► (nothing)
```

`web-canvas` imports `web-paint` from `scene.ts` only. No cycle, and `web-paint` never imports `web-canvas`. `cargo xtask deps-check` does not see these edges (they are TypeScript imports, and both manifests have no Cargo dependencies); the direction is kept by the import list above and by the fact that `web-paint` has no import from another package.

## Lists the new package joins

`apps/web/package.json` (the `lint` loop), `apps/web/tsconfig.json` (`paths`), `apps/web/vite.config.ts` (`webPackages`), `packages/web-tokens/src/contrast.test.ts` (`ROOTS`, which scans class lists in the web packages), `Cargo.lock` (the workspace member `packages/web-*` is a glob, so `Cargo.toml` stays as it is). `packages/web-tokens/src/tailwind.css` is not changed: it lists packages that draw with Tailwind classes, as `web-canvas` does not, and neither does `web-paint`. `xtask/size-allow.toml` has no entry for either package. `ui-literals` finds the packages by the `web-` prefix.

## Line counts

`cargo xtask size-check` counts rs, ts, tsx, js, mjs, css, scss, svelte and html files, tests included, and the one-line `rust/lib.rs`.

| Package | Before | After |
|---|---|---|
| `meldframe-web-canvas` | 2,716 | 2,183 |
| `meldframe-web-paint` | (new) | 541 |

Both are under 2,500 and no file is over 500 (the largest file left in `web-canvas` is `interaction.test.ts`, 397; the largest in `web-paint` is `shadow.test.ts`, 131). Moves do not count towards the pull request size ceilings; the other changes are import paths and the package lists above.

## Not changed

No behaviour: the vitest runs of `packages/web-canvas` and the new package together hold the same tests as before (146: 107 stay, 39 move), the whole `apps/web` run is the same, and `border-e2e.mjs` and `shadow-e2e.mjs` still pass in Chromium. The export, the render step, the protocol and `docs/decisions.md` are untouched.
