#!/usr/bin/env bash
# The web client against the dev server (`just web`): the Vite dev server of apps/web, proxying /v1 to the server of
# `just dev` and adding the dev token there (the browser never holds it).
# Variables (or .env): MELDFRAME_LISTEN (the server, default 127.0.0.1:8080), MELDFRAME_WEB_TOKEN (default: .dev-token),
# MELDFRAME_WEB_ACTOR (default agent:dev, the actor of the seed token), MELDFRAME_WEB_FILE (default file_dev).
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f .env ] && { set -a; . ./.env; set +a; }
if [ -z "${MELDFRAME_WEB_TOKEN:-}" ]; then
  [ -s .dev-token ] || { echo "web: no .dev-token yet; run 'just dev' first (it creates it)" >&2; exit 1; }
  MELDFRAME_WEB_TOKEN=$(cat .dev-token)
fi
export MELDFRAME_WEB_TOKEN
export MELDFRAME_WEB_ACTOR=${MELDFRAME_WEB_ACTOR:-agent:dev}
export MELDFRAME_WEB_FILE=${MELDFRAME_WEB_FILE:-file_dev}
export MELDFRAME_SERVER=${MELDFRAME_SERVER:-http://${MELDFRAME_LISTEN:-127.0.0.1:8080}}
cd apps/web
[ -d node_modules ] || npm ci
exec npm run dev
