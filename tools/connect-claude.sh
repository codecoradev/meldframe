#!/usr/bin/env bash
# Prints the `claude mcp add` line that connects Claude Code to a Meldframe development server
# (docs/quickstart-agent.md). It only assembles the line: it starts nothing, calls no server, runs
# no `claude` command and writes no file. The token goes to standard output in that one line and
# nowhere else; the messages on standard error never hold it.
set -eu

usage() {
  cat <<'EOF'
usage: tools/connect-claude.sh [--port N | --listen HOST:PORT] [--token TOKEN] [--name NAME]

Prints the `claude mcp add` line for the MCP endpoint of a development server.

Server address, first that is set wins:
  --port N              the server listens on 127.0.0.1:N
  --listen HOST:PORT    the same value as MELDFRAME_LISTEN
  MELDFRAME_LISTEN      in the environment (0.0.0.0 or [::] is printed as 127.0.0.1)
  default               127.0.0.1:8080, the address of `just dev`

Token, first that is set wins:
  --token TOKEN         visible to other users of this machine in the process list; prefer the next one
  MELDFRAME_AGENT_TOKEN in the environment
  .dev-token            in the repository root (the file `just dev` writes: the token of the editor's own
                        actor, agent:dev; a token of its own for the agent is better, see the quickstart)
A token must be 16 bytes or more (the server refuses a shorter one) and use only letters, digits and . _ ~ + / = -

  --name NAME           the name Claude Code gives the server (default meldframe)

The line puts the token on the command line and Claude Code keeps it in its own configuration. A development
token only works against a server on this machine; see docs/quickstart-agent.md for the limits.
EOF
}

die() { echo "connect-claude: $1" >&2; exit 1; }

listen=""; token=""; name="meldframe"
while [ $# -gt 0 ]; do
  case "$1" in
    -h | --help) usage; exit 0 ;;
    --port) [ $# -ge 2 ] || die "--port needs a number"; listen="127.0.0.1:$2"; shift 2 ;;
    --listen) [ $# -ge 2 ] || die "--listen needs HOST:PORT"; listen="$2"; shift 2 ;;
    --token) [ $# -ge 2 ] || die "--token needs a value"; token="$2"; shift 2 ;;
    --name) [ $# -ge 2 ] || die "--name needs a value"; name="$2"; shift 2 ;;
    *) die "unknown argument $1 (see --help)" ;;
  esac
done

[ -n "$listen" ] || listen="${MELDFRAME_LISTEN:-127.0.0.1:8080}"
case "$listen" in
  *:*) ;;
  *) die "the address '$listen' is not HOST:PORT" ;;
esac
host=${listen%:*}
port=${listen##*:}
case "$port" in
  '' | *[!0-9]*) die "the port '$port' is not a number" ;;
esac
[ "$port" -ge 1 ] && [ "$port" -le 65535 ] || die "the port $port is out of range"
case "$host" in
  0.0.0.0 | '[::]' | '') host=127.0.0.1 ;;
esac
case "$name" in
  '' | *[!A-Za-z0-9_-]*) die "the name must be letters, digits, _ and - only" ;;
esac

if [ -z "$token" ]; then token="${MELDFRAME_AGENT_TOKEN:-}"; fi
if [ -z "$token" ]; then
  root=$(cd "$(dirname "$0")/.." && pwd)
  if [ -s "$root/.dev-token" ]; then
    token=$(head -n 1 "$root/.dev-token" | tr -d '\r\n')
  fi
fi
[ -n "$token" ] || die "no token: pass --token, set MELDFRAME_AGENT_TOKEN, or run 'just dev' once to create .dev-token"
case "$token" in
  *[!A-Za-z0-9._~+/=-]*) die "the token holds a character outside letters, digits and . _ ~ + / = -" ;;
esac
bytes=$(printf '%s' "$token" | wc -c | tr -d ' ')
[ "$bytes" -ge 16 ] || die "the token is $bytes bytes; the server refuses a token shorter than 16 bytes"

printf 'claude mcp add --transport http %s http://%s:%s/mcp --header "Authorization: Bearer %s"\n' "$name" "$host" "$port" "$token"
