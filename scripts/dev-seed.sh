#!/usr/bin/env bash
# Load the development seed (workspace ws_dev, file file_dev, token below) into DATABASE_URL.
# Development only: refuses to run when the environment marks a release build.
#   dev token: mf_dev_token_do_not_use_in_production
set -euo pipefail

is_release() {
  case "${MELDFRAME_ENV:-}" in release | production | prod) return 0 ;; esac
  case "${MELDFRAME_RELEASE:-}" in 1 | true | yes) return 0 ;; esac
  case "${PROFILE:-}${CARGO_BUILD_PROFILE:-}" in *release*) return 0 ;; esac
  return 1
}
if is_release; then
  echo "refusing to seed: this is a release environment (MELDFRAME_ENV=${MELDFRAME_ENV:-}, MELDFRAME_RELEASE=${MELDFRAME_RELEASE:-}). The development seed is for development only." >&2
  exit 2
fi

ROOT=$(cd "$(dirname "$0")/.." && pwd)
[ -f "$ROOT/.env" ] && [ -z "${DATABASE_URL:-}" ] && { set -a; . "$ROOT/.env"; set +a; }
URL=${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:${MELDFRAME_PG_PORT:-5433}/meldframe_dev}
PGOPTIONS="-c client_min_messages=warning" psql "$URL" -v ON_ERROR_STOP=1 -q -f "$ROOT/scripts/dev-seed.sql"
echo "seeded: workspace ws_dev, file file_dev, token mf_dev_token_do_not_use_in_production"
