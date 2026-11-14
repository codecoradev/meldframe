#!/usr/bin/env bash
# Load the development seed (workspace ws_dev, file file_dev and a random per-install token) into
# DATABASE_URL. The token is printed once and saved to .dev-token (git-ignored); only its hash is stored.
# Development only: refuses to run when the environment marks a release build, and refuses any
# database that is not meldframe_dev or meldframe_test on this machine.
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

# Allowlist, not blocklist: the seed publishes a known token, so it only runs against a database on
# this machine whose name says it is for development or tests. Anything else is refused whatever
# the environment says about releases.
allowed_target() {
  local re='^postgres(ql)?://([^@/]*@)?(\[::1\]|127\.0\.0\.1|localhost)(:[0-9]+)?/(meldframe_dev|meldframe_test)(\?.*)?$'
  [[ "$1" =~ $re ]]
}
if ! allowed_target "$URL"; then
  echo "refusing to seed: DATABASE_URL must point at a database named meldframe_dev or meldframe_test on this machine (127.0.0.1, localhost or [::1]). The development seed publishes a known token." >&2
  exit 3
fi
[ "${MELDFRAME_SEED_DRY_RUN:-}" = 1 ] && { echo "dry run: target allowed"; exit 0; }

# The value is generated here from /dev/urandom on every run that needs one; no secret is in the file.
TOKEN="mf_dev_$(head -c 16 /dev/urandom | od -An -tx1 | tr -d ' \n')" # cora-ignore: sec-hardcoded-secret
if command -v sha256sum >/dev/null 2>&1; then HASH=$(printf %s "$TOKEN" | sha256sum | cut -d' ' -f1); else HASH=$(printf %s "$TOKEN" | shasum -a 256 | cut -d' ' -f1); fi
ISSUED=$(PGOPTIONS="-c client_min_messages=warning" psql "$URL" -v ON_ERROR_STOP=1 -q -At -v token_hash="$HASH" -f "$ROOT/scripts/dev-seed.sql" | tail -1)
echo "seeded: workspace ws_dev, file file_dev"
if [ "$ISSUED" = 1 ]; then
  (umask 077; printf '%s\n' "$TOKEN" > "$ROOT/.dev-token")
  echo "development token (shown once, saved to .dev-token): $TOKEN"
else
  echo "development token already issued (see .dev-token); delete the row in dev_seed.dev_token to issue a new one"
fi
