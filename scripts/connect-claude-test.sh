#!/usr/bin/env bash
# Checks the options of tools/connect-claude.sh: --port and --listen are alternatives, so the two together are
# refused in either order (issue 284), and each alone still prints the line. The script only assembles a line
# and calls nothing; the token here is made up. Run by `just check`.
set -u
cd "$(dirname "$0")/.."
fail=0
token=testtoken0123456789
out=$(mktemp "${TMPDIR:-/tmp}/connect-claude-test.XXXXXX") || exit 1
err=$(mktemp "${TMPDIR:-/tmp}/connect-claude-test.XXXXXX") || exit 1
trap 'rm -f "$out" "$err"' EXIT

# expect EXIT DESCRIPTION STDOUT-SUBSTRING STDERR-SUBSTRING ARG...
# An empty substring is not checked; a refusal (exit 1) must print nothing on standard output.
expect() {
  local want=$1 desc=$2 want_out=$3 want_err=$4
  shift 4
  env -i PATH="$PATH" MELDFRAME_AGENT_TOKEN="$token" bash tools/connect-claude.sh "$@" >"$out" 2>"$err"
  local got=$? ok=1
  [ "$got" -eq "$want" ] || ok=0
  if [ -n "$want_out" ] && ! grep -qF -- "$want_out" "$out"; then ok=0; fi
  if [ -n "$want_err" ] && ! grep -qF -- "$want_err" "$err"; then ok=0; fi
  if [ "$want" -ne 0 ] && [ -s "$out" ]; then ok=0; fi
  if [ "$ok" -eq 1 ]; then echo "pass  $desc"; else echo "FAIL  $desc (exit $got, wanted $want)"; fail=1; fi
}

refusal="connect-claude: use --port or --listen, not both"
expect 0 "--port alone"                      "http://127.0.0.1:8081/mcp" ""         --port 8081
expect 0 "--listen alone"                    "http://example.test:1/mcp" ""         --listen example.test:1
expect 0 "neither, the default address"      "http://127.0.0.1:8080/mcp" ""
expect 1 "--port then --listen"              ""                          "$refusal" --port 8081 --listen example.test:1
expect 1 "--listen then --port"              ""                          "$refusal" --listen example.test:1 --port 8081
expect 1 "both in the --option=VALUE form"   ""                          "$refusal" --port=8081 --listen=example.test:1
expect 1 "both, with other options between"  ""                          "$refusal" --port 8081 --name x --listen example.test:1
expect 0 "--port twice, as before"           "http://127.0.0.1:8082/mcp" ""         --port 8081 --port 8082
[ $fail -eq 0 ] && echo "connect-claude options behave"
exit $fail
