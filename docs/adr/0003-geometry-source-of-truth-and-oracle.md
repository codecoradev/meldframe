---
status: proposed
---
# Core geometry is the source of truth; export is a translation; a browser oracle gates CI

The Rust layout result decides every node's box. The HTML/Tailwind export translates it to CSS and must match within a stated tolerance (0.05 px geometry, 1 px text ink; see `docs/fidelity-contract.md`). A Chromium oracle in CI enforces this for fixtures and fuzzed documents. Documents outside the contract (fill under hug, overflow, zero-width fills) are normalised or warned on write rather than matched in CSS.

## Considered options

- Use the browser's CSS layout as the source of truth: rejected; every non-browser consumer (server, WASM, agents without a browser) would need a browser, and the measurements showed the two engines already agree to 0.034 px in the supported subset.
- Contract plus oracle with the core as the truth (chosen) vs. a hybrid that also verifies at runtime when a headless browser exists; the runtime check is optional and tied to the capture decision.

## Consequences

Each new node property needs a fixture, a probe and an entry in the supported table before it ships. Open items (tolerance, text outside Latin Inter, font registry) are in the owner questionnaire.
