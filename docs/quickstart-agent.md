# Connect your agent in two minutes

This page takes you from a clean checkout to an agent that edits a file on your machine through MCP, with the editor open beside it. It covers the local, development setup only: the server runs on your machine and the agent proves who it is with a development token. Sign-in through Gatehouse (OAuth) is not built yet; see [Limits](#limits).

Terms follow [GLOSSARY.md](../GLOSSARY.md). An agent is an external AI client that drives the canvas through MCP. A development token is a secret you choose; the server maps it to a workspace and an actor.

## What you need

- The Rust toolchain (`cargo`) and a checkout of this repository.
- An MCP client that supports streamable HTTP and a bearer header. The commands below are for Claude Code (`claude`).
- For the editor beside the agent: `just`, Node and `wasm-bindgen-cli` (see `just web` in [AGENTS.md](../AGENTS.md)).
- For `just dev`: Postgres, through Docker or local binaries (see `just dev-db` in AGENTS.md). The by-hand start in step 1, option B needs none.

## 1. Start the server

Pick one.

### Option A: `just dev` (Postgres, one token)

```sh
cp .env.example .env
just dev
```

`just dev` starts Postgres, loads the seed (workspace `ws_dev`, file `file_dev`), creates a token on the first run, saves it to `.dev-token` (git-ignored) and prints it once, then runs the server on `http://127.0.0.1:8080`. The token's actor is `agent:dev`, the actor of the editor that `just web` opens. If you give the same token to an agent, the agent works as `agent:dev` too, and because `just dev` lists that actor in `MELDFRAME_REVIEWERS`, the agent could accept its own batches in the Activity panel. That is fine for a first look. For the review checks of the M1 demonstration use option B.

### Option B: the server by hand, in memory, one token for you and one for the agent

Run from the repository root. `env -u DATABASE_URL` keeps this one server in memory (files are gone when it stops) without changing your shell. The editor's token is the one in `.dev-token` if that file exists (from `just dev`; it is left as it is), else a new one, saved there for `just web` (the file is git-ignored):

```sh
if [ -s .dev-token ]; then editor=$(head -n 1 .dev-token); else
  editor=mf_dev_$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')
  (umask 077; printf '%s\n' "$editor" > .dev-token)
fi
agent=mf_dev_$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')
export MELDFRAME_AGENT_TOKEN="$agent"   # for tools/connect-claude.sh in step 2

env -u DATABASE_URL \
  MELDFRAME_LISTEN=127.0.0.1:8080 \
  MELDFRAME_REVIEWERS=agent:dev \
  MELDFRAME_DEV_TOKENS="$editor:ws_dev:agent:dev,$agent:ws_dev:agent:claude" \
  cargo run -p meldframe-server
```

The server prints its modules and tools, then `listening on 127.0.0.1:8080`. Check it from another terminal: `curl http://127.0.0.1:8080/v1/health` (no token needed).

What the three variables do (all read in `apps/server/src/main.rs`):

- `MELDFRAME_LISTEN`: the address to serve on. Unset, the binary prints its registry and exits without serving. `just dev` defaults it to `127.0.0.1:8080`; by hand there is no default, so set it.
- `MELDFRAME_DEV_TOKENS`: comma-separated `token:workspace:actor` entries. A token is 16 bytes or more, an actor may hold a colon (`agent:claude`), and a token listed twice or an entry with an empty part stops the server at start. Unset, the server does not start. Both tokens above are in workspace `ws_dev`, so the editor and the agent see the same files.
- `MELDFRAME_REVIEWERS`: the actors that may accept or flag a batch. List only the editor's actor, so that the agent cannot review its own work.

## 2. Give the agent the endpoint and the token

The MCP endpoint is `http://127.0.0.1:<port>/mcp`, and the token goes in the header `Authorization: Bearer <token>`. `tools/connect-claude.sh` assembles the Claude Code command from those two facts; it prints a line and does nothing else (it does not start a server, call one, run `claude` or write a file):

```sh
tools/connect-claude.sh                        # option B: reads MELDFRAME_AGENT_TOKEN, port 8080
tools/connect-claude.sh --port 9000            # a server on another port (or --listen HOST:PORT)
tools/connect-claude.sh --help
```

For the address it takes `--port` or `--listen`, then `MELDFRAME_LISTEN`, then `127.0.0.1:8080`. For the token it takes `--token`, then `MELDFRAME_AGENT_TOKEN`, then the first line of `.dev-token` (so after option A it works with no arguments). It refuses a token shorter than 16 bytes, the same minimum as the server. It prints, for example:

```
claude mcp add --transport http meldframe http://127.0.0.1:8080/mcp --header "Authorization: Bearer mf_dev_0123456789abcdef0123456789abcdef"
```

Run that line. It puts the token on the command line, and Claude Code keeps the header in its own configuration; `claude mcp remove meldframe` takes it out again. Prefer `MELDFRAME_AGENT_TOKEN` to `--token`: an argument is visible in the process list of your machine.

The printed line holds the secret. Once you run it, the token is in your shell history (start the command with a space under `HISTCONTROL=ignorespace`, or paste the line from the clipboard instead of typing it), and it stays in the terminal scrollback and in any screenshot of the terminal. Never put the line, or a file that holds its header, in any file of a repository, and do not use an option of `claude mcp add` that writes the configuration into the repository. To rotate the token, restart the server with a new one; the old token stops working at once.

Any other MCP client that supports streamable HTTP with a bearer header needs the same two things: the URL above and the header `Authorization: Bearer <token>`. The server also wants `Accept: application/json, text/event-stream` on a POST, which MCP clients send. Per-client configuration syntax is not documented here because it has not been checked against those clients. The JSON that [`m1-claude-demo.mjs`](../packages/web-e2e/scripts/m1-claude-demo.mjs) writes for Claude Code's `--mcp-config` is:

```json
{ "mcpServers": { "meldframe": { "type": "http", "url": "http://127.0.0.1:8080/mcp", "headers": { "Authorization": "Bearer <token>" } } } }
```

Check the endpoint without any agent:

```sh
printf 'header = "Authorization: Bearer %s"\n' "$MELDFRAME_AGENT_TOKEN" | curl -s --config - \
  -H 'accept: application/json, text/event-stream' -H 'content-type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' http://127.0.0.1:8080/mcp
```

It answers with the tool list. With no token or a wrong one it answers HTTP 401 with `{"error":{"code":"unauthenticated",...}}` before any MCP work. The `printf` keeps the token out of the argument list.

## 3. Hello, world

Open the editor on a file name that is new, as the person:

```sh
MELDFRAME_WEB_FILE=hello just web
```

Open the address Vite prints (usually `http://localhost:5173`). Then, in Claude Code, ask for a hero on that file, for example: "Use the meldframe tools. Read the file `hello` with get_tree, then build a landing hero in it: a frame with a heading, a line of text and a button. Give every node a name that says what it is." The agent reads, builds the hero in an atomic batch and the editor shows it without a reload, with the agent's actor on the nodes it changed. Move the heading yourself and ask the agent to change it: its edit is checked against what it last read (`ifSeq`) and refused with `conflict` if you moved it since.

For the full follow-along, the M1 demonstration of [docs/epics.md](epics.md) with a real Claude Code, run [`packages/web-e2e/scripts/m1-claude-demo.mjs`](../packages/web-e2e/scripts/m1-claude-demo.mjs). Its header says it needs: a server with two tokens and `MELDFRAME_REVIEWERS` naming only the editor's actor (option B above), `just web` on the same file, `claude` installed and logged in, Playwright, a fresh server and a fresh file for each run, and `MELDFRAME_DEMO_TOKEN2` set to the agent's token:

```sh
MELDFRAME_WEB_FILE=<file> MELDFRAME_DEMO_TOKEN2="$MELDFRAME_AGENT_TOKEN" node packages/web-e2e/scripts/m1-claude-demo.mjs
```

[`m1-demo.mjs`](../packages/web-e2e/scripts/m1-demo.mjs) is the same demonstration with a scripted agent instead of Claude Code.

## What the agent can do

[`skills/meldframe/SKILL.md`](../skills/meldframe/SKILL.md) tells an agent how to work in a file: read first, change in small top-down batches guarded by `ifSeq`, name every node, look at the result with `capture`. The tool names, arguments and risk tiers are in [`docs/mcp-tools.json`](mcp-tools.json). In short, an agent can read the tree and nodes of a file and its variables, change a file with `apply_commands`, copy a frame as a variant, capture a frame as a PNG (needs an editor open on the file), upload an image, export a frame as HTML, import variables from CSS, and keep a critique list. Deleting a frame is refused with `approval_required` until an owner answers, and no route lets an owner answer yet (E2.4). A server built without a feature leaves that feature's tools out.

## Limits

- **Development tokens only.** A token is a string you made up and the server's environment maps it to a workspace and an actor. There is no sign-in, no expiry and no revocation short of restarting the server. [E2.2](epics.md) (MCP OAuth through Gatehouse) and [E2.4](epics.md) (agent grants, so that no token is copied by hand) are open; when they land, the token step of this page is replaced.
- **Loopback only.** The server refuses to start with development tokens when `MELDFRAME_LISTEN` is not a loopback address (`check_dev_listen`, `crates/integrations/meldframe-http/src/auth.rs`). `MELDFRAME_ALLOW_DEV_PUBLIC=1` lifts that, and nothing else does. The server has no TLS, so on a public address the token crosses the network in clear text. Only the exact value `1` counts.
- **Release builds.** A release build (`cargo build --release`, or any build without debug assertions) refuses to start with development tokens unless `MELDFRAME_ALLOW_DEV_TOKENS=1` (`check_dev_tokens_allowed`), and prints a warning when it does. A debug build, which is what `cargo run` makes, ignores the variable. The two overrides are separate: a release build on a public address needs both. E2.1 and E2.4 keep the open item "development tokens are disabled in release builds" until Gatehouse replaces the override. [ADR-0022](adr/0022-sign-in-and-agent-grants-through-gatehouse-with-a-principal-that-has-a-kind.md) (status proposed) replaces `MELDFRAME_ALLOW_DEV_TOKENS` with a `dev-tokens` build feature; this page describes the code as it is today and must change with that slice.
- **Host names.** The MCP route answers only to the Host names `localhost`, `127.0.0.1` and `::1`, as a guard against DNS rebinding. Another name must be listed in `MELDFRAME_MCP_HOSTS` (comma-separated), so a server on another address needs that too.
- **Memory.** Without `DATABASE_URL` files, batches and checkpoints are lost when the server stops.

## What was and was not verified

The server was built from this checkout and started in memory (option B, a free loopback port, two throwaway tokens), and these were run against it: `tools/connect-claude.sh` with and without arguments, `GET /v1/health`, `initialize`, `tools/list` and a `whoami` call on `/mcp` with the agent's token (the actor and workspace in the answer were those of the entry in `MELDFRAME_DEV_TOKENS`, and `tools/list` held 13 tools), a 401 `unauthenticated` for a missing token and for a wrong one, a 403 for a Host name outside the loopback names (`evil.example`), and the start-up refusals for a listen address that is not loopback, a missing `MELDFRAME_DEV_TOKENS` and a token under 16 bytes. The built binary was run, not `cargo run`. The `claude mcp add` line follows the form `claude mcp add --transport http <name> <url> --header "<header>"`; it was not run with a real Claude Code, and nothing here was tried with Codex, Cursor or any other MCP client. The Host form `[::1]` and a listen address that is not loopback together with `MELDFRAME_MCP_HOSTS` were not tried against the guard. The release-build rule is stated from `check_dev_tokens_allowed` and its tests in `apps/server/tests/auth.rs`; a release binary was not built for this page. `just dev` and `just web` were not run (they need Postgres and a built WASM bridge); their behaviour is stated from `justfile`, `scripts/dev.sh` and [AGENTS.md](../AGENTS.md). The hello-world of step 3 was not run either. The M1 demonstration scripts need a browser and, for `m1-claude-demo.mjs`, a real Claude Code, so they were not run.
