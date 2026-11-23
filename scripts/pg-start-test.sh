#!/usr/bin/env bash
# Checks that scripts/pg-start.sh does not need local Postgres binaries when Docker is reachable (issue 224: on macOS
# there is no /usr/lib/postgresql, the lookup failed, and `set -e` with `pipefail` ended the script before the Docker
# branch). A fake `docker` that answers, and a fake `ls` that fails as it does for a missing directory, stand in for
# the machine; nothing is started. Run by `just check`.
set -u
cd "$(dirname "$0")/.."
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bin"
cat >"$tmp/bin/docker" <<'DOCKER'
#!/bin/sh
# `docker info` answers; `docker compose ... up` says what it was asked.
[ "$1" = info ] && exit 0
echo "fake docker $*"
exit 0
DOCKER
cat >"$tmp/bin/ls" <<'LS'
#!/bin/sh
# What `ls -d /usr/lib/postgresql/*/bin` does where there is no such directory.
echo "ls: cannot access '$2': No such file or directory" >&2
exit 2
LS
chmod +x "$tmp/bin/docker" "$tmp/bin/ls"
fail=0
check() { # description, wanted exit, wanted text, env...
  local desc=$1 want=$2 text=$3
  shift 3
  local out got
  out=$(env "$@" PATH="$tmp/bin:$PATH" bash scripts/pg-start.sh 2>&1)
  got=$?
  if [ "$got" -eq "$want" ] && printf '%s' "$out" | grep -q -- "$text"; then
    echo "pass  $desc"
  else
    echo "FAIL  $desc (exit $got, wanted $want, wanted text '$text')"
    printf '%s\n' "$out" | sed 's/^/      /'
    fail=1
  fi
}
check "Docker reachable and no local binaries: compose starts Postgres" 0 "postgres ready (docker compose)" \
  MELDFRAME_PG_PORT=5499
# Without Docker and without binaries the script stops with a message that names the way out.
mv "$tmp/bin/docker" "$tmp/bin/docker.off"
printf '#!/bin/sh\nexit 1\n' >"$tmp/bin/docker"
chmod +x "$tmp/bin/docker"
check "no Docker and no local binaries: says so and names MELDFRAME_PG_BIN" 1 "MELDFRAME_PG_BIN" \
  MELDFRAME_PG_PORT=5499
exit $fail
