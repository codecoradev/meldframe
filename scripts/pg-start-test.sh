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
  # The environment of the machine does not decide: no inherited MELDFRAME_PG_BIN, no Homebrew formula found, and a data
  # directory of its own, so a machine that has Postgres installed does not start one here.
  out=$(env -u MELDFRAME_PG_BIN MELDFRAME_PG_BREW_ROOTS="$tmp/none" MELDFRAME_PG_DATA="$tmp/data" "$@" PATH="$tmp/bin:$PATH" bash scripts/pg-start.sh 2>&1)
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
printf '#!/bin/sh\nexit 1\n' >"$tmp/bin/docker"
chmod +x "$tmp/bin/docker"
check "no Docker and no local binaries: says so and names MELDFRAME_PG_BIN" 1 "no Postgres binaries found" \
  MELDFRAME_PG_PORT=5499
# A MELDFRAME_PG_BIN that is wrong is named back, and nothing else is tried in its place.
check "no Docker and a wrong MELDFRAME_PG_BIN: names the path it was given" 1 "MELDFRAME_PG_BIN='$tmp/nowhere'" \
  MELDFRAME_PG_PORT=5499 MELDFRAME_PG_BIN="$tmp/nowhere"
# The local path with a directory of binaries of its own (a Homebrew keg-only postgresql@16 is not on PATH, and its psql is
# the one that matches the server): fakes of pg_ctl, initdb and psql log what they are asked, and the script must start the
# cluster and talk to it with the psql of that directory, not the one on PATH.
mkdir "$tmp/pgbin"
printf '#!/bin/sh\necho 1000\n' >"$tmp/bin/id"
printf '#!/bin/sh\necho "pg_ctl $*" >>"$FAKE_CALLS"\n[ "$3" = status ] && exit 3\nexit 0\n' >"$tmp/pgbin/pg_ctl"
printf '#!/bin/sh\necho "initdb $*" >>"$FAKE_CALLS"\nmkdir -p "$2" && : >"$2/PG_VERSION"\n' >"$tmp/pgbin/initdb"
printf '#!/bin/sh\necho "psql-of-the-bin-dir $*" >>"$FAKE_CALLS"\nexit 0\n' >"$tmp/pgbin/psql"
chmod +x "$tmp/bin/id" "$tmp/pgbin/pg_ctl" "$tmp/pgbin/initdb" "$tmp/pgbin/psql"
export FAKE_CALLS="$tmp/calls.log"
: >"$FAKE_CALLS"
check "no Docker and binaries in MELDFRAME_PG_BIN: starts a cluster and says it is ready" 0 "postgres ready (local, no Docker)" \
  MELDFRAME_PG_PORT=5499 MELDFRAME_PG_BIN="$tmp/pgbin"
if grep -q "^initdb " "$FAKE_CALLS" && grep -q "^pg_ctl .* start" "$FAKE_CALLS" && grep -q "^psql-of-the-bin-dir .*create database meldframe_dev" "$FAKE_CALLS"; then
  echo "pass  the cluster is made with initdb and pg_ctl and the databases with the psql of that directory"
else
  echo "FAIL  the cluster is made with initdb and pg_ctl and the databases with the psql of that directory"
  sed 's/^/      /' "$FAKE_CALLS"
  fail=1
fi
exit $fail
