#!/usr/bin/env bash
# Start Postgres for development and tests.
#   - Docker daemon reachable: `docker compose up -d --wait postgres`.
#   - Otherwise: a local Postgres cluster without Docker (idempotent; run it again after a
#     restart of the machine or sandbox, the process does not survive). The binaries are found in
#     /usr/lib/postgresql/*/bin or Homebrew's postgresql@16, or in MELDFRAME_PG_BIN.
# Both listen on 127.0.0.1:${MELDFRAME_PG_PORT:-5433} and create the databases
# meldframe_dev and meldframe_test. Development only: fsync is off, auth is trust.
set -euo pipefail

PORT=${MELDFRAME_PG_PORT:-5433}
if [ "$(id -u)" = 0 ]; then DEFAULT_DATA=/var/lib/meldframe-pg; else DEFAULT_DATA=${XDG_DATA_HOME:-$HOME/.local/share}/meldframe-pg; fi
DATA=${MELDFRAME_PG_DATA:-$DEFAULT_DATA}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DBS="meldframe_dev meldframe_test"

echo "DATABASE_URL=postgres://postgres:postgres@127.0.0.1:$PORT/meldframe_dev"
echo "TEST_DATABASE_URL=postgres://postgres:postgres@127.0.0.1:$PORT/meldframe_test"

if docker info >/dev/null 2>&1; then
  MELDFRAME_PG_PORT=$PORT docker compose -f "$ROOT/docker-compose.yml" up -d --wait postgres
  echo "postgres ready (docker compose) on 127.0.0.1:$PORT"
  exit 0
fi

# The local binaries are looked up only here: with Docker they are not needed, and where the Debian path does not
# exist (macOS) the lookup must not end the script. MELDFRAME_PG_BIN names the directory of pg_ctl and initdb.
find_bin() {
  if [ -n "${MELDFRAME_PG_BIN:-}" ]; then
    printf '%s\n' "$MELDFRAME_PG_BIN"
    return 0
  fi
  # Homebrew's postgresql@16 (Apple silicon, then Intel); MELDFRAME_PG_BREW_ROOTS replaces the two roots (a test points it
  # at an empty directory so that a machine that has the formula does not change the answer).
  local d root brew=""
  for root in ${MELDFRAME_PG_BREW_ROOTS:-/opt/homebrew/opt /usr/local/opt}; do brew="$brew $root/postgresql@16/bin"; done
  for d in $(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -n 1 || true) $brew; do
    if [ -x "$d/pg_ctl" ]; then
      printf '%s\n' "$d"
      return 0
    fi
  done
  return 0
}
BIN=$(find_bin)
if [ -n "${MELDFRAME_PG_BIN:-}" ] && [ ! -x "$BIN/pg_ctl" ]; then
  echo "no Docker, and MELDFRAME_PG_BIN='$MELDFRAME_PG_BIN' has no executable pg_ctl: set it to the directory of pg_ctl and initdb" >&2
  exit 1
fi
[ -n "$BIN" ] && [ -x "$BIN/pg_ctl" ] || {
  echo "no Docker and no Postgres binaries found (looked for /usr/lib/postgresql/*/bin and Homebrew's postgresql@16); install Docker or PostgreSQL 16, or set MELDFRAME_PG_BIN to the directory of pg_ctl" >&2
  exit 1
}

# Run a command as the postgres OS user when root (initdb refuses to run as root).
as_pg() {
  if [ "$(id -u)" = 0 ]; then su postgres -s /bin/bash -c "$*"; else bash -c "$*"; fi
}

if [ ! -f "$DATA/PG_VERSION" ]; then
  mkdir -p "$DATA"
  [ "$(id -u)" = 0 ] && chown postgres:postgres "$DATA"
  as_pg "'$BIN/initdb' -D '$DATA' -A trust -U postgres >/dev/null"
fi

as_pg "'$BIN/pg_ctl' -D '$DATA' status >/dev/null 2>&1 || '$BIN/pg_ctl' -D '$DATA' -o '-p $PORT -c listen_addresses=127.0.0.1 -c unix_socket_directories=/tmp -c fsync=off -c synchronous_commit=off -c max_connections=200' -l '$DATA/log.txt' -w start >/dev/null"

for db in $DBS; do
  psql -h 127.0.0.1 -p "$PORT" -U postgres -qtAc "select 1 from pg_database where datname='$db'" | grep -q 1 \
    || psql -h 127.0.0.1 -p "$PORT" -U postgres -qc "create database $db"
done
echo "postgres ready (local, no Docker) on 127.0.0.1:$PORT, data in $DATA"
