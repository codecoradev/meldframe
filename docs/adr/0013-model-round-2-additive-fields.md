---
status: accepted
---
# Model round 2 grows by additive fields, not by changing the ones that exist

The paint and layout of W1 (opacity, no fill, per-side padding, margin, absolute children in auto layout, gradients) are added as new optional fields of the node and of its patch, each with a default that reproduces today's behaviour. No existing field changes its type, so a file written before a field existed loads unchanged and `SCHEMA_VERSION` stays 1 until a change cannot be additive.

- Opacity is `opacity`, a number from 0 to 1 that defaults to 1.
- No fill is the word `none` in the existing `fill` field (any other fill is a `#hex` colour as before); `fill2`, the border colour and the shadow colours stay `#hex` only.
- Margin per side is `marT`, `marR`, `marB` and `marL`, plain numbers from 0 that default to 0 (no three-state patch is needed: there is no single `margin` to fall back to). A margin is room around a child in an auto layout frame; a frame without layout ignores it.
- An absolute child is `absolute: true` (default false): the child leaves the flow of an auto layout frame and sits at its own `x` and `y` inside the parent. It needs no extra position field because every child of a frame without layout is already placed that way.
- A gradient is `gradient`, an optional object `{kind, angle, stops}` painted over `fill` and under the image; a patch has three states for it, as for `image` (absent leaves it, `null` removes it, an object sets it). Angles are CSS angles (0 up, 90 right, 180 down), so the canvas and the export agree. It is refused on text, which is painted with its `fill`.
- Padding per side is `padT`, `padR`, `padB` and `padL`, each optional; when one is missing the single `padding` applies to that side, so an old file lays out as before. In a patch the side has three states, as `image` has: absent leaves it, a number sets it, `null` unsets it (so an undo can return a side to "follows `padding`").

## Why

Files are the data of people and agents, and a version bump needs a migration of every file and every client at once. An additive field costs nothing to files that do not use it, the generated TypeScript and the check crate gain a field with a compile-time guard (`patch_fields!` names every patch field), and an old client reading a new file only loses what it never drew.

## Considered options

- A `fillMode` field next to `fill`: more explicit, and it would suit gradients, but it adds a second field that has to agree with `fill`. Not chosen; gradients get their own field when they come.
- `padding` as an object `{t, r, b, l}`: the cleanest shape, but it changes the type of an existing field and needs a migration and `SCHEMA_VERSION` 2. Not chosen.

## Consequences

- Each slice of E3.2 is one pull request through the same places: the protocol node and patch, the check crate, the undo field list, the generated TypeScript, the scene of the canvas and the tool text of `apply_commands`.
- The oracle probe that `docs/fidelity-contract.md` asks for before export treats a field as supported is a separate step (E4.2); until then these fields are drawn on the canvas and exported as unsupported.
