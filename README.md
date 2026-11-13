# Meldframe

Agent-first design canvas. You and AI agents design on one canvas; the output is faithful HTML/Tailwind, and the same design tokens are rendered on the canvas and exported as CSS variables.

Status: pre-alpha, private. This repository starts from a set of validated spikes (Rust core, WASM client, MCP surface). No product code yet.

## Scope (current)

- Web editor plus an MCP server so external agents (Claude Code, Cursor, Codex) can design on the canvas.
- One `Command` model shared by UI, WebSocket ops, MCP tools and undo.
- Design tokens (variables) rendered on the canvas and exported as CSS variables.
- Export first (HTML/Tailwind, PNG). Import is deferred.

Carousel, slides and motion are planned extensions and are on hold until the core above is solid.

## Docs

- [docs/decisions.md](docs/decisions.md): decisions and open questions.
- [docs/concept.md](docs/concept.md): concept v1.

## License

Proprietary. All rights reserved. See [NOTICE](NOTICE).
