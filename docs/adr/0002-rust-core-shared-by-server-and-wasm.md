---
status: accepted
---
# Rust core shared by native server and WASM client; thin server

The document core (validation, history, concurrency rules, layout) is written once in Rust and compiled both natively for the server and to WASM for the browser client. The server stays thin: admission, ordering, persistence, broadcast. This gives identical rules on both sides and cheap convergence tests, at the cost of a WASM bundle (about 0.6 MB in the spike) and a Rust-first stack.

## Considered options

- TypeScript core on both sides: rejected; no single implementation with native speed for the server and property tests at scale.
- Client-only core with a dumb relay server: rejected; the server must validate agent edits.
