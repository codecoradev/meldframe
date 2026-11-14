#!/usr/bin/env bash
# Checks the guards of scripts/dev-seed.sh without a database (MELDFRAME_SEED_DRY_RUN=1 stops
# after the guards). Run by `just check`.
set -u
cd "$(dirname "$0")/.."
fail=0
expect() { # expected-exit description env...
  local want=$1 desc=$2; shift 2
  env "$@" MELDFRAME_SEED_DRY_RUN=1 bash scripts/dev-seed.sh >/dev/null 2>&1
  local got=$?
  if [ "$got" -eq "$want" ]; then echo "pass  $desc"; else echo "FAIL  $desc (exit $got, wanted $want)"; fail=1; fi
}
expect 0 "local dev database"            DATABASE_URL=postgres://postgres:postgres@127.0.0.1:5433/meldframe_dev
expect 0 "local test database, localhost" DATABASE_URL=postgresql://u@localhost/meldframe_test
expect 0 "ipv6 loopback"                  DATABASE_URL='postgres://u@[::1]:5433/meldframe_dev?sslmode=disable'
expect 3 "remote host"                    DATABASE_URL=postgres://u:p@db.example.com:5432/meldframe_dev
expect 3 "other database name"            DATABASE_URL=postgres://u:p@127.0.0.1:5433/production
expect 3 "name that only starts alike"    DATABASE_URL=postgres://u:p@127.0.0.1:5433/meldframe_dev_copy
expect 3 "host that only starts alike"    DATABASE_URL=postgres://u:p@127.0.0.1.evil.example/meldframe_dev
expect 2 "release marker wins"            DATABASE_URL=postgres://u@127.0.0.1/meldframe_dev MELDFRAME_ENV=release
expect 2 "release flag wins"              DATABASE_URL=postgres://u@127.0.0.1/meldframe_dev MELDFRAME_RELEASE=1
[ $fail -eq 0 ] && echo "all seed guards behave"
exit $fail
