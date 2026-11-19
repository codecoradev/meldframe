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
expect 0 "default-style url, localhost:5433" DATABASE_URL=postgres://postgres:postgres@localhost:5433/meldframe_test
expect 0 "harmless query, several keys"   DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?sslmode=disable&connect_timeout=5&application_name=seed'
expect 3 "query dbname override"          DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?dbname=production'
expect 3 "query host override"            DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?host=db.example.com'
expect 3 "query hostaddr override"        DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?hostaddr=10.0.0.5'
expect 3 "query service override"         DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?service=prod'
expect 3 "query port override"            DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?port=6543'
expect 3 "harmless key then host"         DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?sslmode=disable&host=evil'
expect 3 "query key in other case"        DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?Host=evil'
expect 3 "query value with percent"       DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?application_name=%2e'
expect 3 "empty query key"                DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev?'
expect 3 "fragment"                       DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev#x'
expect 3 "second at sign in userinfo"     DATABASE_URL='postgres://a@127.0.0.1@evil/meldframe_dev'
expect 3 "second host after at sign"      DATABASE_URL='postgres://127.0.0.1@evil.example/meldframe_dev'
expect 3 "multi-host list"                DATABASE_URL='postgres://127.0.0.1,evil.example/meldframe_dev'
expect 3 "multi-host list, ports"         DATABASE_URL='postgres://u@127.0.0.1:5433,evil.example:5432/meldframe_dev'
expect 3 "percent-encoded dot in host"    DATABASE_URL='postgres://u@127%2e0.0.1/meldframe_dev'
expect 3 "percent-encoded host"           DATABASE_URL='postgres://u@%6c%6fcalhost/meldframe_dev'
expect 3 "percent-encoded user"  DATABASE_URL='postgres://u%40evil@127.0.0.1/meldframe_dev'
expect 3 "extra path segment"             DATABASE_URL='postgres://u@127.0.0.1/meldframe_dev/x'
expect 3 "no scheme slashes"              DATABASE_URL='postgres:u@127.0.0.1/meldframe_dev'
expect 3 "keyword connection string"      DATABASE_URL='host=evil dbname=meldframe_dev'
expect 2 "release marker wins"            DATABASE_URL=postgres://u@127.0.0.1/meldframe_dev MELDFRAME_ENV=release
expect 2 "release flag wins"              DATABASE_URL=postgres://u@127.0.0.1/meldframe_dev MELDFRAME_RELEASE=1
[ $fail -eq 0 ] && echo "all seed guards behave"
exit $fail
