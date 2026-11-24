# Meldframe

Agent-first design canvas. You and AI agents design on one canvas; the output is faithful HTML/Tailwind, and the same design tokens are rendered on the canvas and exported as CSS variables.

Status: pre-alpha. This repository starts from a set of validated spikes (Rust core, WASM client, MCP surface). No product code yet.

## Scope (current)

- Web editor plus an MCP server so external agents (Claude Code, Cursor, Codex) can design on the canvas.
- One `Command` model shared by UI, WebSocket ops, MCP tools and undo.
- Design tokens (variables) rendered on the canvas and exported as CSS variables.
- Export first (HTML/Tailwind, PNG). Import is variables, plus a scoped HTML importer that is ordered and not built yet (decision 41).

Carousel, slides and motion are planned extensions and are on hold until the core above is solid.

## Docs

- [PRODUCT.md](PRODUCT.md), [GLOSSARY.md](GLOSSARY.md), [AGENTS.md](AGENTS.md)
- [docs/prd.md](docs/prd.md): requirements.
- [docs/architecture.md](docs/architecture.md) and [docs/adr/](docs/adr/): structure and decisions.
- [docs/standards/modular-thin-ui.md](docs/standards/modular-thin-ui.md): CodeCora standard for modules, thin UI, gate and events (mirror).
- [docs/roadmap.md](docs/roadmap.md): open decisions and order of work. [docs/epics.md](docs/epics.md): milestones, epics and acceptance criteria.
- [docs/quickstart-agent.md](docs/quickstart-agent.md): connect an agent to the local development server (`tools/connect-claude.sh` prints the Claude Code command).
- [docs/owner-questionnaire.md](docs/owner-questionnaire.md): questions for the owner.
- [docs/decisions.md](docs/decisions.md): decision register. [docs/concept.md](docs/concept.md): original concept.
- The token loop, with runnable examples: [docs/tokens-and-code.md](docs/tokens-and-code.md).
- HTML import (code to design), the plan of record for E4.4 and the proposed `import_html`: [docs/import-html.md](docs/import-html.md).

## Run locally

```sh
cp .env.example .env
just dev-db       # Postgres on 127.0.0.1:5433 (Docker or no-Docker script) plus the development seed
just dev          # Postgres, seed, then the server on http://127.0.0.1:8080 (token printed once)
just web          # no web client yet (E1.6)
just dev-smoke    # server on Postgres: submit, restart, restored
just test
just check
```

See the Development section of [AGENTS.md](AGENTS.md) for ports, URLs and the seed.

## License

All rights reserved. See [NOTICE](NOTICE).
