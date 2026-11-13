#!/usr/bin/env bash
# Start Postgres for development and tests.
#   - Docker daemon reachable: `docker compose up -d --wait postgres`.
#   - Otherwise: a local Postgres cluster without Docker (idempotent; run it again after a
#     restart of the machine or sandbox, the process does not survive).
# Both listen on 127.0.0.1:${MELDFRAME_PG_PORT:-5433} and create the databases
# meldframe_dev and meldframe_test. Development only: fsync is off, auth is trust.
set -euo pipefail

PORT=${MELDFRAME_PG_PORT:-5433}
BIN=${MELDFRAME_PG_BIN:-$(ls -d /usr/lib/postgresql/*/bin 2>/dev/null | sort -V | tail -n 1)}
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

[ -x "$BIN/pg_ctl" ] || { echo "no Docker and no Postgres binaries (looked in '$BIN'); install postgresql or Docker" >&2; exit 1; }

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
