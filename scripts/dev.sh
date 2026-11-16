#!/usr/bin/env bash
# One command for development: start Postgres, seed, and run the server on it (`just dev`).
#   - Postgres: scripts/pg-start.sh (docker compose, or a local cluster without Docker).
#   - The server applies the migrations itself when it connects (the sqlx migrator owns the schema).
#   - A development token is generated on first run, saved in the git-ignored .dev-token (mode 600) and
#     printed once. The server gets it through the environment, never through argv.
# Variables (or .env, see .env.example): DATABASE_URL, MELDFRAME_LISTEN (default 127.0.0.1:8080).
set -euo pipefail
cd "$(dirname "$0")/.."

[ -f .env ] && { set -a; . ./.env; set +a; }
export MELDFRAME_ENV=${MELDFRAME_ENV:-development}
export DATABASE_URL=${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:${MELDFRAME_PG_PORT:-5433}/meldframe_dev}
export MELDFRAME_LISTEN=${MELDFRAME_LISTEN:-127.0.0.1:8080}

FRESH=0; [ -s .dev-token ] || FRESH=1
scripts/pg-start.sh >/dev/null
scripts/dev-seed.sh >/dev/null

# The seed writes .dev-token when it issues a token; generate one only if it did not.
[ -s .dev-token ] || (umask 077; printf 'mf_dev_%s\n' "$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')" > .dev-token)
if [ "$FRESH" = 1 ]; then
  echo "development token (shown once, saved to .dev-token): $(cat .dev-token)"
else
  echo "development token: see .dev-token"
fi
# token:workspace:actor, the workspace and actor of the seed.
export MELDFRAME_DEV_TOKENS="$(cat .dev-token):ws_dev:agent:dev"

SAFE_DB=${DATABASE_URL#*@}; [ "$SAFE_DB" = "$DATABASE_URL" ] && SAFE_DB=${DATABASE_URL#*://} # host:port/database, no password
echo "server on http://$MELDFRAME_LISTEN (database $SAFE_DB)"
echo "health (no token): curl http://$MELDFRAME_LISTEN/v1/health"
echo "tool call (token read from .dev-token, not from argv):"
echo "  printf 'header = \"Authorization: Bearer %s\"\\n' \"\$(cat .dev-token)\" | curl --config - -d '{\"ops\":[]}' http://$MELDFRAME_LISTEN/v1/files/file_dev/tools/example_edit"
exec cargo run -q -p meldframe-server
