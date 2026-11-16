#!/usr/bin/env bash
# Smoke test of the server binary on Postgres: start it on a random port with a throwaway token and a
# scratch database, submit an op over REST, restart the server and show the file came back from Postgres.
# Needs DATABASE_URL (any database on a Postgres server, used only to create and drop the scratch one);
# without it the test is skipped, unless MELDFRAME_REQUIRE_PG is set. Run `just dev-db` for a local one.
set -euo pipefail
cd "$(dirname "$0")/.."

if [ -z "${DATABASE_URL:-}" ]; then
  [ -n "${MELDFRAME_REQUIRE_PG:-}" ] && { echo "FAIL: DATABASE_URL is not set" >&2; exit 1; }
  echo "SKIP: DATABASE_URL is not set"; exit 0
fi
ADMIN_URL=${DATABASE_URL%%\?*}; ADMIN_URL="${ADMIN_URL%/*}/postgres"
DB="meldframe_smoke_$$"
URL="${ADMIN_URL%/*}/$DB"
TOKEN="smoke-$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')"
PORT=$(shuf -i 20000-60000 -n 1)
LOG=$(mktemp)
PID=""

stop() { [ -n "$PID" ] && kill "$PID" 2>/dev/null && wait "$PID" 2>/dev/null || true; PID=""; }
cleanup() { stop; psql "$ADMIN_URL" -q -c "drop database if exists $DB with (force)" >/dev/null 2>&1 || true; rm -f "$LOG"; }
trap cleanup EXIT
fail() { echo "FAIL: $1" >&2; echo "--- server log ---" >&2; cat "$LOG" >&2; exit 1; }

cargo build -q -p meldframe-server
TARGET=$(cargo metadata --format-version 1 --no-deps | grep -o '"target_directory":"[^"]*"' | sed 's/.*:"\(.*\)"/\1/')
BIN="$TARGET/${CARGO_BUILD_TARGET:+$CARGO_BUILD_TARGET/}debug/meldframe-server"
[ -x "$BIN" ] || fail "no binary at $BIN"
psql "$ADMIN_URL" -q -v ON_ERROR_STOP=1 -c "create database $DB"

run() { # command line of the server; the token goes through the environment
  DATABASE_URL="$URL" MELDFRAME_LISTEN="127.0.0.1:$PORT" MELDFRAME_DEV_TOKENS="$TOKEN:ws_smoke:agent:smoke" "$BIN"
}
start() { # $1 = retry on another port once when the first port is taken
  run >>"$LOG" 2>&1 &
  PID=$!
  for _ in $(seq 100); do
    curl -sf "http://127.0.0.1:$PORT/v1/health" >/dev/null 2>&1 && return 0
    if ! kill -0 "$PID" 2>/dev/null; then
      [ "${1:-}" = retry ] || fail "the server exited at start"
      PORT=$(shuf -i 20000-60000 -n 1); : >"$LOG"; start; return
    fi
    sleep 0.2
  done
  fail "the server did not answer /v1/health"
}
submit() { # key, node id
  printf 'header = "Authorization: Bearer %s"\n' "$TOKEN" | curl -s --config - -H 'content-type: application/json' \
    -d "{\"ops\":[{\"key\":\"$1\",\"command\":{\"type\":\"addNode\",\"node\":{\"id\":\"$2\",\"type\":\"frame\"}}}]}" \
    "http://127.0.0.1:$PORT/v1/files/file_smoke/tools/example_edit"
}
expect() { case "$2" in *"$1"*) ;; *) fail "expected $1 in: $2" ;; esac; }

echo "[1/5] start on 127.0.0.1:$PORT with Postgres"
start retry
expect '"status":"ok"' "$(curl -s "http://127.0.0.1:$PORT/v1/health")"
echo "[2/5] submit an op over REST"
expect '"written":true' "$(submit k1 n1)"
expect '"seq":1' "$(submit k1 n1)"
echo "[3/5] restart the server"
stop; start
echo "[4/5] the file is restored: the old key is a duplicate and the next number continues"
R=$(submit k1 n1); expect '"written":false' "$R"; expect '"seq":1' "$R"
R=$(submit k2 n2); expect '"written":true' "$R"; expect '"seq":2' "$R"
N=$(psql "$URL" -qtAc "select count(*) from ops where file_id like '%file_smoke'")
[ "$N" = 2 ] || fail "expected 2 ops in Postgres, found $N"
echo "[5/5] a taken port is a clean error (exit 1, message, no panic), not a crash"
set +e; BUSY=$(run 2>&1); RC=$?; set -e
[ "$RC" = 1 ] || fail "second server on a taken port exited with $RC, expected 1: $BUSY"
expect "in use" "$BUSY"
case "$BUSY" in *panicked*) fail "the second server panicked: $BUSY" ;; esac
echo "OK: ops submitted over REST survive a server restart (2 ops in Postgres)"
