#!/usr/bin/env bash
# Prints the `claude mcp add` line that connects Claude Code to a Meldframe development server
# (docs/quickstart-agent.md). It only assembles the line: it starts nothing, calls no server, runs
# no `claude` command and writes no file. The token goes to standard output in that one line and
# nowhere else; no message on standard error holds the token or the value of any argument.
set -eu

usage() {
  cat <<'EOF'
usage: tools/connect-claude.sh [--port N | --listen HOST:PORT] [--token TOKEN] [--name NAME]
       (each option also takes the form --option=VALUE)

Prints the `claude mcp add` line for the MCP endpoint of a development server.

Server address, first that is set wins:
  --port N              the server listens on 127.0.0.1:N
  --listen HOST:PORT    the same value as MELDFRAME_LISTEN
  MELDFRAME_LISTEN      in the environment (0.0.0.0 or [::] is printed as 127.0.0.1)
  default               127.0.0.1:8080, the address of `just dev`
The host is an IPv4 address, a bracketed IPv6 address or a name of letters, digits, . and -.

Token, first that is set wins:
  --token TOKEN         visible to other users of this machine in the process list; prefer the next one
  MELDFRAME_AGENT_TOKEN in the environment
  .dev-token            in the repository root (the file `just dev` writes: the token of the editor's own
                        actor, agent:dev; a token of its own for the agent is better, see the quickstart)
A token must be 16 bytes or more (the server refuses a shorter one) and use only letters, digits and . _ ~ + / = -

  --name NAME           the name Claude Code gives the server (default meldframe)

The printed line holds the token. Running it puts the token in your shell history (start the command with a space
under HISTCONTROL=ignorespace, or paste the line from the clipboard), and the terminal scrollback and any screenshot
keep it too. Do not put the line, or a file that holds its header, in any file of a repository. To rotate the token,
restart the server with a new one. A development token only works against a server on this machine; see
docs/quickstart-agent.md for the limits.
EOF
}

die() { echo "connect-claude: $1" >&2; exit 1; }

listen=""; token=""; name="meldframe"; from_file=0
while [ $# -gt 0 ]; do
  case "$1" in
    -h | --help) usage; exit 0 ;;
    --port=*) listen="127.0.0.1:${1#--port=}"; shift ;;
    --listen=*) listen="${1#--listen=}"; shift ;;
    --token=*) token="${1#--token=}"; shift ;;
    --name=*) name="${1#--name=}"; shift ;;
    --port) [ $# -ge 2 ] || die "--port needs a number"; listen="127.0.0.1:$2"; shift 2 ;;
    --listen) [ $# -ge 2 ] || die "--listen needs HOST:PORT"; listen="$2"; shift 2 ;;
    --token) [ $# -ge 2 ] || die "--token needs a value"; token="$2"; shift 2 ;;
    --name) [ $# -ge 2 ] || die "--name needs a value"; name="$2"; shift 2 ;;
    *) die "unknown argument (see --help)" ;;
  esac
done

[ -n "$listen" ] || listen="${MELDFRAME_LISTEN:-127.0.0.1:8080}"
case "$listen" in
  *:*) ;;
  *) die "the address is not HOST:PORT" ;;
esac
host=${listen%:*}
port=${listen##*:}
case "$port" in
  '' | *[!0-9]*) die "the port is not a number" ;;
esac
[ "${#port}" -le 5 ] || die "the port is out of range"
while [ "${#port}" -gt 1 ] && [ "${port#0}" != "$port" ]; do port=${port#0}; done
[ "$port" -ge 1 ] && [ "$port" -le 65535 ] || die "the port is out of range"
case "$host" in
  0.0.0.0 | '[::]' | '') host=127.0.0.1 ;;
esac
case "$host" in
  \[*\])
    inner=${host#\[}; inner=${inner%\]}
    case "$inner" in
      '' | *[!0-9A-Fa-f:.]*) die "the host is not an IPv4 address, a bracketed IPv6 address or a name" ;;
    esac ;;
  '' | *[!A-Za-z0-9.-]*) die "the host is not an IPv4 address, a bracketed IPv6 address or a name" ;;
esac
case "$name" in
  '' | *[!A-Za-z0-9_-]*) die "the name must be letters, digits, _ and - only" ;;
esac

if [ -z "$token" ]; then token="${MELDFRAME_AGENT_TOKEN:-}"; fi
if [ -z "$token" ]; then
  root=$(cd "$(dirname "$0")/.." && pwd)
  if [ -s "$root/.dev-token" ]; then
    token=$(head -n 1 "$root/.dev-token" | tr -d '\r\n')
    from_file=1
  fi
fi
[ -n "$token" ] || die "no token: pass --token, set MELDFRAME_AGENT_TOKEN, or run 'just dev' once to create .dev-token"
case "$token" in
  *[!A-Za-z0-9._~+/=-]*) die "the token holds a character outside letters, digits and . _ ~ + / = -" ;;
esac
bytes=$(printf '%s' "$token" | wc -c | tr -d ' ')
[ "$bytes" -ge 16 ] || die "the token is $bytes bytes; the server refuses a token shorter than 16 bytes"

if [ "$from_file" = 1 ]; then
  echo "connect-claude: using .dev-token, the editor's token (agent:dev); give the agent a token of its own if you want to check review (see docs/quickstart-agent.md)" >&2
fi
case "$host" in
  localhost | 127.0.0.1 | '[::1]') ;;
  *) echo "connect-claude: the server's Host guard answers 403 unless MELDFRAME_MCP_HOSTS lists this host" >&2 ;;
esac

printf 'claude mcp add --transport http %s http://%s:%s/mcp --header "Authorization: Bearer %s"\n' "$name" "$host" "$port" "$token"
